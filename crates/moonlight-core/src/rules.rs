//! Routing rules of the user's own: what to match, and where to send it —
//! around the tunnel, nowhere, or through one of the subscription's groups.
//!
//! Kept apart from the subscription and stored by the app, so a refresh never
//! touches them. Each sits either before the subscription's rules
//! ([`Priority::Override`]) or after them ([`Priority::Extend`]); mihomo takes
//! the first rule that matches, so that choice is the whole of a rule's
//! priority. This is how Flowvy's "Мои правила" work, and the grammar is
//! mihomo's own. They replace the apps screen and its split modes: a process
//! rule does what an app switch did, and can send an app anywhere.

use std::collections::HashSet;
use std::fmt;
use std::net::IpAddr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::SplitMode;

/// What a rule matches on. Serialised by variant name, as the apps screen's
/// rules were, so theirs still read for the carry-over.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Kind {
    Domain,
    DomainSuffix,
    DomainKeyword,
    DomainRegex,
    Geosite,
    IpCidr,
    IpCidr6,
    IpAsn,
    Geoip,
    SrcIpCidr,
    DstPort,
    SrcPort,
    ProcessName,
    ProcessNameRegex,
    ProcessPath,
    ProcessPathRegex,
    Network,
}

/// The headings the type picker groups kinds under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    Domain,
    Ip,
    Port,
    Process,
    Other,
}

impl Kind {
    /// In the order the picker lists them, family by family.
    pub const ALL: &'static [Kind] = &[
        Kind::Domain,
        Kind::DomainSuffix,
        Kind::DomainKeyword,
        Kind::DomainRegex,
        Kind::Geosite,
        Kind::IpCidr,
        Kind::IpCidr6,
        Kind::IpAsn,
        Kind::Geoip,
        Kind::SrcIpCidr,
        Kind::DstPort,
        Kind::SrcPort,
        Kind::ProcessName,
        Kind::ProcessNameRegex,
        Kind::ProcessPath,
        Kind::ProcessPathRegex,
        Kind::Network,
    ];

    /// The token mihomo's rule grammar uses.
    pub fn token(self) -> &'static str {
        match self {
            Kind::Domain => "DOMAIN",
            Kind::DomainSuffix => "DOMAIN-SUFFIX",
            Kind::DomainKeyword => "DOMAIN-KEYWORD",
            Kind::DomainRegex => "DOMAIN-REGEX",
            Kind::Geosite => "GEOSITE",
            Kind::IpCidr => "IP-CIDR",
            Kind::IpCidr6 => "IP-CIDR6",
            Kind::IpAsn => "IP-ASN",
            Kind::Geoip => "GEOIP",
            Kind::SrcIpCidr => "SRC-IP-CIDR",
            Kind::DstPort => "DST-PORT",
            Kind::SrcPort => "SRC-PORT",
            Kind::ProcessName => "PROCESS-NAME",
            Kind::ProcessNameRegex => "PROCESS-NAME-REGEX",
            Kind::ProcessPath => "PROCESS-PATH",
            Kind::ProcessPathRegex => "PROCESS-PATH-REGEX",
            Kind::Network => "NETWORK",
        }
    }

    pub fn family(self) -> Family {
        match self {
            Kind::Domain
            | Kind::DomainSuffix
            | Kind::DomainKeyword
            | Kind::DomainRegex
            | Kind::Geosite => Family::Domain,
            Kind::IpCidr | Kind::IpCidr6 | Kind::IpAsn | Kind::Geoip | Kind::SrcIpCidr => {
                Family::Ip
            }
            Kind::DstPort | Kind::SrcPort => Family::Port,
            Kind::ProcessName
            | Kind::ProcessNameRegex
            | Kind::ProcessPath
            | Kind::ProcessPathRegex => Family::Process,
            Kind::Network => Family::Other,
        }
    }

    /// Whether the core has to identify the process behind a connection to
    /// evaluate this. Only TUN mode can — under a system proxy the core is
    /// handed a socket with no process behind it — so these are written into
    /// the config only in TUN, and the editor marks them.
    pub fn needs_process_matching(self) -> bool {
        self.family() == Family::Process
    }

    /// Address rules carry `no-resolve`, so a domain is not looked up just to
    /// be tested against an address — a DNS query per connection.
    fn wants_no_resolve(self) -> bool {
        matches!(
            self,
            Kind::IpCidr | Kind::IpCidr6 | Kind::IpAsn | Kind::Geoip
        )
    }

    /// The example shown in the empty field. Process examples are
    /// Windows-shaped: mihomo reads the executable name back with its `.exe`,
    /// and an example without one teaches a rule that never matches.
    pub fn placeholder(self) -> &'static str {
        match self {
            Kind::Domain => "example.com",
            Kind::DomainSuffix => "google.com",
            Kind::DomainKeyword => "google",
            Kind::DomainRegex => r"^.*\.discord\.(com|gg)$",
            Kind::Geosite => "youtube",
            Kind::IpCidr => "192.168.1.0/24",
            Kind::IpCidr6 => "2001:db8::/32",
            Kind::IpAsn => "13335",
            Kind::Geoip => "ru",
            Kind::SrcIpCidr => "192.168.1.0/24",
            Kind::DstPort => "443",
            Kind::SrcPort => "7777",
            Kind::ProcessName => "Telegram.exe",
            Kind::ProcessNameRegex => "(?i).*chrome.*",
            Kind::ProcessPath => r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            Kind::ProcessPathRegex => r"(?i).*\\steam.*",
            Kind::Network => "udp",
        }
    }
}

/// The grammar's own token is what the picker and every row show.
impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.token())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Priority {
    /// Before the subscription's rules: wins over them.
    #[default]
    Override,
    /// After the subscription's rules, before its catch-all: only what they
    /// leave unmatched reaches it.
    Extend,
}

/// The two targets every config has, besides the subscription's groups.
pub const DIRECT: &str = "DIRECT";
pub const REJECT: &str = "REJECT";

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutingRule {
    pub id: Uuid,
    pub kind: Kind,
    pub value: String,
    pub target: String,
    pub priority: Priority,
    pub enabled: bool,
}

impl RoutingRule {
    pub fn new(
        kind: Kind,
        value: impl Into<String>,
        target: impl Into<String>,
        priority: Priority,
    ) -> Self {
        RoutingRule {
            id: Uuid::new_v4(),
            kind,
            value: value.into().trim().to_string(),
            target: target.into(),
            priority,
            enabled: true,
        }
    }

    /// The rule as mihomo's rule grammar writes it.
    pub fn line(&self) -> String {
        let suffix = if self.kind.wants_no_resolve() {
            ",no-resolve"
        } else {
            ""
        };
        format!(
            "{},{},{}{suffix}",
            self.kind.token(),
            self.value.trim(),
            self.target
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invalid {
    Empty,
    ContainsComma,
    BadRegex(String),
    BadPort,
    BadCidr,
    BadAsn,
    BadNetwork,
}

impl fmt::Display for Invalid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Invalid::Empty => write!(f, "The rule has no value"),
            Invalid::ContainsComma => write!(f, "A value cannot contain a comma"),
            Invalid::BadRegex(why) => write!(f, "Not a valid regular expression: {why}"),
            Invalid::BadPort => write!(f, "Not a valid port"),
            Invalid::BadCidr => write!(f, "Not a valid CIDR block"),
            Invalid::BadAsn => write!(f, "Not a valid ASN"),
            Invalid::BadNetwork => write!(f, "Not tcp or udp"),
        }
    }
}

impl std::error::Error for Invalid {}

/// Checked before a rule is kept, because a bad one does not fail alone:
/// mihomo refuses the whole config, so the tunnel stops working rather than
/// the rule being skipped.
pub fn validate(kind: Kind, value: &str) -> Option<Invalid> {
    let value = value.trim();
    if value.is_empty() {
        return Some(Invalid::Empty);
    }
    // mihomo splits a rule on commas, so one in the value silently turns it
    // into a different rule.
    if value.contains(',') {
        return Some(Invalid::ContainsComma);
    }

    let port = |p: &str| p.parse::<u32>().ok().filter(|p| (1..=65535).contains(p));
    // "443", a range "1000-2000", or several joined by "/".
    let ports = |value: &str| {
        value.split('/').all(|part| match part.split_once('-') {
            Some((low, high)) => matches!((port(low), port(high)), (Some(l), Some(h)) if l <= h),
            None => port(part).is_some(),
        })
    };
    match kind {
        Kind::ProcessNameRegex | Kind::ProcessPathRegex | Kind::DomainRegex => {
            regex::Regex::new(value)
                .err()
                .map(|e| Invalid::BadRegex(e.to_string()))
        }
        Kind::DstPort | Kind::SrcPort if !ports(value) => Some(Invalid::BadPort),
        Kind::IpCidr | Kind::SrcIpCidr if !is_cidr(value, false) => Some(Invalid::BadCidr),
        Kind::IpCidr6 if !is_cidr(value, true) => Some(Invalid::BadCidr),
        Kind::IpAsn if !value.parse::<u32>().is_ok_and(|asn| asn > 0) => Some(Invalid::BadAsn),
        Kind::Network if !matches!(value.to_lowercase().as_str(), "tcp" | "udp") => {
            Some(Invalid::BadNetwork)
        }
        _ => None,
    }
}

/// A real address and a prefix in its own family's range — `999.1.1.1/24`
/// passes a shape check and is refused by the core, which refuses the config.
fn is_cidr(value: &str, v6_only: bool) -> bool {
    let Some((address, bits)) = value.split_once('/') else {
        return false;
    };
    let (Ok(address), Ok(bits)) = (address.parse::<IpAddr>(), bits.parse::<u32>()) else {
        return false;
    };
    if v6_only && address.is_ipv4() {
        return false;
    }
    bits <= if address.is_ipv4() { 32 } else { 128 }
}

/// Places the user's rules around the subscription's.
///
/// Overrides go before everything. Extensions go after the subscription's
/// rules but *before* its catch-all `MATCH`: appended after it, as the grammar
/// would literally have it, they could never match anything.
///
/// A rule is left out rather than written when it is switched off, when its
/// value no longer validates, when it needs a process and there is no TUN to
/// find one, or when it points at a group the subscription no longer has —
/// mihomo refuses a whole config over one rule naming a proxy it does not
/// know, so a refresh that dropped a group would take the tunnel down with it.
pub fn place(
    own: &[RoutingRule],
    rules: &[String],
    targets: &HashSet<String>,
    processes: bool,
) -> Vec<String> {
    let usable: Vec<&RoutingRule> = own
        .iter()
        .filter(|r| {
            r.enabled
                && targets.contains(&r.target)
                && validate(r.kind, &r.value).is_none()
                && (processes || !r.kind.needs_process_matching())
        })
        .collect();
    let lines = |priority| {
        usable
            .iter()
            .filter(move |r| r.priority == priority)
            .map(|r| r.line())
    };

    let mut out: Vec<String> = lines(Priority::Override).collect();
    let catch_all = rules
        .last()
        .filter(|last| last.trim().to_uppercase().starts_with("MATCH,"));
    let body = if catch_all.is_some() {
        &rules[..rules.len() - 1]
    } else {
        rules
    };
    out.extend_from_slice(body);
    out.extend(lines(Priority::Extend));
    out.extend(catch_all.cloned());
    out
}

/// One of the subscription's own rules, split into what a table shows.
///
/// Parsed rather than split on commas: a logical rule —
/// `AND,((DOMAIN,x),(NETWORK,udp)),Group` — carries commas inside its
/// parentheses, and a trailing `no-resolve` is a parameter, not the target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileRule {
    pub kind: String,
    pub value: String,
    pub target: String,
}

impl ProfileRule {
    pub fn parse(line: &str) -> ProfileRule {
        let mut fields = Vec::new();
        let mut current = String::new();
        let mut depth = 0i32;
        for c in line.chars() {
            match c {
                '(' => {
                    depth += 1;
                    current.push(c);
                }
                ')' => {
                    depth -= 1;
                    current.push(c);
                }
                ',' if depth == 0 => fields.push(std::mem::take(&mut current).trim().to_string()),
                _ => current.push(c),
            }
        }
        fields.push(current.trim().to_string());

        // Parameters the core accepts after the target.
        while fields.len() > 2
            && fields
                .last()
                .is_some_and(|f| matches!(f.to_lowercase().as_str(), "no-resolve" | "src"))
        {
            fields.pop();
        }
        let kind = fields.first().map(|k| k.to_uppercase()).unwrap_or_default();
        if fields.len() >= 3 {
            ProfileRule {
                kind,
                value: fields[1..fields.len() - 1].join(","),
                target: fields[fields.len() - 1].clone(),
            }
        } else {
            // `MATCH,Group` — nothing to match on.
            ProfileRule {
                kind,
                value: String::new(),
                target: fields.get(1).cloned().unwrap_or_default(),
            }
        }
    }
}

/// A rule from the apps screen of versions before 0.12 — an app switch, or a
/// rule written beside them — kept only so it can be read once and carried
/// over into the user's own rules.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SplitRule {
    pub id: Uuid,
    pub kind: Kind,
    pub value: String,
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_executable: Option<String>,
}

/// The apps screen's rules as rules of the user's own, doing what they did.
///
/// - `except` sent what they matched around the tunnel: rules to `DIRECT`.
/// - `only` sent what they matched through it: rules to `group`, the group the
///   subscription routes through. The rest of the traffic now follows the
///   subscription's rules rather than going direct — the one thing a rule
///   cannot say.
/// - `all` ignored them, so they arrive switched off, to read and reuse.
pub fn carried_over(rules: &[SplitRule], mode: SplitMode, group: &str) -> Vec<RoutingRule> {
    rules
        .iter()
        .filter(|r| !r.value.trim().is_empty())
        .map(|r| RoutingRule {
            id: r.id,
            kind: r.kind,
            value: r.value.trim().to_string(),
            target: if mode == SplitMode::Only {
                group
            } else {
                DIRECT
            }
            .to_string(),
            priority: Priority::Override,
            enabled: r.enabled && mode != SplitMode::All,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(kind: Kind, value: &str, target: &str, priority: Priority) -> RoutingRule {
        RoutingRule::new(kind, value, target, priority)
    }

    #[test]
    fn a_rule_writes_mihomos_grammar() {
        let r = rule(Kind::ProcessName, "chrome.exe", DIRECT, Priority::Override);
        assert_eq!(r.line(), "PROCESS-NAME,chrome.exe,DIRECT");
        assert_eq!(Kind::IpCidr.to_string(), "IP-CIDR");
    }

    #[test]
    fn address_rules_carry_no_resolve_and_nothing_else_does() {
        for kind in Kind::ALL {
            let line = rule(*kind, kind.placeholder(), DIRECT, Priority::Override).line();
            let expected = matches!(
                kind,
                Kind::IpCidr | Kind::IpCidr6 | Kind::IpAsn | Kind::Geoip
            );
            assert_eq!(line.ends_with(",no-resolve"), expected, "{line}");
        }
    }

    #[test]
    fn only_process_kinds_need_the_core_to_identify_a_process() {
        for kind in Kind::ALL {
            assert_eq!(
                kind.needs_process_matching(),
                kind.token().starts_with("PROCESS-"),
                "{kind}"
            );
        }
    }

    #[test]
    fn empty_values_and_commas_are_refused_in_every_kind() {
        for kind in Kind::ALL {
            assert_eq!(validate(*kind, "  "), Some(Invalid::Empty), "{kind}");
            assert_eq!(
                validate(*kind, "a,b"),
                Some(Invalid::ContainsComma),
                "{kind}"
            );
        }
    }

    #[test]
    fn every_placeholder_is_itself_valid() {
        // A placeholder that would be refused teaches the user a bad example.
        for kind in Kind::ALL {
            assert_eq!(validate(*kind, kind.placeholder()), None, "{kind}");
        }
    }

    #[test]
    fn regex_kinds_compile_their_pattern() {
        assert_eq!(validate(Kind::DomainRegex, r"^.*\.example\.com$"), None);
        assert!(matches!(
            validate(Kind::DomainRegex, "([unclosed"),
            Some(Invalid::BadRegex(_))
        ));
        assert!(matches!(
            validate(Kind::ProcessNameRegex, "*bad"),
            Some(Invalid::BadRegex(_))
        ));
    }

    #[test]
    fn ports_take_single_values_ranges_and_lists() {
        for good in ["443", "1", "65535", "1000-2000", "80/443/8000-9000"] {
            assert_eq!(validate(Kind::DstPort, good), None, "{good}");
            assert_eq!(validate(Kind::SrcPort, good), None, "{good}");
        }
        for bad in ["0", "65536", "http", "-1", "2000-1000", "80/", "1-2-3"] {
            assert_eq!(
                validate(Kind::DstPort, bad),
                Some(Invalid::BadPort),
                "{bad}"
            );
        }
    }

    #[test]
    fn cidrs_are_checked_as_addresses_in_their_own_family() {
        assert_eq!(validate(Kind::IpCidr, "192.168.1.0/24"), None);
        assert_eq!(validate(Kind::IpCidr, "2001:db8::/64"), None);
        assert_eq!(validate(Kind::SrcIpCidr, "10.0.0.0/8"), None);
        assert_eq!(
            validate(Kind::IpCidr, "999.1.1.1/24"),
            Some(Invalid::BadCidr)
        );
        assert_eq!(
            validate(Kind::IpCidr, "10.0.0.0/64"),
            Some(Invalid::BadCidr)
        );
        assert_eq!(
            validate(Kind::IpCidr, "192.168.1.0"),
            Some(Invalid::BadCidr)
        );
        // IP-CIDR6 is for v6 addresses only.
        assert_eq!(validate(Kind::IpCidr6, "2001:db8::/32"), None);
        assert_eq!(
            validate(Kind::IpCidr6, "10.0.0.0/8"),
            Some(Invalid::BadCidr)
        );
    }

    #[test]
    fn asns_and_networks_are_checked() {
        assert_eq!(validate(Kind::IpAsn, "13335"), None);
        assert_eq!(validate(Kind::IpAsn, "0"), Some(Invalid::BadAsn));
        assert_eq!(validate(Kind::IpAsn, "AS13335"), Some(Invalid::BadAsn));
        assert_eq!(validate(Kind::Network, "UDP"), None);
        assert_eq!(validate(Kind::Network, "icmp"), Some(Invalid::BadNetwork));
    }

    fn targets() -> HashSet<String> {
        [DIRECT, REJECT, "Proxy"]
            .into_iter()
            .map(String::from)
            .collect()
    }

    #[test]
    fn overrides_go_first_and_extensions_before_the_catch_all() {
        let own = vec![
            rule(Kind::Domain, "a.com", DIRECT, Priority::Override),
            rule(Kind::Domain, "b.com", "Proxy", Priority::Extend),
            rule(Kind::DstPort, "25", REJECT, Priority::Override),
        ];
        let subscription = vec!["GEOIP,ru,DIRECT".to_string(), "MATCH,Proxy".to_string()];
        assert_eq!(
            place(&own, &subscription, &targets(), true),
            vec![
                "DOMAIN,a.com,DIRECT",
                "DST-PORT,25,REJECT",
                "GEOIP,ru,DIRECT",
                "DOMAIN,b.com,Proxy",
                "MATCH,Proxy",
            ]
        );
    }

    #[test]
    fn a_rule_that_cannot_be_written_is_left_out_rather_than_breaking_the_config() {
        let mut off = rule(Kind::Domain, "off.com", DIRECT, Priority::Override);
        off.enabled = false;
        let own = vec![
            off,
            rule(Kind::Domain, "gone.com", "Old group", Priority::Override),
            rule(Kind::IpCidr, "999.1.1.1/8", DIRECT, Priority::Override),
            rule(Kind::ProcessName, "a.exe", DIRECT, Priority::Override),
            rule(Kind::Domain, "kept.com", DIRECT, Priority::Override),
        ];
        let subscription = vec!["MATCH,Proxy".to_string()];
        assert_eq!(
            place(&own, &subscription, &targets(), false),
            vec!["DOMAIN,kept.com,DIRECT", "MATCH,Proxy"],
            "no TUN, so no process rule either"
        );
        // Without a catch-all, extensions simply follow.
        let ext = vec![rule(Kind::Domain, "x.com", DIRECT, Priority::Extend)];
        assert_eq!(
            place(&ext, &["GEOIP,ru,DIRECT".to_string()], &targets(), true),
            vec!["GEOIP,ru,DIRECT", "DOMAIN,x.com,DIRECT"]
        );
    }

    #[test]
    fn subscription_rules_are_parsed_not_split() {
        let logical = ProfileRule::parse("AND,((DOMAIN,x.com),(NETWORK,udp)),Proxy");
        assert_eq!(logical.kind, "AND");
        assert_eq!(logical.value, "((DOMAIN,x.com),(NETWORK,udp))");
        assert_eq!(logical.target, "Proxy");

        let resolve = ProfileRule::parse("IP-CIDR,10.0.0.0/8,DIRECT,no-resolve");
        assert_eq!(
            (resolve.value.as_str(), resolve.target.as_str()),
            ("10.0.0.0/8", "DIRECT")
        );

        let catch_all = ProfileRule::parse("MATCH,Proxy");
        assert_eq!(
            (catch_all.kind.as_str(), catch_all.value.as_str()),
            ("MATCH", "")
        );
        assert_eq!(catch_all.target, "Proxy");
    }

    #[test]
    fn the_apps_screen_carries_over_doing_what_it_did() {
        let old = vec![SplitRule {
            id: Uuid::new_v4(),
            kind: Kind::ProcessName,
            value: "Telegram.exe".into(),
            enabled: true,
            app_executable: Some("Telegram.exe".into()),
        }];
        let except = carried_over(&old, SplitMode::Except, "Proxy");
        assert_eq!(
            (except[0].target.as_str(), except[0].enabled),
            (DIRECT, true)
        );
        let only = carried_over(&old, SplitMode::Only, "Proxy");
        assert_eq!((only[0].target.as_str(), only[0].enabled), ("Proxy", true));
        let all = carried_over(&old, SplitMode::All, "Proxy");
        assert!(!all[0].enabled, "kept to read, switched off");
        assert_eq!(all[0].id, old[0].id);
    }

    #[test]
    fn an_old_split_rule_still_reads() {
        let json = r#"{"id":"3f1c0f6e-6c1f-4b8e-9a3a-2b1c2d3e4f50","kind":"ProcessName","value":"a.exe","enabled":true,"app_executable":"a.exe"}"#;
        let old: SplitRule = serde_json::from_str(json).expect("reads");
        assert_eq!(old.kind, Kind::ProcessName);
    }
}
