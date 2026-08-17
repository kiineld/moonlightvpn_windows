//! The single object the UI drives and observes.
//!
//! It owns the order things happen in, which for a tunnel is not
//! interchangeable — the comments on [`Controller::connect`] say why each step
//! is where it is. Everything below it (the core process, the helper, the REST
//! API, the panel) is stateless with respect to the others.
//!
//! SwiftUI's version is an `ObservableObject` the views read directly. iced has
//! no such thing, so the controller runs in its own task and speaks two
//! channels: [`Command`] in, [`Event`] out. That is a better fit than it sounds
//! — the connect sequence is a long-running async sequence with a dozen await
//! points, and running it *inside* the UI's update loop is what makes a client
//! freeze while it connects.
//!
//! ## The core runs whether or not the tunnel is on
//!
//! A core being up and traffic being routed through it are separate facts, and
//! this client keeps them separate. As soon as there is a subscription the core
//! starts — with no proxy settings written and no TUN block in its config, so it
//! routes nothing. Connecting then only points traffic at a core that is already
//! warm.
//!
//! That is what makes a latency pass immediate: the outbounds a probe needs
//! already exist, so pressing **Пинг** starts measuring instead of starting a
//! core.
//!
//! TUN is the one exception. Its core has to run elevated under the helper, so
//! connecting there stops the idle core and starts the privileged one;
//! disconnecting reverses it.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, Mutex};

use crate::api::{Connection, LogLine, MihomoApi, Traffic};
use crate::helper::{self, Request, Response};
use crate::models::{ConnectionState, Node, SplitMode, SubscriptionInfo, TunnelMode};
use crate::preferences::{self, Preferences};
use crate::process::MihomoProcess;
use crate::split_rule::SplitRule;
use crate::subscription::{self, DeviceIdentity, Source, SubscriptionClient};
use crate::system_proxy;
use crate::{mihomo_config, share_link};

/// How many probes run at once.
///
/// Wide enough that a dead node — which holds its slot for the full timeout —
/// does not stall the live ones behind it, and narrow enough that a subscription
/// with sixty nodes does not open sixty handshakes at once and measure
/// congestion instead of latency.
const PROBE_CONCURRENCY: usize = 8;

/// What the UI can ask for.
#[derive(Debug, Clone)]
pub enum Command {
    /// Called once at launch: clean up after a previous session, load the
    /// subscription and warm a core.
    Start,
    Connect,
    Disconnect,
    SelectNode(String),
    SetAutoSelect(bool),
    /// Re-fetch the subscription from the panel.
    Refresh,
    /// Measure every node.
    Ping,
    ImportSubscription(String),
    RemoveSubscription,
    SetMode(TunnelMode),
    SetSplitMode(SplitMode),
    SetSplitRules(Vec<SplitRule>),
    CloseConnection(String),
    CloseAllConnections,
    RefreshConnections,
    /// Put everything back before the window closes.
    Shutdown,
}

/// What the UI is told about.
#[derive(Debug, Clone)]
pub enum Event {
    State(ConnectionState),
    Nodes(Vec<Node>),
    Info(SubscriptionInfo),
    Source(Option<Source>),
    Uptime(i64),
    Rates {
        up: i64,
        down: i64,
    },
    Session {
        up: i64,
        down: i64,
    },
    /// One probe answered. Delivered per node rather than per pass, so the fast
    /// nodes appear straight away instead of behind the slowest entry.
    Latency {
        node: String,
        ms: Option<u32>,
    },
    PingStarted(Vec<String>),
    PingFinished,
    Refreshing(bool),
    Connections(Vec<Connection>),
    Log(LogEntry),
    Error(String),
    /// Everything that had to be put back has been: the proxy settings are
    /// restored and the core — this app's or the helper's — is stopped. The UI
    /// waits for this before closing its window, because closing first races
    /// the restore and can leave the machine pointed at a dead proxy.
    ShutdownComplete,
    /// The controller changed something the UI persists and displays.
    PreferencesChanged(Box<Preferences>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    pub source: LogSource,
    pub level: String,
    pub message: String,
    pub at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogSource {
    /// mihomo's own log.
    Core,
    /// The app's narration.
    App,
}

/// Merging the two on one timeline is the point: read apart, a failed connect is
/// a core error with no cause; together it is "the app switched to TUN, then the
/// core could not take the route".
impl LogEntry {
    pub fn app(level: &str, message: impl Into<String>) -> LogEntry {
        LogEntry {
            source: LogSource::App,
            level: level.to_string(),
            message: message.into(),
            at: time::OffsetDateTime::now_utc().unix_timestamp(),
        }
    }

    pub fn core(line: &LogLine) -> LogEntry {
        LogEntry {
            source: LogSource::Core,
            level: line.kind.to_uppercase(),
            message: line.payload.clone(),
            at: time::OffsetDateTime::now_utc().unix_timestamp(),
        }
    }
}

/// Log levels, as a floor: WARN means warnings and errors.
pub fn level_rank(level: &str) -> u8 {
    match level.to_uppercase().as_str() {
        "DEBUG" => 0,
        "INFO" => 1,
        "WARNING" | "WARN" => 2,
        "ERROR" | "ERRO" => 3,
        _ => 1,
    }
}

pub struct Controller {
    preferences: Preferences,
    state: ConnectionState,
    api: Arc<MihomoApi>,
    core: Arc<Mutex<MihomoProcess>>,
    events: mpsc::UnboundedSender<Event>,
    /// The config the core is currently running, so a mode switch can rebuild it.
    panel_yaml: Option<String>,
    /// The group the app drives when the user picks a node.
    selector: String,
    connected_at: Option<time::OffsetDateTime>,
    /// Byte totals when the session began, so the session counters are a delta
    /// rather than the core's lifetime totals.
    session_base: Traffic,
    pinging: bool,
    /// Nodes a probe pass has actually covered this session.
    probed: std::collections::HashSet<String>,
}

impl Controller {
    pub fn new(preferences: Preferences, events: mpsc::UnboundedSender<Event>) -> Self {
        let api = Arc::new(MihomoApi::new(
            preferences.controller_port,
            preferences.api_secret.clone(),
        ));
        let core = Arc::new(Mutex::new(MihomoProcess::new(
            core_binary(),
            preferences::core_data_directory(),
        )));
        Controller {
            preferences,
            state: ConnectionState::Disconnected,
            api,
            core,
            events,
            panel_yaml: None,
            selector: mihomo_config::DEFAULT_SELECTOR.to_string(),
            connected_at: None,
            session_base: Traffic::default(),
            pinging: false,
            probed: std::collections::HashSet::new(),
        }
    }

    fn emit(&self, event: Event) {
        let _ = self.events.send(event);
    }

    fn narrate(&self, level: &str, message: impl Into<String>) {
        self.emit(Event::Log(LogEntry::app(level, message)));
    }

    fn set_state(&mut self, state: ConnectionState) {
        self.state = state.clone();
        self.emit(Event::State(state));
    }

    fn save(&self) {
        let _ = self.preferences.save();
        self.emit(Event::PreferencesChanged(Box::new(
            self.preferences.clone(),
        )));
    }

    /// The whole command loop. Runs until [`Command::Shutdown`].
    pub async fn run(mut self, mut commands: mpsc::UnboundedReceiver<Command>) {
        while let Some(command) = commands.recv().await {
            match command {
                Command::Start => self.start().await,
                Command::Connect => self.connect().await,
                Command::Disconnect => self.disconnect().await,
                Command::SelectNode(name) => self.select_node(name).await,
                Command::SetAutoSelect(on) => {
                    self.preferences.auto_select = on;
                    self.save();
                }
                Command::Refresh => self.refresh().await,
                Command::Ping => self.ping().await,
                Command::ImportSubscription(url) => self.import(url).await,
                Command::RemoveSubscription => self.remove_subscription().await,
                Command::SetMode(mode) => self.set_mode(mode).await,
                Command::SetSplitMode(mode) => {
                    self.preferences.split_mode = mode;
                    self.save();
                    self.reload_config().await;
                }
                Command::SetSplitRules(rules) => {
                    self.preferences.split_rules = rules;
                    self.save();
                    self.reload_config().await;
                }
                Command::CloseConnection(id) => {
                    let _ = self.api.close_connection(&id).await;
                    self.refresh_connections().await;
                }
                Command::CloseAllConnections => {
                    let _ = self.api.close_all_connections().await;
                    self.refresh_connections().await;
                }
                Command::RefreshConnections => self.refresh_connections().await,
                Command::Shutdown => {
                    self.shutdown().await;
                    return;
                }
            }
        }
        self.shutdown().await;
    }

    /// Launch.
    ///
    /// The order matters. A privileged core outlives the app that started it —
    /// it is a service's child, not the app's — so one left running from a
    /// previous session still holds the controller port. The core this session
    /// starts then cannot bind it, and **every API call silently addresses the
    /// old core instead**: wrong nodes, wrong connections, and a tunnel still
    /// carrying traffic while the window says "Отключено". So the sweep comes
    /// first, before anything is started or read.
    async fn start(&mut self) {
        self.narrate("INFO", "Moonlight starting");

        // 1. Stop any privileged core this session did not ask for.
        if helper::is_installed() {
            if let Ok(Response::Status { running: true }) =
                helper::send(&Request::Status, Duration::from_secs(2))
            {
                self.narrate(
                    "WARNING",
                    "A privileged core was left running from a previous session — stopping it",
                );
                let _ = helper::send(&Request::Stop, Duration::from_secs(10));
            }
        }

        // 2. Put back proxy settings a crash may have stranded. The snapshot is
        //    persisted precisely so this can happen after a crash that never ran
        //    the disconnect path.
        if let Some(snapshot) = self.preferences.proxy_snapshot.take() {
            self.narrate(
                "WARNING",
                "Restoring proxy settings left by a previous session",
            );
            system_proxy::restore(&snapshot);
            self.save();
        }

        self.emit(Event::PreferencesChanged(Box::new(
            self.preferences.clone(),
        )));

        // 3. Last session's servers, straight away.
        //
        //    A refresh is a subscription fetch and a core start — seconds, on a
        //    link the user may be opening the app precisely because it is bad.
        //    The list used to sit empty for all of it, on a screen whose whole
        //    job is picking from that list. The cached one is replaced the
        //    moment the real one arrives.
        if self.preferences.subscription_url.is_some() {
            if let Some(nodes) = cached_nodes() {
                if !nodes.is_empty() {
                    self.emit(Event::Nodes(nodes));
                }
            }
        }

        // 4. Now it is safe to warm a core.
        if self.preferences.subscription_url.is_some() {
            self.refresh().await;
        }
    }

    /// Fetches the subscription and (re)starts the idle core against it.
    async fn refresh(&mut self) {
        let Some(url) = self.preferences.subscription_url.clone() else {
            return;
        };
        self.emit(Event::Refreshing(true));

        let client = match SubscriptionClient::new(self.device_identity()) {
            Ok(client) => client,
            Err(error) => {
                self.emit(Event::Refreshing(false));
                self.fail(error.to_string());
                return;
            }
        };

        match client.fetch(&url).await {
            Err(error) => {
                self.emit(Event::Refreshing(false));
                self.narrate("ERROR", format!("Subscription refresh failed: {error}"));
                self.emit(Event::Error(error.to_string()));
            }
            Ok(fetched) => {
                // `/info` fills in the device count the headers do not carry;
                // the headers still win field by field.
                let info = match client.fetch_info(&url).await {
                    Some(document) => subscription::merging(&document, &fetched.info),
                    None => fetched.info.clone(),
                };
                self.narrate(
                    "INFO",
                    format!(
                        "Subscription loaded from the {} endpoint",
                        fetched.source.as_str()
                    ),
                );
                self.panel_yaml = Some(fetched.yaml);
                self.emit(Event::Source(Some(fetched.source)));
                self.emit(Event::Info(info));
                self.emit(Event::Refreshing(false));

                self.restart_core().await;
            }
        }
    }

    /// Writes the config and starts (or reloads) the core it describes.
    ///
    /// Never restarts a *connected* core: switching a node, changing the split
    /// mode or loading a refreshed subscription all go through a reload, so the
    /// tunnel survives every one of them.
    async fn restart_core(&mut self) {
        let Some(config) = self.build_config() else {
            return;
        };
        let path = preferences::config_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Err(error) = std::fs::write(&path, &config) {
            self.fail(format!("Could not write the core config: {error}"));
            return;
        }

        if self.state.is_connected() && self.preferences.mode == TunnelMode::Tun {
            // The privileged core owns its own copy of the config.
            match helper::send(&Request::Start { config }, Duration::from_secs(30)) {
                Ok(Response::Started) => {}
                Ok(Response::Error { message }) => return self.fail(message),
                _ => return self.fail("The helper did not answer".to_string()),
            }
        } else if self.api.version().await.is_ok() {
            // A core is already up: reload it rather than dropping the tunnel.
            if let Err(error) = self.api.reload(&path.to_string_lossy()).await {
                self.narrate(
                    "WARNING",
                    format!("Config reload failed, restarting: {error}"),
                );
                self.spawn_core(&path).await;
            }
        } else {
            self.spawn_core(&path).await;
        }

        self.load_nodes().await;
    }

    async fn spawn_core(&mut self, path: &std::path::Path) {
        // Before the core, not by it. Every panel config carries GEOSITE and
        // GEOIP rules, and mihomo resolves those *while parsing* — using its own
        // half-configured fake-ip resolver, before any tunnel exists. When that
        // fails the failure is fatal: the process exits without binding its API,
        // and the only symptom the app can see is "the core did not answer".
        // Fetching them here uses the OS resolver, which works.
        let directory = crate::preferences::core_data_directory();
        if !crate::geodata::present(&directory) {
            self.narrate("INFO", "Downloading geo databases (one time, ~15 MB)");
        }
        if let Err(error) = crate::geodata::ensure(&directory).await {
            return self.fail(format!("Could not download the geo databases. {error}"));
        }

        let (tx, mut rx) = mpsc::unbounded_channel::<String>();
        {
            let mut core = self.core.lock().await;
            core.stop().await;
            if let Err(error) = core.start(path, Some(tx)).await {
                drop(core);
                return self.fail(error.to_string());
            }
        }

        // The core's stdout is narrated onto the same timeline as the app's own
        // lines, which is what makes a failed connect readable.
        let events = self.events.clone();
        tokio::spawn(async move {
            while let Some(line) = rx.recv().await {
                let _ = events.send(Event::Log(LogEntry {
                    source: LogSource::Core,
                    level: level_of(&line),
                    message: line,
                    at: time::OffsetDateTime::now_utc().unix_timestamp(),
                }));
            }
        });

        if !self.api.wait_until_ready(Duration::from_secs(45)).await {
            return self.fail(
                "The core did not answer its API. It may still be downloading geodata.".to_string(),
            );
        }
        self.narrate("INFO", "Core is up");
        self.stream_core_logs();
    }

    /// Subscribes to the core's `/logs` stream.
    fn stream_core_logs(&self) {
        let mut lines = self.api.log_stream("info");
        let events = self.events.clone();
        tokio::spawn(async move {
            while let Some(line) = lines.recv().await {
                let _ = events.send(Event::Log(LogEntry::core(&line)));
            }
        });
    }

    fn build_config(&self) -> Option<String> {
        let panel = self.panel_yaml.as_ref()?;
        let overrides = mihomo_config::Overrides {
            controller_port: self.preferences.controller_port,
            secret: self.preferences.api_secret.clone(),
            mixed_port: self.preferences.mixed_port,
            // The idle core is deliberately built without the TUN block: it must
            // route nothing until the user connects.
            mode: if self.state.is_connected() {
                self.preferences.mode
            } else {
                TunnelMode::SystemProxy
            },
            split_mode: self.preferences.split_mode,
            split_rules: self.active_split_rules(),
            log_level: "info".to_string(),
        };
        match mihomo_config::build(panel, &overrides) {
            Ok(config) => Some(config),
            Err(error) => {
                self.narrate("ERROR", format!("Could not build the config: {error}"));
                None
            }
        }
    }

    /// `PROCESS-*` rules need the core to identify the process behind a
    /// connection, which only TUN can do — under a system proxy the core is
    /// handed a socket with no process behind it. They are dropped here rather
    /// than written and silently never matched.
    fn active_split_rules(&self) -> Vec<SplitRule> {
        let tun = self.preferences.mode == TunnelMode::Tun;
        self.preferences
            .split_rules
            .iter()
            .filter(|rule| tun || !rule.kind.needs_process_matching())
            .cloned()
            .collect()
    }

    async fn load_nodes(&mut self) {
        let groups = self.api.groups().await.unwrap_or_default();
        let group_values: Vec<serde_yaml::Value> = groups
            .iter()
            .map(|g| {
                let mut m = serde_yaml::Mapping::new();
                m.insert(
                    serde_yaml::Value::from("name"),
                    serde_yaml::Value::from(g.name.clone()),
                );
                m.insert(
                    serde_yaml::Value::from("type"),
                    serde_yaml::Value::from(g.kind.clone()),
                );
                serde_yaml::Value::Mapping(m)
            })
            .collect();

        // Follow the config's own MATCH rule to the group the app should drive,
        // rather than guessing by name — a panel localises its group labels.
        let rules: Vec<String> = self
            .panel_yaml
            .as_deref()
            .and_then(|y| serde_yaml::from_str::<serde_yaml::Value>(y).ok())
            .and_then(|v| {
                Some(
                    v.get("rules")?
                        .as_sequence()?
                        .iter()
                        .filter_map(|r| r.as_str().map(str::to_string))
                        .collect(),
                )
            })
            .unwrap_or_default();
        self.selector = mihomo_config::primary_selector_name(&group_values, &rules);

        let mut nodes = self.api.nodes(&self.selector).await.unwrap_or_default();

        // The panel's own transport labels come from the subscription document,
        // which knows more than the API's bare type.
        let labels = self.protocol_labels();
        for node in &mut nodes {
            if node.latency.is_none() {
                node.latency = self.preferences.latency(&node.name);
            }
            // A stored number counts as probed: it came from a real probe, just
            // an earlier one.
            node.probed = node.latency.is_some() || self.probed.contains(&node.name);
            node.protocol_label = labels.get(&node.name).cloned();
        }

        let live: Vec<String> = nodes.iter().map(|n| n.name.clone()).collect();
        self.preferences.prune_latencies(&live);
        self.save();
        cache_nodes(&nodes);

        // Nothing measured yet means every row would read as a dash until the
        // user found the Пинг button. Measuring once, here, is what makes the
        // list useful the moment a subscription lands — and it is the same pass
        // the button runs, so it costs nothing extra to have asked.
        let unmeasured = nodes.iter().any(|n| !n.probed && !n.is_auto_picker());
        self.emit(Event::Nodes(nodes));
        if unmeasured && !self.pinging {
            self.ping().await;
        }
    }

    /// "VLESS Reality" rather than "vless", read from the subscription.
    fn protocol_labels(&self) -> std::collections::HashMap<String, String> {
        let mut labels = std::collections::HashMap::new();
        let Some(yaml) = self.panel_yaml.as_deref() else {
            return labels;
        };
        let Ok(value) = serde_yaml::from_str::<serde_yaml::Value>(yaml) else {
            return labels;
        };
        let Some(proxies) = value.get("proxies").and_then(|p| p.as_sequence()) else {
            return labels;
        };
        for proxy in proxies {
            let Some(map) = proxy.as_mapping() else {
                continue;
            };
            let Some(name) = map.get("name").and_then(|v| v.as_str()) else {
                continue;
            };
            let kind = map.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let mut label = kind.to_uppercase();
            if map.contains_key("reality-opts") {
                label.push_str(" Reality");
            } else if map.get("tls").and_then(|v| v.as_bool()) == Some(true) {
                label.push_str(" TLS");
            }
            labels.insert(name.to_string(), label);
        }
        labels
    }

    /// Connect.
    ///
    /// Every step is where it is for a reason:
    ///
    /// 1. **Snapshot the proxy settings and persist them first.** If anything
    ///    below fails — or the process is killed — the settings can still be put
    ///    back. Taking the snapshot after writing would record our own values.
    /// 2. **Rebuild the config with the mode's block in it.** The idle core has
    ///    no TUN block, so connecting in TUN mode is a different config, not a
    ///    flag.
    /// 3. **Start the privileged core before stopping the idle one** is *not*
    ///    what happens — they would fight over the controller port. The idle
    ///    core is stopped first, and the window is short.
    /// 4. **Check the log for a TUN failure before reporting success.** The core
    ///    keeps running with no interface established and answers its API
    ///    normally, so every other signal says connected while nothing is routed.
    /// 5. **Only then write the proxy settings**, in system-proxy mode. Writing
    ///    them before the core is listening points the machine at a closed port.
    async fn connect(&mut self) {
        if self.state.is_busy() || self.state.is_connected() {
            return;
        }
        if self.panel_yaml.is_none() {
            return self.fail("Add a subscription first".to_string());
        }
        self.set_state(ConnectionState::Connecting);
        self.narrate(
            "INFO",
            format!("Connecting in {:?} mode", self.preferences.mode),
        );

        // 1.
        if self.preferences.mode == TunnelMode::SystemProxy
            && self.preferences.proxy_snapshot.is_none()
        {
            self.preferences.proxy_snapshot = Some(system_proxy::snapshot());
            self.save();
        }

        // 2. The config is built as if connected, so it carries the TUN block.
        self.state = ConnectionState::Connected; // read by build_config
        let config = self.build_config();
        self.state = ConnectionState::Connecting;
        let Some(config) = config else {
            return self.fail("Could not build the core config".to_string());
        };

        let path = preferences::config_path();
        if let Err(error) = std::fs::write(&path, &config) {
            return self.fail(format!("Could not write the core config: {error}"));
        }

        match self.preferences.mode {
            TunnelMode::Tun => {
                if !helper::is_installed() {
                    return self.fail(
                        "TUN needs the Moonlight helper service. Install it in Settings."
                            .to_string(),
                    );
                }
                // 3.
                self.core.lock().await.stop().await;
                self.narrate("INFO", "Handing the core to the privileged helper");
                match helper::send(&Request::Start { config }, Duration::from_secs(45)) {
                    Ok(Response::Started) => {}
                    Ok(Response::Error { message }) => return self.fail(message),
                    Ok(_) => return self.fail("The helper gave an unexpected reply".to_string()),
                    Err(error) => return self.fail(error),
                }
                if !self.api.wait_until_ready(Duration::from_secs(45)).await {
                    return self.fail("The privileged core did not answer its API".to_string());
                }
                self.stream_core_logs();

                // 4. The failure that does not look like one.
                tokio::time::sleep(Duration::from_millis(1200)).await;
                if let Some(reason) = self.tun_failure().await {
                    let _ = helper::send(&Request::Stop, Duration::from_secs(10));
                    return self.fail(reason);
                }
            }
            TunnelMode::SystemProxy => {
                if self.api.version().await.is_err() {
                    self.spawn_core(&path).await;
                    if matches!(self.state, ConnectionState::Failed(_)) {
                        return;
                    }
                } else if let Err(error) = self.api.reload(&path.to_string_lossy()).await {
                    return self.fail(format!("Could not load the config: {error}"));
                }
                // 5.
                if !system_proxy::enable(self.preferences.mixed_port) {
                    return self.fail("Could not write the system proxy settings".to_string());
                }
                self.narrate("INFO", "System proxy settings written");
            }
        }

        self.connected_at = Some(time::OffsetDateTime::now_utc());
        self.session_base = self.api.totals().await.unwrap_or_default();
        self.set_state(ConnectionState::Connected);
        self.narrate("INFO", "Connected");

        self.apply_selection().await;
        self.start_polling();
    }

    /// The core's TUN failure is not a crash, so it has to be asked for.
    async fn tun_failure(&self) -> Option<String> {
        let log = self.core.lock().await.log();
        MihomoProcess::tun_failure(&log)
    }

    async fn disconnect(&mut self) {
        if self.state.is_busy() || !self.state.is_connected() {
            return;
        }
        self.set_state(ConnectionState::Disconnecting);
        self.narrate("INFO", "Disconnecting");

        // Proxy settings come back first: if anything below hangs, the machine
        // is not left pointed at a core that is going away.
        if let Some(snapshot) = self.preferences.proxy_snapshot.take() {
            system_proxy::restore(&snapshot);
            self.save();
            self.narrate("INFO", "System proxy settings restored");
        }

        if self.preferences.mode == TunnelMode::Tun {
            let _ = helper::send(&Request::Stop, Duration::from_secs(15));
        }

        self.connected_at = None;
        self.set_state(ConnectionState::Disconnected);
        self.emit(Event::Uptime(0));
        self.emit(Event::Rates { up: 0, down: 0 });

        // Back to a warm idle core, so the next ping is immediate.
        self.restart_core().await;
    }

    /// Uptime, rates and session totals, once a second while connected.
    fn start_polling(&self) {
        let api = Arc::clone(&self.api);
        let events = self.events.clone();
        let started = self.connected_at;
        let base = self.session_base;

        tokio::spawn(async move {
            let mut traffic = api.traffic_stream();
            let mut ticker = tokio::time::interval(Duration::from_secs(1));
            loop {
                tokio::select! {
                    sample = traffic.recv() => {
                        let Some(sample) = sample else { break };
                        // /traffic reports the rate for the last second, and
                        // /connections carries the cumulative totals.
                        if events.send(Event::Rates { up: sample.up, down: sample.down }).is_err() {
                            break;
                        }
                    }
                    _ = ticker.tick() => {
                        if let Some(started) = started {
                            let seconds = (time::OffsetDateTime::now_utc() - started).whole_seconds();
                            if events.send(Event::Uptime(seconds)).is_err() { break; }
                        }
                        if let Ok(totals) = api.totals().await {
                            let _ = events.send(Event::Session {
                                up: (totals.up - base.up).max(0),
                                down: (totals.down - base.down).max(0),
                            });
                        }
                    }
                }
            }
        });
    }

    async fn select_node(&mut self, name: String) {
        self.preferences.selected_node = if name.is_empty() {
            None
        } else {
            Some(name.clone())
        };
        self.preferences.auto_select = name.is_empty();
        self.save();
        self.apply_selection().await;
    }

    /// Points the panel's selector at whatever the user chose.
    ///
    /// Then closes every open connection. The core reopens whatever the program
    /// still wants, so this reads as "move this app onto the node I just picked"
    /// rather than as cutting it off.
    async fn apply_selection(&mut self) {
        let Some(node) = self.preferences.selected_node.clone() else {
            return;
        };
        if let Err(error) = self.api.select(&node, &self.selector).await {
            self.narrate("WARNING", format!("Could not switch node: {error}"));
            return;
        }
        self.narrate("INFO", format!("Node set to {node}"));
        if self.state.is_connected() {
            let _ = self.api.close_all_connections().await;
        }
    }

    /// Measures every node through the running core.
    ///
    /// Available whether or not the tunnel is up, because a core is always
    /// running.
    async fn ping(&mut self) {
        if self.pinging {
            return;
        }
        let nodes = self.api.nodes(&self.selector).await.unwrap_or_default();
        let names: Vec<String> = nodes.iter().map(|n| n.name.clone()).collect();
        if names.is_empty() {
            return;
        }

        self.pinging = true;
        self.emit(Event::PingStarted(names.clone()));

        let (tx, mut rx) = mpsc::unbounded_channel::<(String, Option<u32>)>();
        let events = self.events.clone();
        tokio::spawn(async move {
            while let Some((node, ms)) = rx.recv().await {
                let _ = events.send(Event::Latency { node, ms });
            }
        });

        let results = self.api.delays(names.clone(), PROBE_CONCURRENCY, tx).await;
        // Everything asked has now been asked, answer or not — which is what
        // lets the UI say `n/a` for a silent node and a dash for one that has
        // simply not been measured yet.
        self.probed.extend(names);
        for (node, ms) in &results {
            self.preferences.record_latency(node, Some(*ms));
        }
        // A node that did not answer loses its number rather than keeping a
        // stale one that is no longer true.
        let answered: Vec<&String> = results.keys().collect();
        let stale: Vec<String> = self
            .preferences
            .latencies
            .keys()
            .filter(|k| !answered.contains(k))
            .cloned()
            .collect();
        for node in stale {
            self.preferences.record_latency(&node, None);
        }

        self.save();
        self.pinging = false;
        self.emit(Event::PingFinished);
    }

    async fn import(&mut self, url: String) {
        let Some(normalised) = subscription::normalize(&url) else {
            return self.fail("That is not a valid subscription link".to_string());
        };
        self.preferences.subscription_url = Some(normalised);
        self.save();
        self.refresh().await;
    }

    async fn remove_subscription(&mut self) {
        if self.state.is_connected() {
            self.disconnect().await;
        }
        self.preferences.subscription_url = None;
        self.preferences.selected_node = None;
        self.preferences.latencies.clear();
        self.save();
        self.panel_yaml = None;
        self.core.lock().await.stop().await;
        self.emit(Event::Nodes(Vec::new()));
        self.emit(Event::Info(SubscriptionInfo::default()));
        self.emit(Event::Source(None));
    }

    /// Switching mode is a reconnect, because the two modes are different
    /// configs and different processes — not a flag on one core.
    async fn set_mode(&mut self, mode: TunnelMode) {
        if self.preferences.mode == mode {
            return;
        }
        let was_connected = self.state.is_connected();
        if was_connected {
            self.disconnect().await;
        }
        self.preferences.mode = mode;
        self.save();
        self.reload_config().await;
        if was_connected {
            self.connect().await;
        }
    }

    async fn reload_config(&mut self) {
        if self.panel_yaml.is_some() {
            self.restart_core().await;
        }
    }

    async fn refresh_connections(&self) {
        if let Ok(connections) = self.api.connections().await {
            self.emit(Event::Connections(connections));
        }
    }

    async fn shutdown(&mut self) {
        if let Some(snapshot) = self.preferences.proxy_snapshot.take() {
            system_proxy::restore(&snapshot);
            let _ = self.preferences.save();
        }
        // Always, not only in TUN mode. The mode can have been switched while a
        // helper-run core was still up, and a core left running under the
        // service keeps capturing traffic after the window is gone — which is
        // exactly the state people notice as "it did not close the helper".
        let _ = helper::send(&Request::Stop, Duration::from_secs(10));
        self.core.lock().await.stop().await;
        // The service exists to serve this app, so it goes when the app does
        // rather than idling as a LocalSystem process until the next reboot.
        helper::stop();
        self.emit(Event::ShutdownComplete);
    }

    fn fail(&mut self, why: String) {
        self.narrate("ERROR", why.clone());
        self.set_state(ConnectionState::Failed(why.clone()));
        self.emit(Event::Error(why));
    }

    fn device_identity(&self) -> DeviceIdentity {
        DeviceIdentity {
            hwid: self.preferences.hwid.clone(),
            os_version: os_version(),
            model: machine_model(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

/// Where the last known server list is kept, so the next launch has something
/// to draw before the network answers.
fn nodes_cache_path() -> std::path::PathBuf {
    crate::preferences::support_directory().join("nodes.json")
}

/// Best effort in both directions: this is a convenience, never a source of
/// truth. A stale or unreadable cache costs a moment of the old list, and the
/// real one replaces it either way.
fn cache_nodes(nodes: &[Node]) {
    if let Ok(text) = serde_json::to_string(nodes) {
        let _ = std::fs::write(nodes_cache_path(), text);
    }
}

fn cached_nodes() -> Option<Vec<Node>> {
    let text = std::fs::read_to_string(nodes_cache_path()).ok()?;
    serde_json::from_str(&text).ok()
}

/// mihomo prefixes its lines with a level; anything else reads as INFO.
fn level_of(line: &str) -> String {
    let upper = line.to_uppercase();
    for level in ["ERRO", "ERROR", "WARN", "WARNING", "DEBUG"] {
        if upper.contains(level) {
            return level.to_string();
        }
    }
    "INFO".to_string()
}

/// The core beside the executable, which is where the portable layout puts it.
pub fn core_binary() -> std::path::PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|d| d.join("mihomo.exe")))
        .unwrap_or_else(|| std::path::PathBuf::from("mihomo.exe"))
}

pub fn os_version() -> String {
    #[cfg(windows)]
    {
        // `cmd /c ver` rather than GetVersionEx, which lies to unmanifested
        // processes and reports 6.2 on Windows 10 and 11 alike.
        std::process::Command::new("cmd")
            .args(["/c", "ver"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Windows".to_string())
    }
    #[cfg(not(windows))]
    {
        "Windows".to_string()
    }
}

pub fn machine_model() -> String {
    std::env::var("COMPUTERNAME").unwrap_or_else(|_| "PC".to_string())
}

/// Turns a subscription body into proxies, for the import screen's preview.
pub fn count_nodes(body: &str) -> usize {
    if subscription::looks_like_clash_config(body) {
        return serde_yaml::from_str::<serde_yaml::Value>(body)
            .ok()
            .and_then(|v| v.get("proxies")?.as_sequence().map(|s| s.len()))
            .unwrap_or(0);
    }
    share_link::decode_list(body).len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_levels_are_a_floor_not_an_exact_match() {
        // WARN means warnings and errors, which is what the Logs screen's
        // filter relies on.
        assert!(level_rank("ERROR") > level_rank("WARNING"));
        assert!(level_rank("WARNING") > level_rank("INFO"));
        assert!(level_rank("INFO") > level_rank("DEBUG"));
    }

    #[test]
    fn mihomos_own_level_spellings_are_recognised() {
        // The core writes ERRO and WARN; the API writes error and warning.
        assert_eq!(level_rank("ERRO"), level_rank("ERROR"));
        assert_eq!(level_rank("WARN"), level_rank("WARNING"));
        assert_eq!(level_rank("erro"), level_rank("ERROR"));
    }

    #[test]
    fn an_unknown_level_reads_as_info_rather_than_being_hidden() {
        // Defaulting to DEBUG would hide a line the core thought worth writing.
        assert_eq!(level_rank("something"), level_rank("INFO"));
    }

    #[test]
    fn a_core_line_is_classified_by_what_it_contains() {
        assert_eq!(level_of("ERRO[2026] Start TUN listening error"), "ERRO");
        assert_eq!(level_of("WARN[2026] slow"), "WARN");
        assert_eq!(level_of("INFO[2026] RESTful API listening"), "INFO");
        assert_eq!(level_of("no level here"), "INFO");
    }

    #[test]
    fn app_and_core_lines_are_told_apart_on_one_timeline() {
        // Read apart, a failed connect is a core error with no cause; together
        // it is "the app switched to TUN, then the core could not take it".
        let app = LogEntry::app("INFO", "Connecting");
        let core = LogEntry::core(&LogLine {
            kind: "error".into(),
            payload: "boom".into(),
        });
        assert_eq!(app.source, LogSource::App);
        assert_eq!(core.source, LogSource::Core);
        assert_eq!(core.level, "ERROR");
    }

    #[test]
    fn the_core_is_looked_for_beside_the_executable() {
        // The portable layout puts mihomo.exe next to moonlight.exe, and the
        // helper copies it out of there at install time.
        let path = core_binary();
        assert_eq!(
            path.file_name().and_then(|n| n.to_str()),
            Some("mihomo.exe")
        );
    }

    #[test]
    fn node_counting_handles_both_subscription_shapes() {
        let clash = "proxies:\n  - name: A\n  - name: B\n";
        assert_eq!(count_nodes(clash), 2);

        let links = "vless://u@h:443#A\ntrojan://p@h:443#B\nss://m:p@h:8388#C";
        assert_eq!(count_nodes(links), 3);

        assert_eq!(count_nodes("nothing here"), 0);
    }
}
