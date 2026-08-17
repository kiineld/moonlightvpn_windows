//! The types the whole client is written against.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::country;

/// A node the tunnel can select, as it appears in the mihomo config the panel
/// serves. `name` is the identity: it is what the RESTful API takes to switch a
/// selector, so it must round-trip verbatim, flag emoji and all.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Node {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
    /// Latency in milliseconds, from the last probe. `None` means never
    /// measured — which the UI shows as `n/a`, not as `0 ms`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latency: Option<u32>,
    /// Whether a probe has actually finished for this node.
    ///
    /// Separate from `latency` because `None` alone cannot tell "never measured"
    /// from "measured, no answer", and the two want opposite words: a dash for
    /// the first, `n/a` for the second. Reporting `n/a` before anything has been
    /// probed says the server did not answer a question nobody asked it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub probed: bool,
    /// True for a `url-test`, `fallback` or `load-balance` group the panel put
    /// in its selector. Those are choices the operator built deliberately — a
    /// balancer across several nodes, or an auto-picker — and hiding them
    /// leaves the user picking raw nodes the panel never meant to offer
    /// directly.
    #[serde(default)]
    pub is_group: bool,
    /// How the panel writes this node's transport — "VLESS Reality",
    /// "Hysteria2 TLS". Read from the subscription rather than from the API,
    /// which reports only the bare type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol_label: Option<String>,
}

/// The Unicode block flag emoji are built from. Two regional indicators are
/// just `A`–`Z` shifted into it, which is what lets the ISO code fall straight
/// out of a flag with no table to keep up to date.
const REGIONAL_INDICATOR_A: u32 = 0x1F1E6;
const REGIONAL_INDICATOR_Z: u32 = 0x1F1FF;

fn is_regional_indicator(c: char) -> bool {
    (REGIONAL_INDICATOR_A..=REGIONAL_INDICATOR_Z).contains(&(c as u32))
}

impl Node {
    pub fn new(name: impl Into<String>, kind: impl Into<String>) -> Self {
        Node {
            name: name.into(),
            kind: kind.into(),
            server: None,
            latency: None,
            probed: false,
            is_group: false,
            protocol_label: None,
        }
    }

    pub fn id(&self) -> &str {
        &self.name
    }

    /// True for a group that picks a node by latency on its own — `url-test`
    /// or `fallback`. That is the same job the app's own "Авто" row does, so
    /// when a panel offers one there is no reason to show both.
    pub fn is_auto_picker(&self) -> bool {
        self.is_group && matches!(self.kind.to_lowercase().as_str(), "urltest" | "fallback")
    }

    fn flag_chars(&self) -> Vec<char> {
        self.name
            .chars()
            .take_while(|c| is_regional_indicator(*c))
            .collect()
    }

    /// The flag emoji a panel conventionally prefixes to a node name, split off
    /// so the design's separate flag column can render it.
    ///
    /// `None` for an entry with no flag — a cross-country balancer or an
    /// auto-picker — so the row can show a mark instead of an invented one.
    pub fn flag(&self) -> Option<String> {
        let chars = self.flag_chars();
        (chars.len() == 2).then(|| chars.into_iter().collect())
    }

    /// The ISO 3166-1 alpha-2 code the flag stands for.
    pub fn region_code(&self) -> Option<String> {
        let chars = self.flag_chars();
        if chars.len() != 2 {
            return None;
        }
        Some(
            chars
                .into_iter()
                .map(|c| {
                    char::from_u32(c as u32 - REGIONAL_INDICATOR_A + 'A' as u32).unwrap_or('?')
                })
                .collect(),
        )
    }

    /// The country the flag stands for, in the app's language.
    pub fn country(&self, locale: AppLocale) -> Option<&'static str> {
        country::name(&self.region_code()?, locale)
    }

    /// The node name with the leading flag removed.
    pub fn title(&self) -> String {
        let stripped: String = self
            .name
            .chars()
            .skip_while(|c| is_regional_indicator(*c))
            .collect();
        let stripped = stripped.trim();
        if stripped.is_empty() {
            self.name.clone()
        } else {
            stripped.to_string()
        }
    }

    /// "Швеция · VLESS Reality" — the row's second line. Whatever is known,
    /// joined; a group with no flag simply shows its transport.
    pub fn subtitle(&self, locale: AppLocale) -> String {
        let mut parts: Vec<&str> = Vec::new();
        if let Some(c) = self.country(locale) {
            parts.push(c);
        }
        if let Some(p) = self.protocol_label.as_deref() {
            parts.push(p);
        }
        parts.join(" · ")
    }
}

/// What the panel reports about the subscription itself.
///
/// Every field is optional because a missing field has to read as *unknown*
/// rather than as zero — a subscription whose panel omits `total` is unlimited,
/// and showing "0 GB" for it would be a lie the user acts on.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SubscriptionInfo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upload: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<i64>,
    /// Unix seconds, kept as an offset date so the UI can render it in the
    /// user's own zone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expire: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub devices_used: Option<u32>,
}

impl SubscriptionInfo {
    pub fn used(&self) -> Option<i64> {
        if self.upload.is_none() && self.download.is_none() {
            return None;
        }
        Some(self.upload.unwrap_or(0) + self.download.unwrap_or(0))
    }

    pub fn expire_date(&self) -> Option<OffsetDateTime> {
        OffsetDateTime::from_unix_timestamp(self.expire?).ok()
    }

    pub fn days_left(&self) -> Option<i64> {
        let expire = self.expire?;
        let now = OffsetDateTime::now_utc().unix_timestamp();
        let seconds = expire - now;
        Some(if seconds <= 0 {
            0
        } else {
            (seconds as f64 / 86_400.0).ceil() as i64
        })
    }

    /// Fraction of the quota consumed, 0…1. `None` when the plan is unlimited.
    pub fn used_fraction(&self) -> Option<f64> {
        let total = self.total?;
        if total <= 0 {
            return None;
        }
        let used = self.used()?;
        Some((used as f64 / total as f64).clamp(0.0, 1.0))
    }

    pub fn is_active(&self) -> bool {
        match self.expire {
            None => true,
            Some(expire) => expire > OffsetDateTime::now_utc().unix_timestamp(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ConnectionState {
    #[default]
    Disconnected,
    Connecting,
    Connected,
    Disconnecting,
    Failed(String),
}

impl ConnectionState {
    pub fn is_connected(&self) -> bool {
        *self == ConnectionState::Connected
    }

    pub fn is_busy(&self) -> bool {
        matches!(
            self,
            ConnectionState::Connecting | ConnectionState::Disconnecting
        )
    }
}

/// How traffic reaches the tunnel.
///
/// These are genuinely different mechanisms, not a preference: system proxy
/// rewrites the machine's proxy settings and only captures apps that honour
/// them, while TUN takes a virtual interface and captures everything. Only TUN
/// can enforce per-app rules, which is why the split-tunnel screen is inert
/// without it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TunnelMode {
    #[default]
    SystemProxy,
    Tun,
}

/// Which traffic goes through the tunnel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SplitMode {
    /// Everything.
    #[default]
    All,
    /// Only the selected processes; everything else goes direct.
    Only,
    /// The selected processes go direct; everything else is tunnelled.
    Except,
}

/// An installed application, addressed by the executable name mihomo's
/// `PROCESS-NAME` rules match on.
///
/// On Windows that is the file name with its extension — `chrome.exe`, not
/// `chrome` — because that is the string the core reads back out of the
/// process table. Dropping the extension produces rules that are written,
/// accepted and never match.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AppEntry {
    pub name: String,
    pub executable: String,
    pub path: String,
}

impl AppEntry {
    pub fn id(&self) -> &str {
        &self.executable
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AppLocale {
    #[default]
    Ru,
    En,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(name: &str) -> Node {
        Node::new(name, "vless")
    }

    #[test]
    fn a_flag_prefix_is_split_off_the_title() {
        let n = node("🇸🇪 Stockholm 01");
        assert_eq!(n.flag().as_deref(), Some("🇸🇪"));
        assert_eq!(n.title(), "Stockholm 01");
    }

    #[test]
    fn a_node_with_no_flag_keeps_its_whole_name() {
        let n = node("Balancer EU");
        assert_eq!(n.flag(), None);
        assert_eq!(n.title(), "Balancer EU");
    }

    #[test]
    fn a_name_that_is_only_a_flag_falls_back_to_the_raw_name() {
        // Stripping would leave nothing, and an empty row reads as a bug.
        let n = node("🇩🇪");
        assert_eq!(n.title(), "🇩🇪");
    }

    #[test]
    fn the_iso_code_falls_out_of_the_flag() {
        assert_eq!(node("🇸🇪 x").region_code().as_deref(), Some("SE"));
        assert_eq!(node("🇺🇸 x").region_code().as_deref(), Some("US"));
        assert_eq!(node("🇳🇱 x").region_code().as_deref(), Some("NL"));
    }

    #[test]
    fn a_single_regional_indicator_is_not_a_flag() {
        // One indicator is not a country; treating it as one invents a code.
        let n = node("\u{1F1F8}only");
        assert_eq!(n.flag(), None);
    }

    #[test]
    fn country_is_localised_both_ways() {
        let n = node("🇸🇪 Stockholm");
        assert_eq!(n.country(AppLocale::Ru), Some("Швеция"));
        assert_eq!(n.country(AppLocale::En), Some("Sweden"));
    }

    #[test]
    fn subtitle_joins_only_what_is_known() {
        let mut n = node("🇸🇪 Stockholm");
        n.protocol_label = Some("VLESS Reality".into());
        assert_eq!(n.subtitle(AppLocale::Ru), "Швеция · VLESS Reality");

        let mut bare = node("Balancer");
        bare.protocol_label = Some("VLESS Reality".into());
        assert_eq!(bare.subtitle(AppLocale::Ru), "VLESS Reality");

        assert_eq!(node("Balancer").subtitle(AppLocale::Ru), "");
    }

    #[test]
    fn auto_pickers_are_only_latency_groups() {
        let mut g = Node::new("Auto", "URLTest");
        g.is_group = true;
        assert!(g.is_auto_picker());

        let mut lb = Node::new("Spread", "LoadBalance");
        lb.is_group = true;
        assert!(!lb.is_auto_picker(), "a balancer is not an auto-picker");

        let mut plain = Node::new("Auto", "urltest");
        plain.is_group = false;
        assert!(!plain.is_auto_picker(), "a bare node is never a picker");
    }

    #[test]
    fn used_is_unknown_rather_than_zero_when_the_panel_omits_both() {
        let info = SubscriptionInfo::default();
        assert_eq!(info.used(), None);

        let partial = SubscriptionInfo {
            download: Some(500),
            ..Default::default()
        };
        assert_eq!(partial.used(), Some(500));
    }

    #[test]
    fn an_absent_total_means_unlimited_not_full() {
        let info = SubscriptionInfo {
            download: Some(1_000),
            ..Default::default()
        };
        assert_eq!(info.used_fraction(), None);

        // A zero total is the same claim written differently.
        let zero = SubscriptionInfo {
            download: Some(1_000),
            total: Some(0),
            ..Default::default()
        };
        assert_eq!(zero.used_fraction(), None);
    }

    #[test]
    fn used_fraction_is_clamped() {
        let over = SubscriptionInfo {
            download: Some(300),
            total: Some(100),
            ..Default::default()
        };
        assert_eq!(over.used_fraction(), Some(1.0));
    }

    #[test]
    fn no_expiry_is_active_and_has_no_day_count() {
        let info = SubscriptionInfo::default();
        assert!(info.is_active());
        assert_eq!(info.days_left(), None);
    }

    #[test]
    fn a_past_expiry_is_inactive_with_zero_days() {
        let info = SubscriptionInfo {
            expire: Some(OffsetDateTime::now_utc().unix_timestamp() - 60),
            ..Default::default()
        };
        assert!(!info.is_active());
        assert_eq!(info.days_left(), Some(0));
    }

    #[test]
    fn days_left_rounds_up_so_a_part_day_still_counts() {
        let info = SubscriptionInfo {
            expire: Some(OffsetDateTime::now_utc().unix_timestamp() + 86_400 + 3_600),
            ..Default::default()
        };
        assert_eq!(info.days_left(), Some(2));
    }

    #[test]
    fn connection_state_busy_covers_both_transitions() {
        assert!(ConnectionState::Connecting.is_busy());
        assert!(ConnectionState::Disconnecting.is_busy());
        assert!(!ConnectionState::Connected.is_busy());
        assert!(!ConnectionState::Failed("x".into()).is_busy());
    }
}
