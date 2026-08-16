//! Converts `vless://`, `vmess://`, `trojan://` and `ss://` share links into
//! mihomo proxy entries.
//!
//! This is the **fallback** path, used only when a panel serves no Clash config
//! at all (see [`crate::subscription`]). The format itself throws information
//! away — proxy groups, `url-test` balancers, per-node routing and DNS all
//! flatten to one URI per node — so a config built from here is strictly poorer
//! than one the panel wrote. It is still better than nothing, and for the common
//! case of a flat list of VLESS Reality nodes it is lossless.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use percent_encoding::percent_decode_str;
use serde_yaml::{Mapping, Value};

const SCHEMES: [&str; 4] = ["vless://", "vmess://", "trojan://", "ss://"];

/// Splits a subscription body into individual links.
///
/// The body is detected by content rather than by `Content-Type`, because
/// panels mislabel it. Three shapes are accepted: base64 (standard or URL-safe,
/// padded or not), plain newline-separated links, and a body that is base64 of
/// newline-separated links.
pub fn decode_list(body: &str) -> Vec<String> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let direct = split(trimmed);
    if !direct.is_empty() {
        return direct;
    }

    match decode_base64(trimmed) {
        Some(decoded) => split(&decoded),
        None => Vec::new(),
    }
}

fn split(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| {
            let lowered = line.to_lowercase();
            SCHEMES.iter().any(|s| lowered.starts_with(s))
        })
        .map(str::to_string)
        .collect()
}

pub fn decode_base64(text: &str) -> Option<String> {
    let mut payload: String = text
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| match c {
            '-' => '+',
            '_' => '/',
            other => other,
        })
        .collect();
    // Restore padding the panel may have stripped.
    let remainder = payload.len() % 4;
    if remainder > 0 {
        payload.push_str(&"=".repeat(4 - remainder));
    }
    let data = STANDARD.decode(payload).ok()?;
    String::from_utf8(data).ok()
}

/// A single link as a mihomo proxy mapping, or `None` if the scheme or its
/// parameters are ones mihomo cannot express.
pub fn mihomo_proxy(link: &str) -> Option<Mapping> {
    let scheme = link.split(':').next()?.to_lowercase();
    match scheme.as_str() {
        "vless" => vless(link),
        "vmess" => vmess(link),
        "trojan" => trojan(link),
        "ss" => shadowsocks(link),
        _ => None,
    }
}

fn put(map: &mut Mapping, key: &str, value: impl Into<Value>) {
    map.insert(Value::from(key), value.into());
}

// vless

fn vless(link: &str) -> Option<Mapping> {
    let parts = UriParts::parse(link)?;
    let uuid = parts.user.as_ref()?;

    let mut proxy = Mapping::new();
    put(&mut proxy, "name", parts.name.clone());
    put(&mut proxy, "type", "vless");
    put(&mut proxy, "server", parts.host.clone());
    put(&mut proxy, "port", parts.port);
    put(&mut proxy, "uuid", uuid.clone());
    put(&mut proxy, "udp", true);

    let q = &parts.query;

    if let Some(flow) = q.get("flow").filter(|v| !v.is_empty()) {
        put(&mut proxy, "flow", flow.clone());
    }
    if let Some(fp) = q.get("fp").filter(|v| !v.is_empty()) {
        put(&mut proxy, "client-fingerprint", fp.clone());
    }

    let security = q
        .get("security")
        .map(|s| s.to_lowercase())
        .unwrap_or_else(|| "none".to_string());
    let mut servername: Option<String> = None;
    if security == "tls" || security == "reality" {
        put(&mut proxy, "tls", true);
        if let Some(sni) = q
            .get("sni")
            .or_else(|| q.get("peer"))
            .filter(|v| !v.is_empty())
        {
            servername = Some(sni.clone());
            put(&mut proxy, "servername", sni.clone());
        }
        if let Some(alpn) = q.get("alpn").filter(|v| !v.is_empty()) {
            let list: Vec<Value> = alpn.split(',').map(Value::from).collect();
            put(&mut proxy, "alpn", list);
        }
    }
    if security == "reality" {
        // Reality without a public key is unusable — mihomo will refuse the
        // config rather than fall back to plain TLS, so drop the node here
        // where the reason is still visible.
        let public_key = q.get("pbk").filter(|v| !v.is_empty())?;
        let mut reality = Mapping::new();
        put(&mut reality, "public-key", public_key.clone());
        if let Some(short_id) = q.get("sid").filter(|v| !v.is_empty()) {
            put(&mut reality, "short-id", short_id.clone());
        }
        put(&mut proxy, "reality-opts", reality);
        // Reality assumes a browser fingerprint; mihomo errors without one.
        if !proxy.contains_key(Value::from("client-fingerprint")) {
            put(&mut proxy, "client-fingerprint", "chrome");
        }
    }
    if q.get("allowInsecure").map(String::as_str) == Some("1") {
        put(&mut proxy, "skip-cert-verify", true);
    }

    apply_transport(&mut proxy, q, servername.as_deref());
    Some(proxy)
}

// vmess

fn vmess(link: &str) -> Option<Mapping> {
    // vmess is the odd one out: the whole payload is base64 JSON rather than a
    // URI with a query string.
    let payload = link.get("vmess://".len()..)?;
    let json = decode_base64(payload)?;
    let object: serde_json::Value = serde_json::from_str(&json).ok()?;
    let object = object.as_object()?;

    let host = json_string(object.get("add")).filter(|s| !s.is_empty())?;
    let uuid = json_string(object.get("id")).filter(|s| !s.is_empty())?;
    let port = json_int(object.get("port"))?;

    let mut proxy = Mapping::new();
    put(
        &mut proxy,
        "name",
        json_string(object.get("ps")).unwrap_or_else(|| format!("{host}:{port}")),
    );
    put(&mut proxy, "type", "vmess");
    put(&mut proxy, "server", host.clone());
    put(&mut proxy, "port", port);
    put(&mut proxy, "uuid", uuid);
    put(
        &mut proxy,
        "alterId",
        json_int(object.get("aid")).unwrap_or(0),
    );
    put(
        &mut proxy,
        "cipher",
        json_string(object.get("scy")).unwrap_or_else(|| "auto".to_string()),
    );
    put(&mut proxy, "udp", true);

    let mut servername = None;
    if json_string(object.get("tls"))
        .map(|s| s.to_lowercase())
        .as_deref()
        == Some("tls")
    {
        put(&mut proxy, "tls", true);
        if let Some(sni) = json_string(object.get("sni")).filter(|s| !s.is_empty()) {
            servername = Some(sni.clone());
            put(&mut proxy, "servername", sni);
        }
    }

    let mut query = std::collections::HashMap::new();
    for (from, to) in [
        ("net", "type"),
        ("path", "path"),
        ("host", "host"),
        ("path", "serviceName"),
    ] {
        if let Some(v) = json_string(object.get(from)) {
            query.insert(to.to_string(), v);
        }
    }
    apply_transport(&mut proxy, &query, servername.as_deref());
    Some(proxy)
}

// trojan

fn trojan(link: &str) -> Option<Mapping> {
    let parts = UriParts::parse(link)?;
    let password = parts.user.as_ref()?;

    let mut proxy = Mapping::new();
    put(&mut proxy, "name", parts.name.clone());
    put(&mut proxy, "type", "trojan");
    put(&mut proxy, "server", parts.host.clone());
    put(&mut proxy, "port", parts.port);
    put(&mut proxy, "password", password.clone());
    put(&mut proxy, "udp", true);

    let q = &parts.query;
    let mut sni_value = None;
    if let Some(sni) = q
        .get("sni")
        .or_else(|| q.get("peer"))
        .filter(|v| !v.is_empty())
    {
        sni_value = Some(sni.clone());
        put(&mut proxy, "sni", sni.clone());
    }
    if let Some(alpn) = q.get("alpn").filter(|v| !v.is_empty()) {
        let list: Vec<Value> = alpn.split(',').map(Value::from).collect();
        put(&mut proxy, "alpn", list);
    }
    if q.get("allowInsecure").map(String::as_str) == Some("1") {
        put(&mut proxy, "skip-cert-verify", true);
    }
    apply_transport(&mut proxy, q, sni_value.as_deref());
    Some(proxy)
}

// shadowsocks

fn shadowsocks(link: &str) -> Option<Mapping> {
    let parts = UriParts::parse(link)?;

    // Two encodings are in the wild: `ss://base64(method:password)@host:port`
    // and `ss://method:password@host:port` with the userinfo percent-encoded.
    let user = parts.user.as_ref()?;
    let (method, password) = match decode_base64(user) {
        Some(decoded) if decoded.contains(':') => {
            let (m, p) = decoded.split_once(':')?;
            (m.to_string(), p.to_string())
        }
        _ if user.contains(':') => {
            let (m, p) = user.split_once(':')?;
            (m.to_string(), p.to_string())
        }
        _ => return None,
    };

    let mut proxy = Mapping::new();
    put(&mut proxy, "name", parts.name.clone());
    put(&mut proxy, "type", "ss");
    put(&mut proxy, "server", parts.host.clone());
    put(&mut proxy, "port", parts.port);
    put(&mut proxy, "cipher", method);
    put(&mut proxy, "password", password);
    put(&mut proxy, "udp", true);
    Some(proxy)
}

/// Transport is shared across vless/vmess/trojan and is where most malformed
/// links go wrong, so it lives in one place.
fn apply_transport(
    proxy: &mut Mapping,
    query: &std::collections::HashMap<String, String>,
    servername: Option<&str>,
) {
    let network = query
        .get("type")
        .map(|s| s.to_lowercase())
        .unwrap_or_else(|| "tcp".to_string());

    match network.as_str() {
        "ws" => {
            put(proxy, "network", "ws");
            let mut options = Mapping::new();
            if let Some(path) = query.get("path").filter(|v| !v.is_empty()) {
                put(&mut options, "path", path.clone());
            }
            // A ws Host header defaults to the SNI, not to the dial address —
            // sending the raw IP here is what breaks CDN-fronted nodes.
            let ws_host = query
                .get("host")
                .map(String::as_str)
                .filter(|v| !v.is_empty())
                .or(servername);
            if let Some(ws_host) = ws_host.filter(|v| !v.is_empty()) {
                let mut headers = Mapping::new();
                put(&mut headers, "Host", ws_host);
                put(&mut options, "headers", headers);
            }
            if !options.is_empty() {
                put(proxy, "ws-opts", options);
            }
        }
        "grpc" => {
            put(proxy, "network", "grpc");
            if let Some(service) = query.get("serviceName").filter(|v| !v.is_empty()) {
                let mut options = Mapping::new();
                put(&mut options, "grpc-service-name", service.clone());
                put(proxy, "grpc-opts", options);
            }
        }
        "http" | "h2" => {
            put(proxy, "network", "h2");
            let mut options = Mapping::new();
            if let Some(path) = query.get("path").filter(|v| !v.is_empty()) {
                put(&mut options, "path", path.clone());
            }
            if let Some(http_host) = query.get("host").filter(|v| !v.is_empty()) {
                let list: Vec<Value> = http_host.split(',').map(Value::from).collect();
                put(&mut options, "host", list);
            }
            if !options.is_empty() {
                put(proxy, "h2-opts", options);
            }
        }
        _ => {
            put(proxy, "network", "tcp");
        }
    }
}

fn json_string(value: Option<&serde_json::Value>) -> Option<String> {
    match value? {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn json_int(value: Option<&serde_json::Value>) -> Option<i64> {
    match value? {
        serde_json::Value::Number(n) => n.as_i64(),
        serde_json::Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

fn percent_decode(text: &str) -> String {
    percent_decode_str(text)
        .decode_utf8()
        .map(|c| c.into_owned())
        .unwrap_or_else(|_| text.to_string())
}

/// The pieces of a `scheme://user@host:port?query#fragment` share link.
///
/// A URL parser is not used: a share link's fragment is the node name and is
/// routinely unescaped UTF-8 with emoji and spaces, which makes the whole URL
/// fail to parse rather than just that component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UriParts {
    pub user: Option<String>,
    pub host: String,
    pub port: i64,
    pub query: std::collections::HashMap<String, String>,
    pub name: String,
}

impl UriParts {
    pub fn parse(link: &str) -> Option<Self> {
        let scheme_end = link.find("://")?;
        let mut rest = &link[scheme_end + 3..];

        let mut fragment = "";
        if let Some(hash) = rest.find('#') {
            fragment = &rest[hash + 1..];
            rest = &rest[..hash];
        }

        let mut query_string = "";
        if let Some(mark) = rest.find('?') {
            query_string = &rest[mark + 1..];
            rest = &rest[..mark];
        }

        // The userinfo separator is the *last* `@`: a password may contain one.
        let (user, authority) = match rest.rfind('@') {
            Some(at) => (Some(percent_decode(&rest[..at])), &rest[at + 1..]),
            None => (None, rest),
        };

        // IPv6 literals are bracketed, so the port is after the `]` — or after
        // the only colon for a hostname.
        let (host_part, port_part) = if authority.starts_with('[') {
            let close = authority.find(']')?;
            let after = &authority[close + 1..];
            (&authority[1..close], after.strip_prefix(':'))
        } else if let Some(colon) = authority.rfind(':') {
            (&authority[..colon], Some(&authority[colon + 1..]))
        } else {
            (authority, None)
        };

        if host_part.is_empty() {
            return None;
        }
        let port: i64 = port_part?.parse().ok()?;
        if !(1..=65535).contains(&port) {
            return None;
        }

        let mut query = std::collections::HashMap::new();
        for pair in query_string.split('&').filter(|p| !p.is_empty()) {
            let (key, raw) = match pair.split_once('=') {
                Some((k, v)) => (k, v),
                None => (pair, ""),
            };
            query.insert(key.to_string(), percent_decode(raw));
        }

        let decoded = percent_decode(fragment);
        let name = if decoded.is_empty() {
            format!("{host_part}:{port}")
        } else {
            decoded
        };

        Some(UriParts {
            user,
            host: host_part.to_string(),
            port,
            query,
            name,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(map: &Mapping, key: &str) -> Option<String> {
        map.get(Value::from(key))?.as_str().map(str::to_string)
    }

    #[test]
    fn a_plain_list_is_split_by_line() {
        let body = "vless://a@h:443#One\nss://x@h:8388#Two\nnot-a-link";
        assert_eq!(decode_list(body).len(), 2);
    }

    #[test]
    fn a_base64_body_is_detected_by_content() {
        let inner = "vless://a@h:443#One\ntrojan://p@h:443#Two";
        let encoded = STANDARD.encode(inner);
        assert_eq!(decode_list(&encoded).len(), 2);
    }

    #[test]
    fn url_safe_base64_without_padding_decodes() {
        let inner = "vless://a@h:443#Nod\u{435}"; // non-ASCII forces padding
        let encoded = STANDARD
            .encode(inner)
            .replace('+', "-")
            .replace('/', "_")
            .trim_end_matches('=')
            .to_string();
        assert_eq!(decode_list(&encoded).len(), 1);
    }

    #[test]
    fn an_empty_or_junk_body_yields_nothing() {
        assert!(decode_list("").is_empty());
        assert!(decode_list("   \n  ").is_empty());
        assert!(decode_list("hello there").is_empty());
    }

    #[test]
    fn the_fragment_is_the_node_name_even_with_emoji_and_spaces() {
        let parts =
            UriParts::parse("vless://uuid@1.2.3.4:443#%F0%9F%87%B8%F0%9F%87%AA%20Stockholm")
                .expect("parses");
        assert_eq!(parts.name, "🇸🇪 Stockholm");
    }

    #[test]
    fn a_raw_unescaped_fragment_is_kept_verbatim() {
        // Panels routinely emit this, and a URL parser rejects the whole link.
        let parts = UriParts::parse("vless://uuid@1.2.3.4:443#🇸🇪 Stockholm 01").expect("parses");
        assert_eq!(parts.name, "🇸🇪 Stockholm 01");
    }

    #[test]
    fn a_missing_fragment_falls_back_to_host_and_port() {
        let parts = UriParts::parse("vless://uuid@1.2.3.4:443").expect("parses");
        assert_eq!(parts.name, "1.2.3.4:443");
    }

    #[test]
    fn the_userinfo_separator_is_the_last_at_sign() {
        let parts = UriParts::parse("trojan://pass@word@host.example:443").expect("parses");
        assert_eq!(parts.user.as_deref(), Some("pass@word"));
        assert_eq!(parts.host, "host.example");
    }

    #[test]
    fn ipv6_literals_keep_their_port() {
        let parts = UriParts::parse("vless://uuid@[2001:db8::1]:8443#N").expect("parses");
        assert_eq!(parts.host, "2001:db8::1");
        assert_eq!(parts.port, 8443);
    }

    #[test]
    fn a_link_with_no_port_is_refused() {
        assert!(UriParts::parse("vless://uuid@host.example").is_none());
    }

    #[test]
    fn an_out_of_range_port_is_refused() {
        assert!(UriParts::parse("vless://uuid@host:0").is_none());
        assert!(UriParts::parse("vless://uuid@host:70000").is_none());
    }

    #[test]
    fn vless_reality_carries_its_public_key_and_a_fingerprint() {
        let proxy = mihomo_proxy(
            "vless://uuid@1.2.3.4:443?security=reality&pbk=KEY&sid=ab&sni=example.com&flow=xtls-rprx-vision#N",
        )
        .expect("parses");

        assert_eq!(s(&proxy, "type").as_deref(), Some("vless"));
        assert_eq!(s(&proxy, "servername").as_deref(), Some("example.com"));
        assert_eq!(s(&proxy, "flow").as_deref(), Some("xtls-rprx-vision"));
        // Reality errors without a fingerprint, so one is supplied.
        assert_eq!(s(&proxy, "client-fingerprint").as_deref(), Some("chrome"));

        let reality = proxy
            .get(Value::from("reality-opts"))
            .and_then(Value::as_mapping)
            .expect("reality-opts");
        assert_eq!(s(reality, "public-key").as_deref(), Some("KEY"));
        assert_eq!(s(reality, "short-id").as_deref(), Some("ab"));
    }

    #[test]
    fn a_reality_node_with_no_public_key_is_dropped() {
        // mihomo refuses the whole config rather than skipping the node, so it
        // has to be dropped here.
        assert!(mihomo_proxy("vless://uuid@1.2.3.4:443?security=reality&sni=e.com#N").is_none());
    }

    #[test]
    fn an_explicit_fingerprint_is_not_overwritten() {
        let proxy =
            mihomo_proxy("vless://u@h:443?security=reality&pbk=K&fp=firefox#N").expect("parses");
        assert_eq!(s(&proxy, "client-fingerprint").as_deref(), Some("firefox"));
    }

    #[test]
    fn a_websocket_host_header_defaults_to_the_sni_not_the_dial_address() {
        let proxy = mihomo_proxy(
            "vless://u@1.2.3.4:443?security=tls&sni=cdn.example.com&type=ws&path=%2Fws#N",
        )
        .expect("parses");

        let opts = proxy
            .get(Value::from("ws-opts"))
            .and_then(Value::as_mapping)
            .expect("ws-opts");
        assert_eq!(s(opts, "path").as_deref(), Some("/ws"));
        let headers = opts
            .get(Value::from("headers"))
            .and_then(Value::as_mapping)
            .expect("headers");
        assert_eq!(s(headers, "Host").as_deref(), Some("cdn.example.com"));
    }

    #[test]
    fn an_explicit_ws_host_wins_over_the_sni() {
        let proxy =
            mihomo_proxy("vless://u@1.2.3.4:443?security=tls&sni=a.com&type=ws&host=b.com#N")
                .expect("parses");
        let headers = proxy
            .get(Value::from("ws-opts"))
            .and_then(Value::as_mapping)
            .and_then(|m| m.get(Value::from("headers")))
            .and_then(Value::as_mapping)
            .expect("headers");
        assert_eq!(s(headers, "Host").as_deref(), Some("b.com"));
    }

    #[test]
    fn grpc_carries_its_service_name() {
        let proxy = mihomo_proxy("vless://u@h:443?type=grpc&serviceName=gun#N").expect("parses");
        assert_eq!(s(&proxy, "network").as_deref(), Some("grpc"));
        let opts = proxy
            .get(Value::from("grpc-opts"))
            .and_then(Value::as_mapping)
            .expect("grpc-opts");
        assert_eq!(s(opts, "grpc-service-name").as_deref(), Some("gun"));
    }

    #[test]
    fn an_unknown_transport_falls_back_to_tcp() {
        let proxy = mihomo_proxy("vless://u@h:443?type=quic#N").expect("parses");
        assert_eq!(s(&proxy, "network").as_deref(), Some("tcp"));
    }

    #[test]
    fn vmess_is_base64_json_rather_than_a_uri() {
        let json = r#"{"add":"1.2.3.4","id":"uuid-here","port":"443","ps":"Node A","net":"ws","path":"/p","host":"h.com","tls":"tls","sni":"s.com","aid":0}"#;
        let link = format!("vmess://{}", STANDARD.encode(json));
        let proxy = mihomo_proxy(&link).expect("parses");

        assert_eq!(s(&proxy, "type").as_deref(), Some("vmess"));
        assert_eq!(s(&proxy, "name").as_deref(), Some("Node A"));
        assert_eq!(s(&proxy, "server").as_deref(), Some("1.2.3.4"));
        assert_eq!(
            proxy.get(Value::from("port")).and_then(Value::as_i64),
            Some(443)
        );
        assert_eq!(s(&proxy, "network").as_deref(), Some("ws"));
    }

    #[test]
    fn vmess_port_survives_being_a_string_or_a_number() {
        for port in ["443", "\"443\""] {
            let json = format!(r#"{{"add":"h","id":"u","port":{port}}}"#);
            let link = format!("vmess://{}", STANDARD.encode(&json));
            let proxy = mihomo_proxy(&link).expect("parses");
            assert_eq!(
                proxy.get(Value::from("port")).and_then(Value::as_i64),
                Some(443)
            );
        }
    }

    #[test]
    fn a_vmess_with_no_name_falls_back_to_host_and_port() {
        let json = r#"{"add":"1.2.3.4","id":"u","port":443}"#;
        let link = format!("vmess://{}", STANDARD.encode(json));
        let proxy = mihomo_proxy(&link).expect("parses");
        assert_eq!(s(&proxy, "name").as_deref(), Some("1.2.3.4:443"));
    }

    #[test]
    fn trojan_carries_its_password_and_sni() {
        let proxy =
            mihomo_proxy("trojan://secret@h.example:443?sni=s.example&alpn=h2,http/1.1#Node")
                .expect("parses");
        assert_eq!(s(&proxy, "type").as_deref(), Some("trojan"));
        assert_eq!(s(&proxy, "password").as_deref(), Some("secret"));
        assert_eq!(s(&proxy, "sni").as_deref(), Some("s.example"));
        let alpn = proxy
            .get(Value::from("alpn"))
            .and_then(Value::as_sequence)
            .expect("alpn");
        assert_eq!(alpn.len(), 2);
    }

    #[test]
    fn shadowsocks_accepts_both_userinfo_encodings() {
        let plain = mihomo_proxy("ss://aes-256-gcm:pass@h:8388#N").expect("plain parses");
        assert_eq!(s(&plain, "cipher").as_deref(), Some("aes-256-gcm"));
        assert_eq!(s(&plain, "password").as_deref(), Some("pass"));

        let encoded = format!("ss://{}@h:8388#N", STANDARD.encode("aes-256-gcm:pass"));
        let decoded = mihomo_proxy(&encoded).expect("base64 parses");
        assert_eq!(s(&decoded, "cipher").as_deref(), Some("aes-256-gcm"));
        assert_eq!(s(&decoded, "password").as_deref(), Some("pass"));
    }

    #[test]
    fn a_shadowsocks_link_with_no_method_is_dropped() {
        assert!(mihomo_proxy("ss://justpassword@h:8388#N").is_none());
    }

    #[test]
    fn an_unknown_scheme_is_refused() {
        assert!(mihomo_proxy("hysteria2://x@h:443#N").is_none());
        assert!(mihomo_proxy("https://example.com").is_none());
    }

    #[test]
    fn allow_insecure_becomes_skip_cert_verify() {
        let proxy = mihomo_proxy("trojan://p@h:443?allowInsecure=1#N").expect("parses");
        assert_eq!(
            proxy
                .get(Value::from("skip-cert-verify"))
                .and_then(Value::as_bool),
            Some(true)
        );
    }

    #[test]
    fn every_proxy_enables_udp() {
        for link in [
            "vless://u@h:443#N",
            "trojan://p@h:443#N",
            "ss://aes-256-gcm:p@h:8388#N",
        ] {
            let proxy = mihomo_proxy(link).expect(link);
            assert_eq!(
                proxy.get(Value::from("udp")).and_then(Value::as_bool),
                Some(true),
                "{link} did not enable udp"
            );
        }
    }
}
