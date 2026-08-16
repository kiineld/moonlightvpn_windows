//! Fetches a Remnawave subscription and returns a mihomo config plus whatever
//! the panel reports about the plan.
//!
//! ## Why the mihomo endpoint is tried first
//!
//! Remnawave serves a subscription in six shapes, selected by a path suffix:
//! `mihomo`, `clash`, `singbox`, `stash`, `json` (xray) and `v2ray-json`. The
//! order here is not a preference:
//!
//! 1. **`/mihomo`** — a Clash.Meta YAML written by the panel operator. It can
//!    carry proxy groups, a `url-test` balancer across a dozen nodes, its own
//!    DNS and routing rules. This client keeps that config *verbatim* and
//!    overrides only the parts it must own (controller, ports, TUN, split
//!    rules), because the panel's tuning is usually better than anything
//!    generated here.
//! 2. **`/clash`** — the same idea for stock Clash. Slightly fewer features,
//!    still a real config with groups intact.
//! 3. **The bare URL** — base64 or plain share links, one URI per node. Every
//!    group, balancer and routing rule is flattened away by that format, so a
//!    node whose panel entry was a balancer arrives as a single unusable
//!    placeholder. This is the last resort, not a peer of the other two.
//!
//! A panel with no MIHOMO template configured 404s the first two, which is
//! exactly when the third earns its place.

use std::time::Duration;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use regex::Regex;
use thiserror::Error;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::mihomo_config;
use crate::models::SubscriptionInfo;
use crate::share_link;

#[derive(Debug, Error, PartialEq)]
pub enum Failure {
    #[error("Subscription link is not a valid http(s) URL")]
    BadUrl,
    #[error("Panel returned HTTP {0}")]
    Http(u16),
    #[error("Panel returned an empty subscription")]
    Empty,
    #[error("{0}")]
    Unusable(String),
    #[error("Could not reach the panel: {0}")]
    Transport(String),
}

/// Which endpoint answered — surfaced in the UI so a base64 fallback (with its
/// lost groups) is visible rather than silent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Mihomo,
    Clash,
    ShareLinks,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Mihomo => "mihomo",
            Source::Clash => "clash",
            Source::ShareLinks => "shareLinks",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Fetched {
    /// Raw mihomo/Clash YAML, ready for [`crate::mihomo_config`] to graft onto.
    pub yaml: String,
    pub info: SubscriptionInfo,
    pub source: Source,
}

/// The identity this install presents to the panel.
#[derive(Debug, Clone)]
pub struct DeviceIdentity {
    /// A random UUID minted once and stored, **not** a hardware identifier. It
    /// gives the panel a stable per-install handle for its device limit while
    /// carrying no hardware identity off the machine. Resetting the app mints a
    /// new one, which is the right trade.
    pub hwid: String,
    pub os_version: String,
    pub model: String,
    pub app_version: String,
}

pub struct SubscriptionClient {
    http: reqwest::Client,
    device: DeviceIdentity,
}

impl SubscriptionClient {
    pub fn new(device: DeviceIdentity) -> Result<Self, Failure> {
        Ok(SubscriptionClient {
            http: Self::make_client()?,
            device,
        })
    }

    /// A client that **ignores the machine's proxy settings**.
    ///
    /// This is the Windows counterpart of the Android client excluding itself
    /// from its own tunnel, and it matters for two reasons. While connected in
    /// system-proxy mode the app has pointed the whole machine at its own core,
    /// so a shared client would send the panel request back through the tunnel
    /// it is trying to manage — and a refresh during a half-open tunnel then
    /// hangs instead of failing. It also means a *stale* proxy left by any other
    /// client on the machine cannot swallow this app's requests, which is a
    /// silent hang with no timeout, because the connection is established and
    /// simply never answered.
    ///
    /// On Windows `no_proxy` is doing more work than on macOS: reqwest reads
    /// `HKCU\…\Internet Settings` for its default proxy, which is the exact key
    /// this client writes when it connects.
    fn make_client() -> Result<reqwest::Client, Failure> {
        reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(40))
            .connect_timeout(Duration::from_secs(20))
            // A subscription URL points at whatever host the panel operator
            // runs, and self-hosted panels are routinely reached by bare IP
            // with a self-signed certificate. This is the counterpart of the
            // macOS build's NSAllowsArbitraryLoads.
            .danger_accept_invalid_certs(true)
            .build()
            .map_err(|e| Failure::Transport(e.to_string()))
    }

    pub async fn fetch(&self, subscription_url: &str) -> Result<Fetched, Failure> {
        let base = normalize(subscription_url).ok_or(Failure::BadUrl)?;

        let mut last_error = Failure::Empty;

        for (suffix, source) in [("mihomo", Source::Mihomo), ("clash", Source::Clash)] {
            match self.get(&format!("{base}/{suffix}")).await {
                Ok((body, info)) => {
                    if looks_like_clash_config(&body) {
                        return Ok(Fetched {
                            yaml: body,
                            info,
                            source,
                        });
                    }
                    last_error =
                        Failure::Unusable(format!("{suffix} endpoint did not return a Clash config"));
                }
                Err(error) => last_error = error,
            }
        }

        // Last resort: share links, with every group and routing rule already
        // flattened out of them by the format itself.
        match self.get(&base).await {
            Ok((body, info)) => {
                if looks_like_clash_config(&body) {
                    return Ok(Fetched {
                        yaml: body,
                        info,
                        source: Source::Clash,
                    });
                }
                let links = share_link::decode_list(&body);
                if links.is_empty() {
                    return Err(Failure::Unusable(
                        "No nodes found in subscription".to_string(),
                    ));
                }
                let proxies: Vec<_> = links
                    .iter()
                    .filter_map(|l| share_link::mihomo_proxy(l))
                    .collect();
                if proxies.is_empty() {
                    return Err(Failure::Unusable(format!(
                        "Subscription has {} nodes, none of a type mihomo supports",
                        links.len()
                    )));
                }
                Ok(Fetched {
                    yaml: mihomo_config::yaml_from_proxies(&proxies),
                    info,
                    source: Source::ShareLinks,
                })
            }
            Err(error) => Err(if last_error == Failure::Empty {
                error
            } else {
                last_error
            }),
        }
    }

    /// `GET <sub>/info` — Remnawave's own JSON, which carries the device count
    /// the response headers do not.
    pub async fn fetch_info(&self, subscription_url: &str) -> Option<SubscriptionInfo> {
        let base = normalize(subscription_url)?;
        let response = self.send(&format!("{base}/info")).await.ok()?;
        if response.status() != reqwest::StatusCode::OK {
            return None;
        }
        let body = response.text().await.ok()?;
        info_from_remnawave_json(&body)
    }

    async fn get(&self, url: &str) -> Result<(String, SubscriptionInfo), Failure> {
        let response = self.send(url).await?;
        let status = response.status();
        if !status.is_success() {
            return Err(Failure::Http(status.as_u16()));
        }

        let info = info_from_headers(|name| {
            response
                .headers()
                .get(name)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string)
        });

        let body = response
            .text()
            .await
            .map_err(|e| Failure::Transport(e.to_string()))?;
        if body.is_empty() {
            return Err(Failure::Empty);
        }
        Ok((body, info))
    }

    async fn send(&self, url: &str) -> Result<reqwest::Response, Failure> {
        self.http
            .get(url)
            // Remnawave's device headers. The panel uses these for its device
            // limit and for picking a template when no suffix is given.
            .header("x-hwid", &self.device.hwid)
            .header("x-device-os", "Windows")
            .header("x-ver-os", &self.device.os_version)
            .header("x-device-model", &self.device.model)
            // Panels that route on User-Agent rather than on the suffix still
            // need to see a mihomo client here.
            .header(
                "User-Agent",
                format!("mihomo/1.19 moonlight/{}", self.device.app_version),
            )
            .send()
            .await
            .map_err(|e| Failure::Transport(e.to_string()))
    }
}

pub fn normalize(raw: &str) -> Option<String> {
    let text = raw.trim();
    if text.is_empty() {
        return None;
    }

    // A scheme that is already present must be http(s) — prepending `https://`
    // to `file:///etc/passwd` or to a `vless://` node link would turn a
    // rejection into a plausible-looking URL, and a deep link must not be able
    // to point the import flow at either.
    let scheme = Regex::new(r"^[a-zA-Z][a-zA-Z0-9+.\-]*://").expect("static pattern");
    let text = match scheme.find(text) {
        Some(m) => {
            let name = m.as_str().to_lowercase();
            if name != "http://" && name != "https://" {
                return None;
            }
            text.to_string()
        }
        None => format!("https://{text}"),
    };

    let url = url::Url::parse(&text).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    if url.host_str().is_none_or(str::is_empty) {
        return None;
    }

    // A trailing slash would make the suffix join produce `//mihomo`.
    Some(url.as_str().trim_end_matches('/').to_string())
}

/// Detected by content, not by `Content-Type` — panels mislabel it, and a
/// base64 body served as `text/yaml` would otherwise be fed to the parser.
pub fn looks_like_clash_config(body: &str) -> bool {
    body.lines()
        .any(|line| Regex::new(r"^\s*proxies\s*:").expect("static pattern").is_match(line))
}

/// Parses the two headers every panel implements consistently.
///
/// ```text
/// subscription-userinfo: upload=0; download=0; total=0; expire=0
/// profile-title: <utf8 or base64:…>
/// ```
///
/// A zero `total` or `expire` means *unlimited* in this format, not zero, so
/// both map to `None` rather than to 0.
pub fn info_from_headers(header: impl Fn(&str) -> Option<String>) -> SubscriptionInfo {
    let mut info = SubscriptionInfo::default();

    if let Some(raw) = header("subscription-userinfo") {
        for field in raw.split(';') {
            let Some((name, value)) = field.split_once('=') else {
                continue;
            };
            let name = name.trim().to_lowercase();
            let value = value.trim().parse::<i64>().ok();
            match name.as_str() {
                "upload" => info.upload = value,
                "download" => info.download = value,
                "total" => info.total = value.filter(|v| *v > 0),
                "expire" => info.expire = value.filter(|v| *v > 0),
                _ => {}
            }
        }
    }

    if let Some(title) = header("profile-title") {
        info.title = Some(decode_title(&title));
    }
    info
}

/// Remnawave's `/info` JSON. Only the fields the design shows are read; the rest
/// of the document is left alone so a schema addition cannot break it.
pub fn info_from_remnawave_json(body: &str) -> Option<SubscriptionInfo> {
    let root: serde_json::Value = serde_json::from_str(body).ok()?;
    let response = root.get("response").unwrap_or(&root);
    let user = response.get("user").unwrap_or(response);

    let number = |v: &serde_json::Value, key: &str| -> Option<i64> {
        match v.get(key)? {
            serde_json::Value::Number(n) => n.as_i64(),
            serde_json::Value::String(s) => s.parse().ok(),
            _ => None,
        }
    };

    let mut info = SubscriptionInfo {
        title: user
            .get("username")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        download: number(user, "trafficUsed").or_else(|| number(user, "usedTrafficBytes")),
        ..Default::default()
    };

    if let Some(limit) = number(user, "trafficLimit")
        .or_else(|| number(user, "trafficLimitBytes"))
        .filter(|v| *v > 0)
    {
        info.total = Some(limit);
    }
    if let Some(expire) = user.get("expiresAt").and_then(|v| v.as_str()) {
        info.expire = OffsetDateTime::parse(expire, &Rfc3339)
            .ok()
            .map(|d| d.unix_timestamp());
    }
    if let Some(limit) = number(user, "hwidDeviceLimit").filter(|v| *v > 0) {
        info.device_limit = Some(limit as u32);
    }
    if let Some(used) = number(response, "devicesUsed").or_else(|| number(user, "devicesUsed")) {
        info.devices_used = Some(used as u32);
    }
    Some(info)
}

/// Fields present in `other` win, field by field — the header values are the
/// authoritative ones, and a `None` there must not erase a good value.
pub fn merging(base: &SubscriptionInfo, other: &SubscriptionInfo) -> SubscriptionInfo {
    SubscriptionInfo {
        title: other.title.clone().or_else(|| base.title.clone()),
        upload: other.upload.or(base.upload),
        download: other.download.or(base.download),
        total: other.total.or(base.total),
        expire: other.expire.or(base.expire),
        device_limit: other.device_limit.or(base.device_limit),
        devices_used: other.devices_used.or(base.devices_used),
    }
}

fn decode_title(raw: &str) -> String {
    // Panels send this either plain or as `base64:<payload>`.
    if let Some(payload) = raw
        .strip_prefix("base64:")
        .or_else(|| raw.strip_prefix("BASE64:"))
    {
        if let Ok(bytes) = STANDARD.decode(payload) {
            if let Ok(text) = String::from_utf8(bytes) {
                return text;
            }
        }
    }
    raw.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn headers(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |name: &str| map.get(name).cloned()
    }

    #[test]
    fn a_bare_host_is_upgraded_to_https() {
        assert_eq!(
            normalize("panel.example.com/sub/abc").as_deref(),
            Some("https://panel.example.com/sub/abc")
        );
    }

    #[test]
    fn an_explicit_http_scheme_is_left_alone() {
        // Cleartext only happens when the user types it themselves.
        assert_eq!(
            normalize("http://10.0.0.1:8080/sub").as_deref(),
            Some("http://10.0.0.1:8080/sub")
        );
    }

    #[test]
    fn a_non_http_scheme_is_refused_rather_than_rewritten() {
        // Prepending https:// would turn a rejection into a plausible URL, and
        // a deep link must not be able to point the import flow at a file.
        assert_eq!(normalize("file:///etc/passwd"), None);
        assert_eq!(normalize("vless://uuid@host:443#N"), None);
        assert_eq!(normalize("ftp://example.com/x"), None);
        assert_eq!(normalize("javascript://alert(1)"), None);
    }

    #[test]
    fn a_trailing_slash_is_trimmed_so_the_suffix_joins_cleanly() {
        assert_eq!(
            normalize("https://panel.example.com/sub/").as_deref(),
            Some("https://panel.example.com/sub")
        );
    }

    #[test]
    fn blank_input_is_refused() {
        assert_eq!(normalize(""), None);
        assert_eq!(normalize("   \n "), None);
    }

    #[test]
    fn surrounding_whitespace_is_tolerated() {
        assert_eq!(
            normalize("  https://panel.example.com/sub  ").as_deref(),
            Some("https://panel.example.com/sub")
        );
    }

    #[test]
    fn a_clash_config_is_detected_by_content_not_by_content_type() {
        assert!(looks_like_clash_config("proxies:\n  - name: A\n"));
        assert!(looks_like_clash_config("port: 7890\nproxies:\n  - x\n"));
        assert!(looks_like_clash_config("  proxies :\n"));
    }

    #[test]
    fn a_base64_body_is_not_mistaken_for_a_config() {
        assert!(!looks_like_clash_config("dmxlc3M6Ly9hYmNkZWY="));
        assert!(!looks_like_clash_config("vless://u@h:443#N"));
        assert!(!looks_like_clash_config(""));
        // "proxies" inside a value is not a top-level key.
        assert!(!looks_like_clash_config("note: these are proxies: yes\n"));
    }

    #[test]
    fn subscription_userinfo_is_parsed_field_by_field() {
        let info = info_from_headers(headers(&[(
            "subscription-userinfo",
            "upload=1024; download=2048; total=107374182400; expire=1893456000",
        )]));
        assert_eq!(info.upload, Some(1024));
        assert_eq!(info.download, Some(2048));
        assert_eq!(info.total, Some(107_374_182_400));
        assert_eq!(info.expire, Some(1_893_456_000));
    }

    #[test]
    fn a_zero_total_or_expire_means_unlimited_not_zero() {
        let info = info_from_headers(headers(&[(
            "subscription-userinfo",
            "upload=0; download=0; total=0; expire=0",
        )]));
        // Nil, not 0 — showing "0 GB" for an unlimited plan is a lie the user
        // acts on.
        assert_eq!(info.total, None);
        assert_eq!(info.expire, None);
        // But a genuine zero of traffic used is still zero.
        assert_eq!(info.upload, Some(0));
        assert_eq!(info.download, Some(0));
    }

    #[test]
    fn a_partial_header_leaves_the_rest_unknown() {
        let info = info_from_headers(headers(&[("subscription-userinfo", "download=500")]));
        assert_eq!(info.download, Some(500));
        assert_eq!(info.upload, None);
        assert_eq!(info.total, None);
    }

    #[test]
    fn a_malformed_header_does_not_poison_the_good_fields() {
        let info = info_from_headers(headers(&[(
            "subscription-userinfo",
            "upload; download=500; total=notanumber; ;expire=1893456000",
        )]));
        assert_eq!(info.download, Some(500));
        assert_eq!(info.total, None);
        assert_eq!(info.expire, Some(1_893_456_000));
    }

    #[test]
    fn an_absent_header_yields_an_empty_info_rather_than_zeroes() {
        let info = info_from_headers(headers(&[]));
        assert_eq!(info, SubscriptionInfo::default());
        assert_eq!(info.used(), None);
    }

    #[test]
    fn a_profile_title_is_read_plain_or_base64() {
        let plain = info_from_headers(headers(&[("profile-title", "Мой план")]));
        assert_eq!(plain.title.as_deref(), Some("Мой план"));

        let encoded = format!("base64:{}", STANDARD.encode("Мой план"));
        let decoded = info_from_headers(headers(&[("profile-title", &encoded)]));
        assert_eq!(decoded.title.as_deref(), Some("Мой план"));
    }

    #[test]
    fn an_undecodable_base64_title_falls_back_to_the_raw_string() {
        let info = info_from_headers(headers(&[("profile-title", "base64:!!!not base64!!!")]));
        assert_eq!(info.title.as_deref(), Some("base64:!!!not base64!!!"));
    }

    #[test]
    fn remnawave_info_json_is_read_through_its_wrapper() {
        let body = r#"{
            "response": {
                "user": {
                    "username": "alice",
                    "trafficUsed": 1024,
                    "trafficLimit": 107374182400,
                    "expiresAt": "2027-01-01T00:00:00.000Z",
                    "hwidDeviceLimit": 3
                },
                "devicesUsed": 1
            }
        }"#;
        let info = info_from_remnawave_json(body).expect("parses");
        assert_eq!(info.title.as_deref(), Some("alice"));
        assert_eq!(info.download, Some(1024));
        assert_eq!(info.total, Some(107_374_182_400));
        assert_eq!(info.device_limit, Some(3));
        assert_eq!(info.devices_used, Some(1));
        assert!(info.expire.is_some());
    }

    #[test]
    fn remnawave_info_also_parses_without_the_wrapper() {
        let body = r#"{"username":"bob","usedTrafficBytes":50,"trafficLimitBytes":100}"#;
        let info = info_from_remnawave_json(body).expect("parses");
        assert_eq!(info.title.as_deref(), Some("bob"));
        assert_eq!(info.download, Some(50));
        assert_eq!(info.total, Some(100));
    }

    #[test]
    fn a_zero_traffic_limit_in_json_is_unlimited_too() {
        let body = r#"{"username":"c","trafficLimit":0,"hwidDeviceLimit":0}"#;
        let info = info_from_remnawave_json(body).expect("parses");
        assert_eq!(info.total, None);
        assert_eq!(info.device_limit, None);
    }

    #[test]
    fn unknown_json_fields_are_ignored_rather_than_fatal() {
        let body = r#"{"response":{"user":{"username":"d","somethingNew":{"a":1}}}}"#;
        let info = info_from_remnawave_json(body).expect("parses");
        assert_eq!(info.title.as_deref(), Some("d"));
    }

    #[test]
    fn junk_json_is_refused() {
        assert!(info_from_remnawave_json("not json").is_none());
    }

    #[test]
    fn headers_win_field_by_field_over_the_info_document() {
        let from_json = SubscriptionInfo {
            title: Some("json".into()),
            download: Some(1),
            total: Some(100),
            devices_used: Some(2),
            ..Default::default()
        };
        let from_headers = SubscriptionInfo {
            download: Some(999),
            ..Default::default()
        };

        let merged = merging(&from_json, &from_headers);
        assert_eq!(merged.download, Some(999), "the header value wins");
        // A nil in the headers must not erase what /info supplied.
        assert_eq!(merged.title.as_deref(), Some("json"));
        assert_eq!(merged.total, Some(100));
        assert_eq!(merged.devices_used, Some(2));
    }

    #[test]
    fn merging_an_empty_info_changes_nothing() {
        let base = SubscriptionInfo {
            title: Some("x".into()),
            total: Some(5),
            ..Default::default()
        };
        assert_eq!(merging(&base, &SubscriptionInfo::default()), base);
    }
}
