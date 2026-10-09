//! Builds the config mihomo actually runs, from the config the panel serves.
//!
//! The panel's document is kept **verbatim** — its `proxies`, `proxy-groups`,
//! `rules` and `dns` are usually better tuned than anything generated here, and
//! a panel that ships a `url-test` balancer or a `geosite:category-ru` direct
//! rule means it. This module overrides only what the client must own:
//!
//! - the RESTful API address and secret, which is how the app talks to the core
//! - the local listener port
//! - `allow-lan: false` and a loopback bind — this is a single-machine client,
//!   and an unbound listener is an open proxy on the network
//! - the TUN block, when the tunnel runs in TUN mode
//! - the user's own rules, placed around the panel's (see [`rules::place`])
//! - a sniffer, when the panel has none switched on (see [`sniffer_block`])

use serde_yaml::{Mapping, Value};
use thiserror::Error;

use crate::models::TunnelMode;
use crate::rules::{self, RoutingRule};

/// The target the latency probe and the injected `url-test` group both use.
/// Kept here as well as in [`crate::api`] so a config can be built without a
/// running core.
pub const PROBE_URL: &str = "http://cp.cloudflare.com/generate_204";

pub const DEFAULT_SELECTOR: &str = "MOONLIGHT";
pub const DEFAULT_AUTO_GROUP: &str = "MOONLIGHT-AUTO";

#[derive(Debug, Clone)]
pub struct Overrides {
    pub controller_port: u16,
    pub secret: String,
    pub mixed_port: u16,
    pub mode: TunnelMode,
    /// The user's own rules, placed around the subscription's.
    pub routing_rules: Vec<RoutingRule>,
    pub log_level: String,
    pub routing_mode: crate::models::RoutingMode,
}

impl Default for Overrides {
    fn default() -> Self {
        Overrides {
            controller_port: 9797,
            secret: String::new(),
            mixed_port: 7897,
            mode: TunnelMode::SystemProxy,
            routing_rules: Vec::new(),
            log_level: "warning".to_string(),
            routing_mode: Default::default(),
        }
    }
}

#[derive(Debug, Error)]
pub enum Failure {
    #[error("Subscription is not a YAML mapping")]
    NotAMapping,
    #[error("Subscription contains no proxies")]
    NoProxies,
    #[error("Subscription is not valid YAML: {0}")]
    Yaml(#[from] serde_yaml::Error),
}

fn key(name: &str) -> Value {
    Value::from(name)
}

/// Grafts `overrides` onto the panel's YAML and returns the result.
pub fn build(panel_yaml: &str, overrides: &Overrides) -> Result<String, Failure> {
    let parsed: Value = serde_yaml::from_str(panel_yaml)?;
    let Value::Mapping(mut root) = parsed else {
        return Err(Failure::NotAMapping);
    };

    let proxies = root
        .get(key("proxies"))
        .and_then(Value::as_sequence)
        .cloned()
        .unwrap_or_default();
    if proxies.is_empty() {
        return Err(Failure::NoProxies);
    }

    // Client-owned general settings
    root.insert(key("mixed-port"), Value::from(overrides.mixed_port));
    root.insert(
        key("external-controller"),
        Value::from(format!("127.0.0.1:{}", overrides.controller_port)),
    );
    root.insert(key("secret"), Value::from(overrides.secret.clone()));
    root.insert(key("log-level"), Value::from(overrides.log_level.clone()));
    root.insert(key("mode"), Value::from(overrides.routing_mode.as_str()));
    root.insert(key("allow-lan"), Value::from(false));
    root.insert(key("bind-address"), Value::from("127.0.0.1"));

    // Ports the panel may have set are removed rather than left listening: one
    // mixed port is the whole surface this client needs.
    for name in [
        "port",
        "socks-port",
        "redir-port",
        "tproxy-port",
        "external-ui",
        "external-controller-tls",
        "external-controller-unix",
    ] {
        root.remove(key(name));
    }

    // Always on. It costs a process lookup per connection, which is cheap, and
    // two things depend on it: `PROCESS-*` split rules, and the connections
    // screen — whose entire question is *which program* is going where.
    // Switching it off when no process rule happened to be configured left that
    // screen showing every connection as "—".
    root.insert(key("find-process-mode"), Value::from("always"));

    // Groups. A config from the share-link fallback has none; one from a panel
    // template almost always does, and those are left exactly as they are.
    let mut groups = root
        .get(key("proxy-groups"))
        .and_then(Value::as_sequence)
        .cloned()
        .unwrap_or_default();
    if groups.is_empty() {
        let names: Vec<String> = proxies
            .iter()
            .filter_map(|p| {
                p.as_mapping()?
                    .get(key("name"))?
                    .as_str()
                    .map(str::to_string)
            })
            .collect();
        groups = default_groups(&names);
        root.insert(key("proxy-groups"), Value::Sequence(groups.clone()));
    }

    // Routing
    let mut rules: Vec<String> = root
        .get(key("rules"))
        .and_then(Value::as_sequence)
        .map(|s| {
            s.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    if rules.is_empty() {
        rules = vec![format!("MATCH,{DEFAULT_SELECTOR}")];
    }

    // The user's rules may name any group or server the config has, or the
    // two built-in targets; a rule naming anything else is left out.
    let mut targets: std::collections::HashSet<String> = [rules::DIRECT, rules::REJECT]
        .into_iter()
        .map(String::from)
        .collect();
    for entry in groups.iter().chain(proxies.iter()) {
        if let Some(name) = entry.get(key("name")).and_then(Value::as_str) {
            targets.insert(name.to_string());
        }
    }
    let composed = rules::place(
        &overrides.routing_rules,
        &rules,
        &targets,
        overrides.mode == TunnelMode::Tun,
    );
    root.insert(
        key("rules"),
        Value::Sequence(composed.into_iter().map(Value::from).collect()),
    );

    // Sniffer. A panel that switched one on means it, and keeps it. One that
    // has none — or has one switched off, as mihomo's sample config ships it —
    // gets the app's, because without it the rules above go blind the moment a
    // browser resolves names itself.
    let panel_sniffer_on = root
        .get(key("sniffer"))
        .and_then(|s| s.get("enable"))
        .and_then(Value::as_bool)
        == Some(true);
    if !panel_sniffer_on {
        root.insert(key("sniffer"), Value::Mapping(sniffer_block()));
    }

    // TUN
    if overrides.mode == TunnelMode::Tun {
        root.insert(key("tun"), Value::Mapping(tun_block()));
        // TUN without DNS hijacking leaks every lookup to the resolver the
        // machine had before the interface came up.
        let existing = root.get(key("dns")).and_then(Value::as_mapping).cloned();
        root.insert(key("dns"), Value::Mapping(dns_block(existing)));
    } else {
        root.remove(key("tun"));
    }

    Ok(serde_yaml::to_string(&Value::Mapping(root))?)
}

/// A minimal config that carries just a proxy list — the shape the share-link
/// fallback produces before [`build`] runs.
pub fn yaml_from_proxies(proxies: &[Mapping]) -> String {
    let names: Vec<String> = proxies
        .iter()
        .filter_map(|p| p.get(key("name"))?.as_str().map(str::to_string))
        .collect();

    let mut root = Mapping::new();
    root.insert(
        key("proxies"),
        Value::Sequence(proxies.iter().cloned().map(Value::Mapping).collect()),
    );
    root.insert(key("proxy-groups"), Value::Sequence(default_groups(&names)));
    root.insert(
        key("rules"),
        Value::Sequence(vec![Value::from(format!("MATCH,{DEFAULT_SELECTOR}"))]),
    );
    serde_yaml::to_string(&Value::Mapping(root)).unwrap_or_default()
}

pub fn default_groups(proxy_names: &[String]) -> Vec<Value> {
    let mut selector = Mapping::new();
    selector.insert(key("name"), Value::from(DEFAULT_SELECTOR));
    selector.insert(key("type"), Value::from("select"));
    let mut members = vec![Value::from(DEFAULT_AUTO_GROUP)];
    members.extend(proxy_names.iter().map(|n| Value::from(n.clone())));
    selector.insert(key("proxies"), Value::Sequence(members));

    let mut auto = Mapping::new();
    auto.insert(key("name"), Value::from(DEFAULT_AUTO_GROUP));
    auto.insert(key("type"), Value::from("url-test"));
    auto.insert(
        key("proxies"),
        Value::Sequence(proxy_names.iter().map(|n| Value::from(n.clone())).collect()),
    );
    auto.insert(key("url"), Value::from(PROBE_URL));
    auto.insert(key("interval"), Value::from(300));
    auto.insert(key("tolerance"), Value::from(50));

    vec![Value::Mapping(selector), Value::Mapping(auto)]
}

/// The group the app drives when the user picks a node.
///
/// A panel names its groups whatever it likes, so the group is found the way
/// the config itself points at it: the target of the catch-all `MATCH` rule,
/// falling back to the first `select` group. Guessing by name would break on any
/// panel that localises its group labels.
pub fn primary_selector_name(groups: &[Value], rules: &[String]) -> String {
    let group_name = |v: &Value| -> Option<String> {
        v.as_mapping()?
            .get(key("name"))?
            .as_str()
            .map(str::to_string)
    };

    if let Some(target) = rules
        .iter()
        .rev()
        .find(|r| r.to_uppercase().starts_with("MATCH,"))
        .map(|r| r["MATCH,".len()..].trim().to_string())
    {
        if groups
            .iter()
            .any(|g| group_name(g).as_deref() == Some(&target))
        {
            return target;
        }
    }

    if let Some(name) = groups
        .iter()
        .find(|g| {
            g.as_mapping()
                .and_then(|m| m.get(key("type")))
                .and_then(Value::as_str)
                == Some("select")
        })
        .and_then(group_name)
    {
        return name;
    }

    groups
        .first()
        .and_then(group_name)
        .unwrap_or_else(|| DEFAULT_SELECTOR.to_string())
}

/// What the rules page needs from a subscription: its groups, in order —
/// the targets a rule can name besides DIRECT and REJECT — and its own rules,
/// to list.
pub fn routing_inputs(panel_yaml: &str) -> (Vec<String>, Vec<String>) {
    let Ok(root) = serde_yaml::from_str::<Value>(panel_yaml) else {
        return (Vec::new(), Vec::new());
    };
    let names = |section: &str| -> Vec<String> {
        root.get(section)
            .and_then(Value::as_sequence)
            .into_iter()
            .flatten()
            .filter_map(|entry| entry.get("name")?.as_str().map(str::to_string))
            .collect()
    };
    let mut groups = names("proxy-groups");
    // A config with no groups gets the app's own when it is built.
    if groups.is_empty() && root.get("proxies").is_some() {
        groups = vec![DEFAULT_SELECTOR.to_string(), DEFAULT_AUTO_GROUP.to_string()];
    }
    let rules = root
        .get("rules")
        .and_then(Value::as_sequence)
        .into_iter()
        .flatten()
        .filter_map(|r| r.as_str().map(str::to_string))
        .collect();
    (groups, rules)
}

/// Has the core read the hostname out of a connection that arrived as a bare
/// address.
///
/// A browser on secure DNS — Chrome's "Use secure DNS" with Google or
/// Cloudflare — resolves names itself, over HTTPS, past the core's DNS. In TUN
/// its connections then reach the core as bare addresses, with nothing for the
/// fake-ip table to map back, so no `DOMAIN-SUFFIX` or `GEOSITE` rule can match
/// them: everything fell through to the catch-all and went to the server as a
/// bare address — a `.ru` site the panel sends `DIRECT` included, and IPv6 ones
/// the core's own DNS (`ipv6: false`) never hands out — and the connections
/// screen listed addresses where it had always listed hosts. The browser saw
/// `ERR_CONNECTION_CLOSED` wherever the far end could not carry that.
///
/// With the sniffer the core reads the name from the TLS ClientHello, the QUIC
/// Initial, or the HTTP `Host` header, matches the rules on it, and —
/// `override-destination` — dials the *name* rather than the address: the
/// server resolves it the way it can reach it, and a `DIRECT` connection
/// resolves it through the panel's DNS, Yandex for Russian names. Ports and
/// skips are those of the example configs in mihomo's documentation; the three
/// switches are the core's own defaults for a sniffer that is on, written out so
/// the config says what it does. The same block the macOS client writes.
pub fn sniffer_block() -> Mapping {
    let ports = |list: &[&str]| -> Value {
        let mut protocol = Mapping::new();
        // Ports as strings: a range has to be one, and the core's parser takes
        // a list of them.
        protocol.insert(
            key("ports"),
            Value::Sequence(list.iter().map(|p| Value::from(*p)).collect()),
        );
        Value::Mapping(protocol)
    };

    let mut sniff = Mapping::new();
    sniff.insert(key("HTTP"), ports(&["80", "8080-8880"]));
    sniff.insert(key("TLS"), ports(&["443", "8443"]));
    sniff.insert(key("QUIC"), ports(&["443", "8443"]));

    let mut sniffer = Mapping::new();
    sniffer.insert(key("enable"), Value::from(true));
    // Sniff connections that came in as a bare address — the browser case — and
    // those whose name the core could only recover from a real (not fake) IP it
    // had answered with.
    sniffer.insert(key("parse-pure-ip"), Value::from(true));
    sniffer.insert(key("force-dns-mapping"), Value::from(true));
    sniffer.insert(key("override-destination"), Value::from(true));
    sniffer.insert(key("sniff"), Value::Mapping(sniff));
    // Apple's push service is left at the address the system chose for it, as
    // those examples leave it.
    sniffer.insert(
        key("skip-domain"),
        Value::Sequence(vec![Value::from("+.push.apple.com")]),
    );
    sniffer
}

pub fn tun_block() -> Mapping {
    let mut tun = Mapping::new();
    tun.insert(key("enable"), Value::from(true));
    // No `device`: the core creates the Wintun adapter itself and naming one
    // that another client already holds fails the whole start. Letting the core
    // pick is the only way to be sure.
    //
    // `mixed` is the recommended stack: gvisor's userspace TCP with the system
    // stack's UDP, which avoids gvisor's UDP throughput cost.
    tun.insert(key("stack"), Value::from("mixed"));
    tun.insert(key("auto-route"), Value::from(true));
    tun.insert(key("auto-detect-interface"), Value::from(true));
    tun.insert(key("strict-route"), Value::from(false));
    tun.insert(
        key("dns-hijack"),
        Value::Sequence(vec![Value::from("any:53"), Value::from("tcp://any:53")]),
    );
    tun.insert(key("mtu"), Value::from(1500));
    tun
}

/// DNS for TUN mode.
///
/// A panel's own `dns` block is kept if it has one — it may point at a resolver
/// inside the tunnel on purpose. Only the fields TUN needs are forced on:
/// without `enable`, mihomo does not answer the queries `dns-hijack` redirects
/// to it, and the tunnel resolves nothing.
pub fn dns_block(existing: Option<Mapping>) -> Mapping {
    let mut dns = existing.unwrap_or_default();
    dns.insert(key("enable"), Value::from(true));
    let mut default = |name: &str, value: Value| {
        if !dns.contains_key(key(name)) {
            dns.insert(key(name), value);
        }
    };
    default("ipv6", Value::from(false));
    default("listen", Value::from("127.0.0.1:53535"));
    // A fake-ip range keeps DNS out of the round trip for proxied hosts.
    default("enhanced-mode", Value::from("fake-ip"));
    default("fake-ip-range", Value::from("198.18.0.1/16"));
    default(
        "nameserver",
        Value::Sequence(vec![
            Value::from("https://1.1.1.1/dns-query"),
            Value::from("https://dns.google/dns-query"),
        ]),
    );
    dns
}

/// The order the panel itself lists its servers in.
///
/// mihomo hands back a selector's members in *its* order — the group's explicit
/// entries first, then everything `include-all` swept up — which is not the
/// order the panel's document lists them in, and so not the order the macOS and
/// mobile clients show. There the destinations come in document order with each
/// transit group sitting beside the server it fronts (Finland, then
/// `Russia -> Finland`), and the squads stay in their blocks.
///
/// This reads that order back out of the subscription: every proxy is ranked by
/// its position in `proxies:`, and a group is ranked by the first member it
/// covers — resolved through the group's explicit list, or through the `filter`
/// an `include-all` group selects its members with.
/// The service's notes on its servers and groups, by name: `serverDescription`
/// on a proxy, `description` on a group.
pub fn server_descriptions(yaml: &str) -> std::collections::HashMap<String, String> {
    let mut descriptions = std::collections::HashMap::new();
    let Ok(root) = serde_yaml::from_str::<Value>(yaml) else {
        return descriptions;
    };
    for (section, field) in [
        ("proxies", "serverDescription"),
        ("proxy-groups", "description"),
    ] {
        for entry in root
            .get(section)
            .and_then(Value::as_sequence)
            .into_iter()
            .flatten()
        {
            let name = entry.get("name").and_then(Value::as_str);
            let text = entry.get(field).and_then(Value::as_str).map(str::trim);
            if let (Some(name), Some(text)) = (name, text.filter(|t| !t.is_empty())) {
                descriptions.insert(name.to_string(), text.to_string());
            }
        }
    }
    descriptions
}

pub fn panel_order(yaml: &str) -> std::collections::HashMap<String, usize> {
    let mut order = std::collections::HashMap::new();
    let Ok(document) = serde_yaml::from_str::<Value>(yaml) else {
        return order;
    };

    let names: Vec<String> = document
        .get("proxies")
        .and_then(Value::as_sequence)
        .map(|proxies| {
            proxies
                .iter()
                .filter_map(|proxy| {
                    proxy
                        .get("name")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default();

    for (index, name) in names.iter().enumerate() {
        order.insert(name.clone(), index);
    }

    let Some(groups) = document.get("proxy-groups").and_then(Value::as_sequence) else {
        return order;
    };

    // Two passes, so a group whose members are themselves groups still lands:
    // the first pass ranks the groups that sit directly on proxies, the second
    // sees those ranks.
    for _ in 0..2 {
        for group in groups {
            let Some(name) = group.get("name").and_then(Value::as_str) else {
                continue;
            };
            if order.contains_key(name) {
                continue;
            }

            let mut rank: Option<usize> = None;

            if let Some(members) = group.get("proxies").and_then(Value::as_sequence) {
                for member in members.iter().filter_map(Value::as_str) {
                    if let Some(&at) = order.get(member) {
                        rank = Some(rank.map_or(at, |best: usize| best.min(at)));
                    }
                }
            }

            // `include-all` groups name no members; what they take is whatever
            // the filter matches, so the filter is where their position is.
            if rank.is_none() {
                if let Some(filter) = group.get("filter").and_then(Value::as_str) {
                    if let Ok(pattern) = regex::Regex::new(filter) {
                        for (index, candidate) in names.iter().enumerate() {
                            if pattern.is_match(candidate) {
                                rank = Some(rank.map_or(index, |best: usize| best.min(index)));
                            }
                        }
                    }
                }
            }

            if let Some(rank) = rank {
                order.insert(name.to_string(), rank);
            }
        }
    }

    order
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_order_follows_the_document_and_seats_groups_beside_their_members() {
        // The shape the panel actually sends: destinations in document order,
        // each transit balancer sitting next to the server it fronts, and the
        // selector listing the transit groups first — which is the order this
        // client used to show, and the one being corrected.
        let yaml = r#"
proxies:
  - {name: "RU", type: vless}
  - {name: "FI", type: vless}
  - {name: "Russia Finland Balance 1", type: vless}
  - {name: "DE", type: vless}
  - {name: "Russia Germany Balance 1", type: vless}
proxy-groups:
  - name: "picker"
    type: select
    include-all: true
    proxies: ["Auto", "RU -> DE", "RU -> FI"]
  - name: "RU -> FI"
    type: load-balance
    filter: "Russia Finland Balance"
    proxies: []
  - name: "RU -> DE"
    type: load-balance
    filter: "Russia Germany Balance"
    proxies: []
"#;
        let order = panel_order(yaml);

        assert_eq!(order.get("RU"), Some(&0));
        assert_eq!(order.get("FI"), Some(&1));
        assert_eq!(order.get("DE"), Some(&3));
        // Each balancer takes the position of the proxy its filter selects, so
        // it sorts next to that server rather than up with the other groups.
        assert_eq!(order.get("RU -> FI"), Some(&2));
        assert_eq!(order.get("RU -> DE"), Some(&4));

        let mut listed = vec!["DE", "RU -> FI", "RU", "RU -> DE", "FI"];
        listed.sort_by_key(|name| order.get(*name).copied().unwrap_or(usize::MAX));
        assert_eq!(listed, ["RU", "FI", "RU -> FI", "DE", "RU -> DE"]);
    }

    #[test]
    fn panel_order_is_empty_when_the_document_has_no_proxies() {
        assert!(panel_order("rules: []").is_empty());
        assert!(panel_order("not: [valid").is_empty());
    }
    use crate::rules::{Kind, Priority, RoutingRule};

    const PANEL: &str = r#"
proxies:
  - name: "Node A"
    type: vless
    server: 1.2.3.4
    port: 443
  - name: "Node B"
    type: trojan
    server: 5.6.7.8
    port: 443
proxy-groups:
  - name: "PANEL-SELECT"
    type: select
    proxies: ["Node A", "Node B"]
rules:
  - "GEOSITE,category-ru,DIRECT"
  - "MATCH,PANEL-SELECT"
"#;

    fn overrides() -> Overrides {
        Overrides {
            secret: "s3cret".into(),
            ..Default::default()
        }
    }

    fn parse(yaml: &str) -> Mapping {
        match serde_yaml::from_str::<Value>(yaml).expect("valid yaml") {
            Value::Mapping(m) => m,
            other => panic!("expected a mapping, got {other:?}"),
        }
    }

    fn rules_of(root: &Mapping) -> Vec<String> {
        root.get(key("rules"))
            .and_then(Value::as_sequence)
            .expect("rules")
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect()
    }

    #[test]
    fn the_client_owns_the_controller_and_the_listener() {
        let built = build(PANEL, &overrides()).expect("builds");
        let root = parse(&built);
        assert_eq!(
            root.get(key("external-controller")).and_then(Value::as_str),
            Some("127.0.0.1:9797")
        );
        assert_eq!(
            root.get(key("secret")).and_then(Value::as_str),
            Some("s3cret")
        );
        assert_eq!(
            root.get(key("mixed-port")).and_then(Value::as_i64),
            Some(7897)
        );
    }

    #[test]
    fn the_listener_is_never_open_to_the_network() {
        let built = build(PANEL, &overrides()).expect("builds");
        let root = parse(&built);
        assert_eq!(
            root.get(key("allow-lan")).and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            root.get(key("bind-address")).and_then(Value::as_str),
            Some("127.0.0.1")
        );
    }

    #[test]
    fn extra_listeners_the_panel_set_are_removed() {
        let panel = format!("{PANEL}\nport: 7890\nsocks-port: 7891\nexternal-ui: ./ui\n");
        let built = build(&panel, &overrides()).expect("builds");
        let root = parse(&built);
        for name in ["port", "socks-port", "external-ui"] {
            assert!(!root.contains_key(key(name)), "{name} was left listening");
        }
    }

    #[test]
    fn the_panels_own_rules_and_groups_are_kept_verbatim() {
        let built = build(PANEL, &overrides()).expect("builds");
        let root = parse(&built);
        assert_eq!(
            rules_of(&root),
            vec!["GEOSITE,category-ru,DIRECT", "MATCH,PANEL-SELECT"]
        );
        let groups = root
            .get(key("proxy-groups"))
            .and_then(Value::as_sequence)
            .expect("groups");
        assert_eq!(groups.len(), 1, "the panel's single group must survive");
    }

    #[test]
    fn find_process_mode_is_always_on() {
        // The connections screen shows "—" for every row without it, whether or
        // not a process rule happens to be configured.
        let built = build(PANEL, &overrides()).expect("builds");
        let root = parse(&built);
        assert_eq!(
            root.get(key("find-process-mode")).and_then(Value::as_str),
            Some("always")
        );
    }

    #[test]
    fn a_config_with_no_proxies_is_refused() {
        assert!(matches!(
            build("proxies: []\nrules: []", &overrides()),
            Err(Failure::NoProxies)
        ));
        assert!(matches!(
            build("rules: []", &overrides()),
            Err(Failure::NoProxies)
        ));
    }

    #[test]
    fn a_non_mapping_document_is_refused() {
        assert!(matches!(
            build("- just\n- a\n- list", &overrides()),
            Err(Failure::NotAMapping)
        ));
    }

    #[test]
    fn a_config_with_no_groups_gets_the_default_pair() {
        let bare = "proxies:\n  - name: A\n    type: vless\n";
        let built = build(bare, &overrides()).expect("builds");
        let root = parse(&built);
        let groups = root
            .get(key("proxy-groups"))
            .and_then(Value::as_sequence)
            .expect("groups");
        assert_eq!(groups.len(), 2);
        assert_eq!(rules_of(&root), vec![format!("MATCH,{DEFAULT_SELECTOR}")]);
    }

    #[test]
    fn the_selector_is_found_through_the_match_rule() {
        let groups = default_groups(&["A".into()]);
        let rules = vec!["MATCH,MOONLIGHT".to_string()];
        assert_eq!(primary_selector_name(&groups, &rules), "MOONLIGHT");
    }

    #[test]
    fn a_panel_that_localises_its_group_names_is_still_followed() {
        // Guessing by name would fail here; following MATCH does not.
        let yaml = r#"
proxy-groups:
  - name: "Выбор узла"
    type: select
    proxies: ["A"]
rules:
  - "MATCH,Выбор узла"
"#;
        let root = parse(yaml);
        let groups = root
            .get(key("proxy-groups"))
            .and_then(Value::as_sequence)
            .unwrap()
            .clone();
        let rules = rules_of(&root);
        assert_eq!(primary_selector_name(&groups, &rules), "Выбор узла");
    }

    #[test]
    fn a_match_pointing_at_direct_falls_back_to_the_first_select_group() {
        let groups = default_groups(&["A".into()]);
        let rules = vec!["MATCH,DIRECT".to_string()];
        assert_eq!(primary_selector_name(&groups, &rules), DEFAULT_SELECTOR);
    }

    #[test]
    fn tun_mode_adds_the_interface_and_the_dns_that_makes_it_resolve() {
        let mut o = overrides();
        o.mode = TunnelMode::Tun;
        let built = build(PANEL, &o).expect("builds");
        let root = parse(&built);

        let tun = root
            .get(key("tun"))
            .and_then(Value::as_mapping)
            .expect("tun");
        assert_eq!(tun.get(key("enable")).and_then(Value::as_bool), Some(true));
        assert_eq!(tun.get(key("stack")).and_then(Value::as_str), Some("mixed"));
        assert!(
            !tun.contains_key(key("device")),
            "naming the adapter collides with whatever already holds it"
        );

        let dns = root
            .get(key("dns"))
            .and_then(Value::as_mapping)
            .expect("dns");
        assert_eq!(dns.get(key("enable")).and_then(Value::as_bool), Some(true));
    }

    #[test]
    fn a_panel_with_no_sniffer_gets_the_apps_in_either_mode() {
        // A browser on secure DNS reaches the core with bare addresses, and
        // without a sniffer no domain rule can match them.
        for mode in [TunnelMode::SystemProxy, TunnelMode::Tun] {
            let mut o = overrides();
            o.mode = mode;
            let root = parse(&build(PANEL, &o).expect("builds"));
            let sniffer = root
                .get(key("sniffer"))
                .and_then(Value::as_mapping)
                .expect("sniffer");
            let flag = |name: &str| sniffer.get(key(name)).and_then(Value::as_bool);
            assert_eq!(flag("enable"), Some(true), "{mode:?}");
            // A connection to a bare address is sniffed, and dialled by the
            // name found rather than the address the browser resolved.
            assert_eq!(flag("parse-pure-ip"), Some(true));
            assert_eq!(flag("override-destination"), Some(true));

            let ports = |protocol: &str| -> Vec<String> {
                sniffer
                    .get(key("sniff"))
                    .and_then(|s| s.get(protocol))
                    .and_then(|p| p.get("ports"))
                    .and_then(Value::as_sequence)
                    .expect("ports")
                    .iter()
                    .filter_map(|p| p.as_str().map(str::to_string))
                    .collect()
            };
            assert_eq!(ports("TLS"), ["443", "8443"]);
            // QUIC too, which a browser uses wherever a site offers it.
            assert_eq!(ports("QUIC"), ["443", "8443"]);
            // A range survives as a string.
            assert_eq!(ports("HTTP"), ["80", "8080-8880"]);
        }
    }

    #[test]
    fn a_panels_own_sniffer_is_kept_as_it_wrote_it() {
        let panel = format!(
            "{PANEL}\nsniffer:\n  enable: true\n  override-destination: false\n  sniff:\n    TLS: {{ports: [443]}}\n"
        );
        let root = parse(&build(&panel, &overrides()).expect("builds"));
        let sniffer = root
            .get(key("sniffer"))
            .and_then(Value::as_mapping)
            .expect("sniffer");
        assert_eq!(
            sniffer
                .get(key("override-destination"))
                .and_then(Value::as_bool),
            Some(false)
        );
        // And not merged with the app's.
        assert!(sniffer
            .get(key("sniff"))
            .and_then(|s| s.get("QUIC"))
            .is_none());
    }

    #[test]
    fn a_switched_off_sniffer_is_replaced_by_the_apps() {
        // mihomo's sample config ships one switched off, which is not a reason
        // to leave the rules blind.
        let panel = format!("{PANEL}\nsniffer:\n  enable: false\n");
        let root = parse(&build(&panel, &overrides()).expect("builds"));
        assert_eq!(
            root.get(key("sniffer"))
                .and_then(|s| s.get("enable"))
                .and_then(Value::as_bool),
            Some(true)
        );
    }

    #[test]
    fn system_proxy_mode_carries_no_tun_block() {
        let built = build(PANEL, &overrides()).expect("builds");
        assert!(!parse(&built).contains_key(key("tun")));
    }

    #[test]
    fn a_panels_own_dns_survives_tun_mode() {
        // A panel may point DNS at a resolver inside the tunnel on purpose.
        let panel = format!("{PANEL}\ndns:\n  nameserver: ['10.0.0.1']\n  ipv6: true\n");
        let mut o = overrides();
        o.mode = TunnelMode::Tun;
        let built = build(&panel, &o).expect("builds");
        let root = parse(&built);
        let dns = root
            .get(key("dns"))
            .and_then(Value::as_mapping)
            .expect("dns");

        assert_eq!(dns.get(key("ipv6")).and_then(Value::as_bool), Some(true));
        let ns = dns
            .get(key("nameserver"))
            .and_then(Value::as_sequence)
            .expect("nameserver");
        assert_eq!(ns[0].as_str(), Some("10.0.0.1"));
        // But enable is forced, or hijacked queries go unanswered.
        assert_eq!(dns.get(key("enable")).and_then(Value::as_bool), Some(true));
    }

    #[test]
    fn a_panel_dns_that_disables_itself_is_overridden_in_tun_mode() {
        let panel = format!("{PANEL}\ndns:\n  enable: false\n");
        let mut o = overrides();
        o.mode = TunnelMode::Tun;
        let built = build(&panel, &o).expect("builds");
        let root = parse(&built);
        let dns = root.get(key("dns")).and_then(Value::as_mapping).unwrap();
        assert_eq!(dns.get(key("enable")).and_then(Value::as_bool), Some(true));
    }

    #[test]
    fn yaml_from_proxies_produces_something_build_accepts() {
        let mut proxy = Mapping::new();
        proxy.insert(key("name"), Value::from("Node A"));
        proxy.insert(key("type"), Value::from("vless"));

        let yaml = yaml_from_proxies(&[proxy]);
        let built = build(&yaml, &overrides()).expect("the fallback shape must build");
        let root = parse(&built);
        assert!(root.contains_key(key("proxy-groups")));
    }

    #[test]
    fn the_users_rules_land_around_the_panels_and_name_only_what_exists() {
        let mut o = overrides();
        o.routing_rules = vec![
            RoutingRule::new(Kind::DomainSuffix, "a.com", "DIRECT", Priority::Override),
            RoutingRule::new(
                Kind::DomainSuffix,
                "b.com",
                "PANEL-SELECT",
                Priority::Extend,
            ),
            RoutingRule::new(Kind::DomainSuffix, "c.com", "Node B", Priority::Extend),
            // A group the panel dropped: left out, or the core refuses it all.
            RoutingRule::new(Kind::DomainSuffix, "d.com", "Gone", Priority::Override),
            // No TUN here, so no process to match on.
            RoutingRule::new(Kind::ProcessName, "a.exe", "DIRECT", Priority::Override),
        ];
        let built = build(PANEL, &o).expect("builds");
        assert_eq!(
            rules_of(&parse(&built)),
            vec![
                "DOMAIN-SUFFIX,a.com,DIRECT",
                "GEOSITE,category-ru,DIRECT",
                "DOMAIN-SUFFIX,b.com,PANEL-SELECT",
                "DOMAIN-SUFFIX,c.com,Node B",
                "MATCH,PANEL-SELECT",
            ]
        );

        o.mode = TunnelMode::Tun;
        let tun = build(PANEL, &o).expect("builds");
        assert_eq!(
            rules_of(&parse(&tun))[1],
            "PROCESS-NAME,a.exe,DIRECT",
            "in the user's order"
        );
    }

    #[test]
    fn the_rules_page_reads_the_panels_groups_and_rules() {
        let (groups, rules) = routing_inputs(PANEL);
        assert_eq!(groups, vec!["PANEL-SELECT"]);
        assert_eq!(
            rules,
            vec!["GEOSITE,category-ru,DIRECT", "MATCH,PANEL-SELECT"]
        );
        let (fallback, _) = routing_inputs("proxies:\n  - {name: A, type: ss}\n");
        assert_eq!(fallback, vec![DEFAULT_SELECTOR, DEFAULT_AUTO_GROUP]);
    }

    #[test]
    fn the_services_notes_are_read_from_proxies_and_groups() {
        let yaml = "proxies:\n  - {name: Poland LTE 1, type: vless, serverDescription: ' Доступность во время БС 🌟 '}\n  - {name: Plain, type: vless}\nproxy-groups:\n  - {name: Auto, type: url-test, description: Лучший, proxies: [Plain]}\n";
        let notes = server_descriptions(yaml);
        assert_eq!(
            notes.get("Poland LTE 1").map(String::as_str),
            Some("Доступность во время БС 🌟")
        );
        assert_eq!(notes.get("Auto").map(String::as_str), Some("Лучший"));
        assert!(!notes.contains_key("Plain"));
    }

    #[test]
    fn the_routing_mode_is_written_as_the_cores_own_mode() {
        let config = build(
            "proxies:\n  - {name: A, type: ss, server: a.example, port: 1, cipher: aes-128-gcm, password: x}\n",
            &Overrides {
                routing_mode: crate::models::RoutingMode::Global,
                ..Overrides::default()
            },
        )
        .expect("builds");
        let value: Value = serde_yaml::from_str(&config).expect("yaml");
        assert_eq!(value.get("mode").and_then(Value::as_str), Some("global"));
    }
}
