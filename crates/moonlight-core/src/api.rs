//! Client for mihomo's RESTful API — the only channel the app uses to observe
//! or steer a running core.
//!
//! The core is never reconfigured by restarting it: switching a node, changing
//! mode, or reloading a new subscription all go through here, so the tunnel
//! stays up across every one of them.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use serde_json::Value;
use thiserror::Error;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tokio::sync::mpsc;

use crate::models::Node;

/// The target every latency measurement is made against — both the `Пинг`
/// button's probes and the `url-test` group this client injects.
///
/// Cloudflare's captive-portal endpoint over **http**, not https: the probe is
/// timing the path to the node, and a TLS handshake to the target adds a round
/// trip that has nothing to do with it. It answers `204` with an empty body
/// from a global anycast address, so the number is about the node rather than
/// about which continent the target happens to be on.
pub const PROBE_URL: &str = "http://cp.cloudflare.com/generate_204";

/// Group types the core reports. A member of one of these is a choice the panel
/// operator built, not a raw node.
const GROUP_TYPES: [&str; 5] = ["selector", "urltest", "fallback", "loadbalance", "relay"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Traffic {
    pub up: i64,
    pub down: i64,
}

#[derive(Debug, Clone)]
pub struct ProxyGroup {
    pub name: String,
    pub kind: String,
    pub now: Option<String>,
    pub options: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct LogLine {
    pub kind: String,
    pub payload: String,
}

#[derive(Debug, Clone)]
pub struct Connection {
    pub id: String,
    /// The proxy chain the core picked, outermost last — the group that chose,
    /// then what it chose. The UI shows the node that carried it.
    pub chains: Vec<String>,
    pub rule: String,
    pub rule_payload: String,
    pub network: String,
    pub host: String,
    pub process: String,
    pub process_path: String,
    pub upload: i64,
    pub download: i64,
    pub start: OffsetDateTime,
}

impl Connection {
    /// The node that actually carried the connection: the innermost link.
    pub fn node(&self) -> &str {
        self.chains.first().map(String::as_str).unwrap_or("")
    }
}

#[derive(Debug, Error)]
pub enum Failure {
    #[error("Core API returned {status}{}", if .body.is_empty() { String::new() } else { format!(": {}", .body) })]
    Http { status: u16, body: String },
    #[error("Core is not running")]
    NotRunning,
}

#[derive(Clone)]
pub struct MihomoApi {
    base: String,
    secret: String,
    http: reqwest::Client,
}

impl MihomoApi {
    pub fn new(port: u16, secret: impl Into<String>) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            // The core is on loopback; a proxy setting the app itself installed
            // must not be applied to it, or switching a node would race the
            // tunnel.
            .no_proxy()
            .build()
            .expect("a loopback client with no TLS cannot fail to build");

        MihomoApi {
            base: format!("http://127.0.0.1:{port}"),
            secret: secret.into(),
            http,
        }
    }

    /// Polls until the core answers, or gives up. Called right after spawn:
    /// mihomo binds its controller after loading geodata, which on a cold start
    /// includes downloading it.
    pub async fn wait_until_ready(&self, timeout: Duration) -> bool {
        let deadline = tokio::time::Instant::now() + timeout;
        while tokio::time::Instant::now() < deadline {
            if self.version().await.is_ok() {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        false
    }

    pub async fn version(&self) -> Result<String, Failure> {
        let object = self.get("/version").await?;
        Ok(object
            .get("version")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string())
    }

    // Proxies

    pub async fn groups(&self) -> Result<Vec<ProxyGroup>, Failure> {
        let object = self.get("/proxies").await?;
        let Some(proxies) = object.get("proxies").and_then(Value::as_object) else {
            return Ok(Vec::new());
        };

        Ok(proxies
            .values()
            .filter_map(|entry| {
                Some(ProxyGroup {
                    name: entry.get("name")?.as_str()?.to_string(),
                    kind: entry.get("type")?.as_str()?.to_string(),
                    now: entry.get("now").and_then(Value::as_str).map(str::to_string),
                    options: entry
                        .get("all")?
                        .as_array()?
                        .iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect(),
                })
            })
            .collect())
    }

    /// Everything the selector offers, in the order it lists them.
    ///
    /// Groups are **kept**. A panel template routinely puts a `url-test`
    /// auto-picker and a set of `load-balance` groups in its selector — those
    /// are the choices its operator built, and the raw nodes underneath them are
    /// implementation detail. Filtering them out left the user picking from
    /// twenty nodes the panel never meant to offer directly, with the balancers
    /// nowhere to be seen.
    pub async fn nodes(&self, group: &str) -> Result<Vec<Node>, Failure> {
        let object = self.get("/proxies").await?;
        let Some(proxies) = object.get("proxies").and_then(Value::as_object) else {
            return Ok(Vec::new());
        };
        let Some(names) = proxies
            .get(group)
            .and_then(|s| s.get("all"))
            .and_then(Value::as_array)
        else {
            return Ok(Vec::new());
        };

        Ok(names
            .iter()
            .filter_map(Value::as_str)
            .filter_map(|name| {
                let entry = proxies.get(name)?;
                let kind = entry.get("type")?.as_str()?.to_string();
                Some(Node {
                    name: name.to_string(),
                    latency: last_history_delay(entry),
                    is_group: GROUP_TYPES.contains(&kind.to_lowercase().as_str()),
                    kind,
                    server: None,
                    protocol_label: None,
                })
            })
            .collect())
    }

    /// Points `group` at `node`. This is the whole of "pick a server".
    pub async fn select(&self, node: &str, group: &str) -> Result<(), Failure> {
        let mut body = serde_json::Map::new();
        body.insert("name".into(), Value::from(node));
        self.request(
            reqwest::Method::PUT,
            &format!("/proxies/{}", escape(group)),
            Some(Value::Object(body)),
        )
        .await?;
        Ok(())
    }

    /// Measures one node through the core.
    ///
    /// Returns `None` rather than an error for an unreachable node: a timeout is
    /// the expected answer for a node that is down, not a failure of the probe
    /// the caller should surface.
    pub async fn delay(&self, node: &str, timeout_ms: u32) -> Option<u32> {
        let url = format!(
            "{}/proxies/{}/delay?url={}&timeout={timeout_ms}",
            self.base,
            escape(node),
            escape(PROBE_URL)
        );
        let response = self
            .http
            .get(&url)
            .timeout(Duration::from_millis(timeout_ms as u64 + 3_000))
            .header("Authorization", format!("Bearer {}", self.secret))
            .send()
            .await
            .ok()?;
        if response.status() != reqwest::StatusCode::OK {
            return None;
        }
        let object: Value = response.json().await.ok()?;
        let delay = object.get("delay")?.as_u64()? as u32;
        (delay > 0).then_some(delay)
    }

    /// Probes every node concurrently.
    ///
    /// The core multiplexes these itself — each probe is an independent
    /// connection through that node's own outbound — so a full pass costs about
    /// as long as its slowest node rather than the sum. Concurrency is still
    /// capped, because a subscription with sixty nodes would otherwise open
    /// sixty handshakes at once and measure congestion instead of latency.
    ///
    /// Results are delivered on `sink` **as each node answers**, not at the end
    /// of the pass. A pass over twenty nodes takes several seconds no matter how
    /// it is written — the dead ones have to time out. Reporting each result as
    /// it lands is what makes it feel immediate: the fast nodes, which are the
    /// ones being chosen between, appear straight away instead of behind the
    /// slowest entry in the list.
    pub async fn delays(
        self: &Arc<Self>,
        nodes: Vec<String>,
        concurrency: usize,
        sink: mpsc::UnboundedSender<(String, Option<u32>)>,
    ) -> HashMap<String, u32> {
        let results = futures_util::stream::iter(nodes.into_iter().map(|node| {
            let api = Arc::clone(self);
            let sink = sink.clone();
            async move {
                let delay = api.delay(&node, 3_000).await;
                // Ignore a closed receiver: the user changing screens mid-pass
                // drops it, and that is not a reason to stop measuring.
                let _ = sink.send((node.clone(), delay));
                (node, delay)
            }
        }))
        .buffer_unordered(concurrency.max(1))
        .collect::<Vec<_>>()
        .await;

        results
            .into_iter()
            .filter_map(|(node, delay)| delay.map(|d| (node, d)))
            .collect()
    }

    // Config

    /// Live-patches the running core. Used for the mode switch, so toggling TUN
    /// does not drop the tunnel.
    pub async fn patch_config(&self, patch: Value) -> Result<(), Failure> {
        self.request(reqwest::Method::PATCH, "/configs", Some(patch))
            .await?;
        Ok(())
    }

    /// Reloads from a config file on disk, replacing proxies and rules in place.
    pub async fn reload(&self, path: &str) -> Result<(), Failure> {
        let mut body = serde_json::Map::new();
        body.insert("path".into(), Value::from(path));
        self.request(
            reqwest::Method::PUT,
            "/configs?force=true",
            Some(Value::Object(body)),
        )
        .await?;
        Ok(())
    }

    // Streams

    /// Cumulative up/down counters, one sample per second.
    ///
    /// mihomo streams these as newline-delimited JSON on a connection it never
    /// closes, so this hands back a channel rather than returning a body: the
    /// caller stops by dropping the receiver.
    pub fn traffic_stream(&self) -> mpsc::UnboundedReceiver<Traffic> {
        self.line_stream("/traffic", |object| {
            Some(Traffic {
                up: object.get("up")?.as_i64()?,
                down: object.get("down")?.as_i64()?,
            })
        })
    }

    /// The core's own log, as it writes it.
    pub fn log_stream(&self, level: &str) -> mpsc::UnboundedReceiver<LogLine> {
        self.line_stream(&format!("/logs?level={level}"), |object| {
            Some(LogLine {
                kind: object
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or("info")
                    .to_string(),
                payload: object.get("payload")?.as_str()?.to_string(),
            })
        })
    }

    fn line_stream<T, F>(&self, path: &str, decode: F) -> mpsc::UnboundedReceiver<T>
    where
        T: Send + 'static,
        F: Fn(&Value) -> Option<T> + Send + 'static,
    {
        let (tx, rx) = mpsc::unbounded_channel();
        // Concatenated rather than joined as a path: joining escapes the `?` of
        // a query string into `%3F`, so `/logs?level=info` asks for a path
        // called "logs?level=info" and the stream never opens.
        let url = format!("{}{path}", self.base);
        let secret = self.secret.clone();
        let http = self.http.clone();

        tokio::spawn(async move {
            let response = match http
                .get(&url)
                // These connections are open for the life of the core; the
                // client-wide 15s timeout would cut them every 15 seconds.
                .timeout(Duration::from_secs(60 * 60 * 24))
                .header("Authorization", format!("Bearer {secret}"))
                .send()
                .await
            {
                Ok(response) => response,
                // The core going away ends the stream; the supervisor is what
                // notices and reports it, not this.
                Err(_) => return,
            };

            let mut stream = response.bytes_stream();
            let mut buffer = Vec::new();
            while let Some(Ok(chunk)) = stream.next().await {
                buffer.extend_from_slice(&chunk);
                while let Some(newline) = buffer.iter().position(|b| *b == b'\n') {
                    let line: Vec<u8> = buffer.drain(..=newline).collect();
                    let Ok(object) = serde_json::from_slice::<Value>(&line[..line.len() - 1])
                    else {
                        continue;
                    };
                    let Some(value) = decode(&object) else {
                        continue;
                    };
                    if tx.send(value).is_err() {
                        return; // the caller dropped the receiver
                    }
                }
            }
        });

        rx
    }

    // Connections

    pub async fn connections(&self) -> Result<Vec<Connection>, Failure> {
        let object = self.get("/connections").await?;
        let Some(list) = object.get("connections").and_then(Value::as_array) else {
            return Ok(Vec::new());
        };
        Ok(list.iter().filter_map(parse_connection).collect())
    }

    /// Closes every open connection. The core reopens what is still wanted, so
    /// this is how a user forces traffic onto a node they just switched to.
    pub async fn close_all_connections(&self) -> Result<(), Failure> {
        self.request(reqwest::Method::DELETE, "/connections", None)
            .await?;
        Ok(())
    }

    pub async fn close_connection(&self, id: &str) -> Result<(), Failure> {
        self.request(reqwest::Method::DELETE, &format!("/connections/{id}"), None)
            .await?;
        Ok(())
    }

    /// Total bytes transferred by the current core process.
    pub async fn totals(&self) -> Result<Traffic, Failure> {
        let object = self.get("/connections").await?;
        Ok(Traffic {
            up: object
                .get("uploadTotal")
                .and_then(Value::as_i64)
                .unwrap_or(0),
            down: object
                .get("downloadTotal")
                .and_then(Value::as_i64)
                .unwrap_or(0),
        })
    }

    async fn get(&self, path: &str) -> Result<Value, Failure> {
        self.request(reqwest::Method::GET, path, None).await
    }

    async fn request(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<Value, Failure> {
        let mut request = self
            .http
            .request(method, format!("{}{path}", self.base))
            .header("Authorization", format!("Bearer {}", self.secret));
        if let Some(body) = body {
            request = request.json(&body);
        }

        let response = request.send().await.map_err(|_| Failure::NotRunning)?;
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(Failure::Http {
                status: status.as_u16(),
                body: text,
            });
        }
        if text.is_empty() {
            return Ok(Value::Object(Default::default()));
        }
        Ok(serde_json::from_str(&text).unwrap_or_else(|_| Value::Object(Default::default())))
    }
}

/// `history` is the core's own record of past delay probes; its last entry is
/// what the UI shows until a fresh probe replaces it.
fn last_history_delay(entry: &Value) -> Option<u32> {
    let delay = entry
        .get("history")?
        .as_array()?
        .last()?
        .get("delay")?
        .as_u64()? as u32;
    (delay > 0).then_some(delay)
}

fn parse_connection(entry: &Value) -> Option<Connection> {
    let id = entry.get("id")?.as_str()?.to_string();
    let empty = Value::Object(Default::default());
    let meta = entry.get("metadata").unwrap_or(&empty);

    let text = |v: &Value, key: &str| -> Option<String> {
        v.get(key)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };

    // `host` is empty for a connection opened straight to an address, in which
    // case the destination IP is the only name there is.
    let host = text(meta, "host")
        .or_else(|| text(meta, "destinationIP"))
        .unwrap_or_else(|| "—".to_string());
    let port = text(meta, "destinationPort").or_else(|| {
        meta.get("destinationPort")
            .and_then(Value::as_u64)
            .map(|p| p.to_string())
    });

    let path = text(meta, "processPath").unwrap_or_default();
    let process = text(meta, "process").unwrap_or_else(|| {
        // Windows paths are backslash-separated; splitting on '/' alone leaves
        // the whole path in a column sized for a file name.
        path.rsplit(['\\', '/'])
            .next()
            .filter(|s| !s.is_empty())
            .unwrap_or("—")
            .to_string()
    });

    Some(Connection {
        id,
        chains: entry
            .get("chains")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
        rule: text(entry, "rule").unwrap_or_default(),
        rule_payload: text(entry, "rulePayload").unwrap_or_default(),
        network: text(meta, "network")
            .unwrap_or_else(|| "tcp".to_string())
            .to_uppercase(),
        host: match port {
            Some(port) => format!("{host}:{port}"),
            None => host,
        },
        process,
        process_path: path,
        upload: entry.get("upload").and_then(Value::as_i64).unwrap_or(0),
        download: entry.get("download").and_then(Value::as_i64).unwrap_or(0),
        start: text(entry, "start")
            .and_then(|s| OffsetDateTime::parse(&s, &Rfc3339).ok())
            .unwrap_or_else(OffsetDateTime::now_utc),
    })
}

/// Node names carry spaces, slashes and emoji, all of which have to survive the
/// round trip into a path component.
fn escape(value: &str) -> String {
    percent_encoding::utf8_percent_encode(value, percent_encoding::NON_ALPHANUMERIC).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_names_survive_the_round_trip_into_a_path() {
        // Spaces, slashes and emoji all appear in real panel node names.
        assert_eq!(escape("Node A"), "Node%20A");
        assert_eq!(escape("eu/se-01"), "eu%2Fse%2D01");
        assert!(escape("🇸🇪 Stockholm").starts_with("%F0%9F%87%B8"));
    }

    #[test]
    fn the_probe_target_is_http_and_cloudflare() {
        // https would time a TLS handshake to the target, which says nothing
        // about the path to the node.
        assert!(PROBE_URL.starts_with("http://"));
        assert!(PROBE_URL.contains("cp.cloudflare.com"));
    }

    #[test]
    fn a_history_entry_supplies_the_last_measured_delay() {
        let entry: Value =
            serde_json::from_str(r#"{"history":[{"delay":100},{"delay":37}]}"#).unwrap();
        assert_eq!(last_history_delay(&entry), Some(37));
    }

    #[test]
    fn a_zero_delay_means_unreachable_not_instant() {
        let entry: Value = serde_json::from_str(r#"{"history":[{"delay":0}]}"#).unwrap();
        assert_eq!(last_history_delay(&entry), None);

        let empty: Value = serde_json::from_str(r#"{"history":[]}"#).unwrap();
        assert_eq!(last_history_delay(&empty), None);

        let absent: Value = serde_json::from_str("{}").unwrap();
        assert_eq!(last_history_delay(&absent), None);
    }

    #[test]
    fn a_connection_is_named_by_its_host_and_port() {
        let entry: Value = serde_json::from_str(
            r#"{"id":"a","chains":["Node A","MOONLIGHT"],"rule":"GeoSite",
                "rulePayload":"google","upload":10,"download":20,
                "metadata":{"host":"example.com","destinationPort":"443",
                            "network":"tcp","process":"chrome.exe"}}"#,
        )
        .unwrap();
        let c = parse_connection(&entry).expect("parses");
        assert_eq!(c.host, "example.com:443");
        assert_eq!(c.network, "TCP");
        assert_eq!(c.process, "chrome.exe");
        // The innermost chain link is what carried it.
        assert_eq!(c.node(), "Node A");
    }

    #[test]
    fn a_connection_with_no_host_falls_back_to_the_destination_address() {
        let entry: Value = serde_json::from_str(
            r#"{"id":"b","metadata":{"host":"","destinationIP":"1.2.3.4","destinationPort":443}}"#,
        )
        .unwrap();
        let c = parse_connection(&entry).expect("parses");
        assert_eq!(c.host, "1.2.3.4:443");
    }

    #[test]
    fn a_windows_process_path_yields_its_file_name() {
        // Splitting on '/' alone would leave the whole path in the column.
        let entry: Value = serde_json::from_str(
            r#"{"id":"c","metadata":{"processPath":"C:\\Program Files\\App\\thing.exe"}}"#,
        )
        .unwrap();
        let c = parse_connection(&entry).expect("parses");
        assert_eq!(c.process, "thing.exe");
    }

    #[test]
    fn a_connection_with_no_process_at_all_reads_as_a_dash() {
        let entry: Value = serde_json::from_str(r#"{"id":"d","metadata":{}}"#).unwrap();
        let c = parse_connection(&entry).expect("parses");
        assert_eq!(c.process, "—");
        assert_eq!(c.host, "—");
    }

    #[test]
    fn an_entry_with_no_id_is_dropped() {
        let entry: Value = serde_json::from_str(r#"{"metadata":{}}"#).unwrap();
        assert!(parse_connection(&entry).is_none());
    }

    #[test]
    fn group_types_cover_every_kind_a_panel_can_build() {
        for kind in ["selector", "urltest", "fallback", "loadbalance", "relay"] {
            assert!(GROUP_TYPES.contains(&kind));
        }
        assert!(!GROUP_TYPES.contains(&"vless"));
        assert!(!GROUP_TYPES.contains(&"trojan"));
    }
}
