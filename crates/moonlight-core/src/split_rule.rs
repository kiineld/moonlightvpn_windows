//! One user-authored routing rule.
//!
//! The app list on the split screen is a convenience over `PROCESS-NAME`; this
//! is the general form, so a rule can also match a process the scanner never
//! found, a domain, a regex, a CIDR or a port. Every kind here is validated
//! against the real core in the test suite, both as a plain rule and inside the
//! `SUB-RULE` matcher that "only these" mode uses — mihomo accepts different
//! grammars in the two positions, and a rule that only works in one is a config
//! the core refuses to load.

use std::fmt;
use std::net::IpAddr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Kind {
    ProcessName,
    ProcessNameRegex,
    ProcessPath,
    ProcessPathRegex,
    Domain,
    DomainSuffix,
    DomainKeyword,
    DomainRegex,
    IpCidr,
    Geosite,
    Geoip,
    DstPort,
}

impl Kind {
    pub const ALL: &'static [Kind] = &[
        Kind::ProcessName,
        Kind::ProcessNameRegex,
        Kind::ProcessPath,
        Kind::ProcessPathRegex,
        Kind::Domain,
        Kind::DomainSuffix,
        Kind::DomainKeyword,
        Kind::DomainRegex,
        Kind::IpCidr,
        Kind::Geosite,
        Kind::Geoip,
        Kind::DstPort,
    ];

    /// The token mihomo's rule grammar uses.
    pub fn token(self) -> &'static str {
        match self {
            Kind::ProcessName => "PROCESS-NAME",
            Kind::ProcessNameRegex => "PROCESS-NAME-REGEX",
            Kind::ProcessPath => "PROCESS-PATH",
            Kind::ProcessPathRegex => "PROCESS-PATH-REGEX",
            Kind::Domain => "DOMAIN",
            Kind::DomainSuffix => "DOMAIN-SUFFIX",
            Kind::DomainKeyword => "DOMAIN-KEYWORD",
            Kind::DomainRegex => "DOMAIN-REGEX",
            Kind::IpCidr => "IP-CIDR",
            Kind::Geosite => "GEOSITE",
            Kind::Geoip => "GEOIP",
            Kind::DstPort => "DST-PORT",
        }
    }

    /// Whether the core has to identify the process behind a connection to
    /// evaluate this. Only TUN mode can — under a system proxy the core is
    /// handed a socket with no process behind it. Domain and address rules work
    /// in both modes, which is why the warning is per-rule rather than
    /// per-screen.
    pub fn needs_process_matching(self) -> bool {
        matches!(
            self,
            Kind::ProcessName | Kind::ProcessNameRegex | Kind::ProcessPath | Kind::ProcessPathRegex
        )
    }

    /// Address rules carry `no-resolve` so a domain is not resolved just to
    /// test it against a CIDR — that would send a DNS query for every
    /// connection and defeat the point of matching on address.
    fn wants_no_resolve(self) -> bool {
        matches!(self, Kind::IpCidr | Kind::Geoip)
    }

    /// The example shown in the empty field.
    ///
    /// The process examples are Windows-shaped: mihomo reads the executable
    /// name back out of the process table, which on Windows carries its `.exe`
    /// extension. An example without one teaches a rule that is accepted and
    /// never matches.
    pub fn placeholder(self) -> &'static str {
        match self {
            Kind::ProcessName => "Telegram.exe",
            Kind::ProcessNameRegex => "(?i).*chrome.*",
            Kind::ProcessPath => r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            Kind::ProcessPathRegex => r"(?i).*\\steam.*",
            Kind::Domain => "example.com",
            Kind::DomainSuffix => "openai.com",
            Kind::DomainKeyword => "google",
            Kind::DomainRegex => r"^.*\.discord\.(com|gg)$",
            Kind::IpCidr => "192.168.1.0/24",
            Kind::Geosite => "youtube",
            Kind::Geoip => "ru",
            Kind::DstPort => "443",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SplitRule {
    pub id: Uuid,
    pub kind: Kind,
    pub value: String,
    pub enabled: bool,
    /// Set for rules the app list generated, so removing an app removes its
    /// rule and a hand-written rule for the same process is left alone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_executable: Option<String>,
}

impl SplitRule {
    pub fn new(kind: Kind, value: impl Into<String>) -> Self {
        SplitRule {
            id: Uuid::new_v4(),
            kind,
            value: value.into(),
            enabled: true,
            app_executable: None,
        }
    }

    pub fn for_app(executable: impl Into<String>) -> Self {
        let executable = executable.into();
        SplitRule {
            id: Uuid::new_v4(),
            kind: Kind::ProcessName,
            value: executable.clone(),
            enabled: true,
            app_executable: Some(executable),
        }
    }

    pub fn is_from_app_list(&self) -> bool {
        self.app_executable.is_some()
    }

    /// The rule as mihomo's rule grammar writes it, pointed at `target`.
    pub fn line(&self, target: &str) -> String {
        let suffix = if self.kind.wants_no_resolve() {
            ",no-resolve"
        } else {
            ""
        };
        format!("{},{},{}{}", self.kind.token(), self.value, target, suffix)
    }

    /// The rule as a `SUB-RULE` matcher expression.
    ///
    /// `no-resolve` is *not* included: it is a rule parameter, and the matcher
    /// position does not take one.
    pub fn matcher(&self) -> String {
        format!("({},{})", self.kind.token(), self.value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invalid {
    Empty,
    ContainsComma,
    BadRegex(String),
    BadPort,
    BadCidr,
}

impl fmt::Display for Invalid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Invalid::Empty => write!(f, "The rule has no value"),
            Invalid::ContainsComma => write!(
                f,
                "A value cannot contain a comma — mihomo splits rules on it"
            ),
            Invalid::BadRegex(why) => write!(f, "Not a valid regular expression: {why}"),
            Invalid::BadPort => write!(f, "Not a valid port"),
            Invalid::BadCidr => write!(f, "Not a valid CIDR block, e.g. 192.168.1.0/24"),
        }
    }
}

impl std::error::Error for Invalid {}

/// Checked before a rule can be added, because a bad one does not fail alone:
/// mihomo refuses the whole config, so the tunnel stops working rather than the
/// rule being skipped.
pub fn validate(kind: Kind, value: &str) -> Option<Invalid> {
    let value = value.trim();
    if value.is_empty() {
        return Some(Invalid::Empty);
    }
    // mihomo parses a rule by splitting on commas, so a comma anywhere in the
    // value silently turns into a different rule.
    if value.contains(',') {
        return Some(Invalid::ContainsComma);
    }

    match kind {
        Kind::ProcessNameRegex | Kind::ProcessPathRegex | Kind::DomainRegex => {
            if let Err(error) = regex::Regex::new(value) {
                return Some(Invalid::BadRegex(error.to_string()));
            }
        }
        Kind::DstPort => match value.parse::<u32>() {
            Ok(port) if (1..=65535).contains(&port) => {}
            _ => return Some(Invalid::BadPort),
        },
        Kind::IpCidr => {
            // Stricter than the macOS client, which only counted the slash and
            // range-checked the prefix. An address like `999.1.1.1/24` passed
            // there and was refused by the core — which refuses the whole
            // config, so the tunnel stopped rather than the rule.
            let Some((address, bits)) = value.split_once('/') else {
                return Some(Invalid::BadCidr);
            };
            let Ok(address) = address.parse::<IpAddr>() else {
                return Some(Invalid::BadCidr);
            };
            let Ok(bits) = bits.parse::<u32>() else {
                return Some(Invalid::BadCidr);
            };
            let max = if address.is_ipv4() { 32 } else { 128 };
            if bits > max {
                return Some(Invalid::BadCidr);
            }
        }
        _ => {}
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rule_writes_mihomos_grammar() {
        let rule = SplitRule::new(Kind::ProcessName, "chrome.exe");
        assert_eq!(rule.line("DIRECT"), "PROCESS-NAME,chrome.exe,DIRECT");
    }

    #[test]
    fn address_rules_carry_no_resolve() {
        // Without it every connection triggers a DNS lookup just to test an
        // address rule, which defeats matching on address at all.
        let cidr = SplitRule::new(Kind::IpCidr, "10.0.0.0/8");
        assert_eq!(cidr.line("DIRECT"), "IP-CIDR,10.0.0.0/8,DIRECT,no-resolve");

        let geoip = SplitRule::new(Kind::Geoip, "ru");
        assert_eq!(geoip.line("DIRECT"), "GEOIP,ru,DIRECT,no-resolve");
    }

    #[test]
    fn other_kinds_do_not_carry_no_resolve() {
        for kind in Kind::ALL.iter().filter(|k| !k.wants_no_resolve()) {
            let rule = SplitRule::new(*kind, "x");
            assert!(
                !rule.line("DIRECT").ends_with(",no-resolve"),
                "{} should not take no-resolve",
                kind.token()
            );
        }
    }

    #[test]
    fn the_matcher_position_drops_no_resolve() {
        // no-resolve is a rule parameter; a SUB-RULE matcher does not take one,
        // and including it makes the core refuse the config.
        let cidr = SplitRule::new(Kind::IpCidr, "10.0.0.0/8");
        assert_eq!(cidr.matcher(), "(IP-CIDR,10.0.0.0/8)");
        assert!(!cidr.matcher().contains("no-resolve"));
    }

    #[test]
    fn every_kind_produces_both_forms() {
        for kind in Kind::ALL {
            let rule = SplitRule::new(*kind, kind.placeholder());
            let line = rule.line("DIRECT");
            assert!(line.starts_with(kind.token()), "{}", kind.token());
            assert!(rule.matcher().starts_with(&format!("({}", kind.token())));
        }
    }

    #[test]
    fn only_process_kinds_need_the_core_to_identify_a_process() {
        assert!(Kind::ProcessName.needs_process_matching());
        assert!(Kind::ProcessNameRegex.needs_process_matching());
        assert!(Kind::ProcessPath.needs_process_matching());
        assert!(Kind::ProcessPathRegex.needs_process_matching());

        for kind in [
            Kind::Domain,
            Kind::DomainSuffix,
            Kind::DomainKeyword,
            Kind::DomainRegex,
            Kind::IpCidr,
            Kind::Geosite,
            Kind::Geoip,
            Kind::DstPort,
        ] {
            assert!(
                !kind.needs_process_matching(),
                "{} works under a system proxy too",
                kind.token()
            );
        }
    }

    #[test]
    fn an_empty_value_is_refused() {
        assert_eq!(validate(Kind::Domain, ""), Some(Invalid::Empty));
        assert_eq!(validate(Kind::Domain, "   "), Some(Invalid::Empty));
    }

    #[test]
    fn a_comma_is_refused_in_every_kind() {
        // mihomo splits a rule on commas, so a comma silently produces a
        // different rule rather than a broken one.
        for kind in Kind::ALL {
            assert_eq!(
                validate(*kind, "a,b"),
                Some(Invalid::ContainsComma),
                "{} accepted a comma",
                kind.token()
            );
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
    fn ports_are_range_checked() {
        assert_eq!(validate(Kind::DstPort, "443"), None);
        assert_eq!(validate(Kind::DstPort, "1"), None);
        assert_eq!(validate(Kind::DstPort, "65535"), None);
        assert_eq!(validate(Kind::DstPort, "0"), Some(Invalid::BadPort));
        assert_eq!(validate(Kind::DstPort, "65536"), Some(Invalid::BadPort));
        assert_eq!(validate(Kind::DstPort, "http"), Some(Invalid::BadPort));
        assert_eq!(validate(Kind::DstPort, "-1"), Some(Invalid::BadPort));
    }

    #[test]
    fn cidrs_are_checked_as_addresses_not_just_as_shapes() {
        assert_eq!(validate(Kind::IpCidr, "192.168.1.0/24"), None);
        assert_eq!(validate(Kind::IpCidr, "2001:db8::/32"), None);
        assert_eq!(validate(Kind::IpCidr, "0.0.0.0/0"), None);

        assert_eq!(validate(Kind::IpCidr, "192.168.1.0"), Some(Invalid::BadCidr));
        assert_eq!(
            validate(Kind::IpCidr, "999.1.1.1/24"),
            Some(Invalid::BadCidr),
            "an impossible octet must not reach the core"
        );
        assert_eq!(validate(Kind::IpCidr, "notanip/24"), Some(Invalid::BadCidr));
        assert_eq!(validate(Kind::IpCidr, "10.0.0.0/x"), Some(Invalid::BadCidr));
    }

    #[test]
    fn a_prefix_is_checked_against_its_own_family() {
        // /64 is valid for v6 and nonsense for v4; one range check for both
        // would let the v4 case through.
        assert_eq!(validate(Kind::IpCidr, "2001:db8::/64"), None);
        assert_eq!(validate(Kind::IpCidr, "10.0.0.0/64"), Some(Invalid::BadCidr));
        assert_eq!(validate(Kind::IpCidr, "10.0.0.0/32"), None);
    }

    #[test]
    fn plain_kinds_accept_anything_without_a_comma() {
        assert_eq!(validate(Kind::Domain, "example.com"), None);
        assert_eq!(validate(Kind::Geosite, "youtube"), None);
        assert_eq!(validate(Kind::ProcessName, "chrome.exe"), None);
        assert_eq!(
            validate(Kind::ProcessPath, r"C:\Program Files\app\a.exe"),
            None
        );
    }

    #[test]
    fn every_placeholder_is_itself_valid() {
        // A placeholder that would be rejected teaches the user a bad example.
        for kind in Kind::ALL {
            assert_eq!(
                validate(*kind, kind.placeholder()),
                None,
                "{} has an invalid placeholder: {}",
                kind.token(),
                kind.placeholder()
            );
        }
    }

    #[test]
    fn an_app_rule_remembers_which_app_made_it() {
        let rule = SplitRule::for_app("chrome.exe");
        assert!(rule.is_from_app_list());
        assert_eq!(rule.kind, Kind::ProcessName);
        assert_eq!(rule.value, "chrome.exe");

        assert!(!SplitRule::new(Kind::ProcessName, "chrome.exe").is_from_app_list());
    }
}
