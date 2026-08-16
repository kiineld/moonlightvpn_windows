//! Everything the app remembers between launches, as one JSON document.
//!
//! macOS has `UserDefaults`. Windows's nearest equivalent is the registry, which
//! is the wrong shape for this: the state below is nested (a list of rules, a
//! map of latencies), the registry is flat, and a portable build has to be able
//! to run from a USB stick without leaving a machine-wide trace. So it is a file
//! under `%APPDATA%\Moonlight`, written whole.
//!
//! ## Why latencies are persisted
//!
//! A measurement opens a connection through every node and takes several
//! seconds. Throwing the results away because the user looked at Settings makes
//! the server list useless exactly when they are choosing from it, so they
//! survive a screen change, a reconnect and a relaunch.
//!
//! ## Why the proxy snapshot is persisted
//!
//! The app has to be able to put the machine's proxy settings back after a crash
//! that skipped the disconnect path. Keeping the snapshot only in memory means a
//! crash leaves every browser pointed at a core that is no longer running, which
//! presents to the user as "the internet stopped working" with nothing on screen
//! to connect it to this app.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::{AppLocale, SplitMode, TunnelMode};
use crate::split_rule::SplitRule;
use crate::system_proxy::Snapshot;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Preferences {
    pub subscription_url: Option<String>,
    /// A random UUID minted once. Not a hardware identifier — see
    /// [`crate::subscription::DeviceIdentity`].
    pub hwid: String,
    pub selected_node: Option<String>,
    pub auto_select: bool,
    pub mode: TunnelMode,
    pub split_mode: SplitMode,
    pub split_rules: Vec<SplitRule>,
    /// Node name → last measured latency in milliseconds.
    pub latencies: HashMap<String, u32>,
    pub locale: AppLocale,
    /// `None` follows the system setting.
    pub appearance: Option<String>,
    pub sidebar_collapsed: bool,
    pub launch_at_login: bool,
    /// The machine's proxy settings as they were before this client touched
    /// them, so a crash cannot strand them.
    pub proxy_snapshot: Option<Snapshot>,
    /// The secret the core's API is protected with, minted once per install.
    pub api_secret: String,
    pub controller_port: u16,
    pub mixed_port: u16,
}

impl Default for Preferences {
    fn default() -> Self {
        Preferences {
            subscription_url: None,
            hwid: Uuid::new_v4().to_string(),
            selected_node: None,
            auto_select: true,
            mode: TunnelMode::SystemProxy,
            split_mode: SplitMode::All,
            split_rules: Vec::new(),
            latencies: HashMap::new(),
            locale: AppLocale::Ru,
            appearance: None,
            sidebar_collapsed: false,
            launch_at_login: false,
            proxy_snapshot: None,
            api_secret: Uuid::new_v4().to_string(),
            controller_port: 9797,
            mixed_port: 7897,
        }
    }
}

/// `%APPDATA%\Moonlight`, or the platform equivalent when running the port on a
/// developer's machine.
pub fn support_directory() -> PathBuf {
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Moonlight"))
            .join("Moonlight")
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join(".moonlight")
    }
}

pub fn preferences_path() -> PathBuf {
    support_directory().join("preferences.json")
}

/// Where mihomo keeps its geo databases and cache.
///
/// Not shipped in the installer: the core downloads `GeoSite.dat`/`GeoIP.dat`
/// on demand the first time a config references a `geosite:`/`geoip:` rule,
/// which every panel config does. That costs one download on first connect and
/// saves ~24 MB in the download.
pub fn core_data_directory() -> PathBuf {
    support_directory().join("core")
}

pub fn config_path() -> PathBuf {
    support_directory().join("core.yaml")
}

impl Preferences {
    pub fn load() -> Preferences {
        let path = preferences_path();
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Preferences::default();
        };
        // A preferences file from a newer build, or a half-written one from a
        // crash mid-save, must not stop the app starting. Defaults are always a
        // usable state.
        serde_json::from_str(&text).unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        let directory = support_directory();
        std::fs::create_dir_all(&directory)?;

        // Written to a temporary file and renamed, so a crash mid-write leaves
        // the previous preferences rather than a truncated file. On Windows the
        // rename must go to a path on the same volume, which this is.
        let final_path = preferences_path();
        let temporary = final_path.with_extension("json.tmp");
        std::fs::write(&temporary, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(&temporary, &final_path)
    }

    /// The latency to show for a node, if one was ever measured.
    pub fn latency(&self, node: &str) -> Option<u32> {
        self.latencies.get(node).copied()
    }

    pub fn record_latency(&mut self, node: &str, latency: Option<u32>) {
        match latency {
            Some(value) => {
                self.latencies.insert(node.to_string(), value);
            }
            // A node that stopped answering must lose its old number rather
            // than keep showing a stale one that is no longer true.
            None => {
                self.latencies.remove(node);
            }
        }
    }

    /// Drops measurements for nodes the subscription no longer offers, so a map
    /// does not grow forever across subscription changes.
    pub fn prune_latencies(&mut self, live_nodes: &[String]) {
        self.latencies.retain(|name, _| live_nodes.contains(name));
    }

    /// The rules the app-list toggles contribute, keyed by executable.
    pub fn app_rules(&self) -> Vec<&SplitRule> {
        self.split_rules
            .iter()
            .filter(|r| r.is_from_app_list())
            .collect()
    }

    pub fn toggle_app(&mut self, executable: &str) {
        match self
            .split_rules
            .iter()
            .position(|r| r.app_executable.as_deref() == Some(executable))
        {
            // Only the generated rule is removed; a hand-written rule for the
            // same process is left alone.
            Some(index) => {
                self.split_rules.remove(index);
            }
            None => self.split_rules.push(SplitRule::for_app(executable)),
        }
    }

    pub fn has_app(&self, executable: &str) -> bool {
        self.split_rules
            .iter()
            .any(|r| r.app_executable.as_deref() == Some(executable))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::split_rule::Kind;

    #[test]
    fn defaults_are_a_usable_state() {
        let prefs = Preferences::default();
        assert!(prefs.auto_select);
        assert_eq!(prefs.mode, TunnelMode::SystemProxy);
        assert_eq!(prefs.split_mode, SplitMode::All);
        assert_eq!(prefs.controller_port, 9797);
        assert!(!prefs.hwid.is_empty());
        assert!(!prefs.api_secret.is_empty());
    }

    #[test]
    fn each_install_mints_its_own_identifiers() {
        // A shared hwid would make the panel count every install as one device.
        let a = Preferences::default();
        let b = Preferences::default();
        assert_ne!(a.hwid, b.hwid);
        assert_ne!(a.api_secret, b.api_secret);
    }

    #[test]
    fn the_hwid_is_a_uuid_not_anything_from_the_machine() {
        let prefs = Preferences::default();
        assert!(Uuid::parse_str(&prefs.hwid).is_ok());
    }

    #[test]
    fn a_corrupt_file_falls_back_to_defaults_rather_than_refusing_to_start() {
        let recovered: Preferences = serde_json::from_str("{ truncated").unwrap_or_default();
        assert_eq!(recovered.controller_port, 9797);
    }

    #[test]
    fn an_unknown_field_from_a_newer_build_is_ignored() {
        let json = r#"{"autoSelect": false, "somethingFromTheFuture": 1}"#;
        let prefs: Preferences = serde_json::from_str(json).expect("parses");
        assert!(!prefs.auto_select);
        // And the fields it did not carry still get their defaults.
        assert_eq!(prefs.mixed_port, 7897);
    }

    #[test]
    fn preferences_round_trip() {
        let mut prefs = Preferences {
            subscription_url: Some("https://panel/sub".into()),
            split_rules: vec![SplitRule::new(Kind::Domain, "x.com")],
            ..Default::default()
        };
        prefs.record_latency("Node A", Some(37));

        let json = serde_json::to_string(&prefs).expect("serialises");
        let back: Preferences = serde_json::from_str(&json).expect("deserialises");
        assert_eq!(prefs, back);
    }

    #[test]
    fn a_node_that_stopped_answering_loses_its_stale_number() {
        let mut prefs = Preferences::default();
        prefs.record_latency("Node A", Some(37));
        assert_eq!(prefs.latency("Node A"), Some(37));

        prefs.record_latency("Node A", None);
        assert_eq!(
            prefs.latency("Node A"),
            None,
            "a timeout must not leave the old figure on screen"
        );
    }

    #[test]
    fn pruning_drops_nodes_the_subscription_no_longer_offers() {
        let mut prefs = Preferences::default();
        prefs.record_latency("Old", Some(10));
        prefs.record_latency("Kept", Some(20));

        prefs.prune_latencies(&["Kept".to_string()]);
        assert_eq!(prefs.latency("Old"), None);
        assert_eq!(prefs.latency("Kept"), Some(20));
    }

    #[test]
    fn toggling_an_app_adds_then_removes_its_own_rule() {
        let mut prefs = Preferences::default();
        assert!(!prefs.has_app("chrome.exe"));

        prefs.toggle_app("chrome.exe");
        assert!(prefs.has_app("chrome.exe"));
        assert_eq!(prefs.split_rules.len(), 1);

        prefs.toggle_app("chrome.exe");
        assert!(!prefs.has_app("chrome.exe"));
        assert!(prefs.split_rules.is_empty());
    }

    #[test]
    fn removing_an_app_leaves_a_hand_written_rule_for_the_same_process() {
        let mut prefs = Preferences::default();
        prefs
            .split_rules
            .push(SplitRule::new(Kind::ProcessName, "chrome.exe"));
        prefs.toggle_app("chrome.exe");
        assert_eq!(prefs.split_rules.len(), 2);

        prefs.toggle_app("chrome.exe");
        assert_eq!(prefs.split_rules.len(), 1, "the hand-written rule survives");
        assert!(!prefs.split_rules[0].is_from_app_list());
    }

    #[test]
    fn the_support_directory_is_per_user_not_next_to_the_executable() {
        let directory = support_directory();
        assert!(directory.ends_with("Moonlight") || directory.ends_with(".moonlight"));
    }

    #[test]
    fn a_proxy_snapshot_survives_a_round_trip_so_a_crash_cannot_strand_it() {
        let prefs = Preferences {
            proxy_snapshot: Some(Snapshot {
                enabled: true,
                server: "corp:8080".into(),
                bypass: "<local>".into(),
                auto_config_url: None,
            }),
            ..Default::default()
        };
        let json = serde_json::to_string(&prefs).unwrap();
        let back: Preferences = serde_json::from_str(&json).unwrap();
        assert_eq!(back.proxy_snapshot, prefs.proxy_snapshot);
    }
}
