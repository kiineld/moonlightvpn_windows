//! Moonlight VPN for Windows.
//!
//! `windows_subsystem = "windows"` so launching the app does not also open a
//! console behind it. It is left on for debug builds, where a `println!` in a
//! panic handler is worth more than a clean taskbar.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod components;
mod dial;
mod localization;
mod logo;
mod screens;
mod theme;

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use iced::widget::{column, container, row, scrollable, Space};
use iced::{Element, Length, Subscription, Task};

use moonlight_core::api::Connection;
use moonlight_core::controller::{Command, Controller, Event, LogEntry};
use moonlight_core::preferences::Preferences;
use moonlight_core::split_rule::{self, Kind, SplitRule};
use moonlight_core::subscription::Source;
use moonlight_core::{
    AppEntry, AppLocale, ConnectionState, Node, SplitMode, SubscriptionInfo, TunnelMode,
};
use moonlight_design::{Appearance, Palette};

use localization::{t, S};

pub const APP_NAME: &str = "Moonlight";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Where *Проверить обновления* and the support links point.
///
/// Compiled in with an environment override, so a fork points these at its own
/// endpoints without touching source — the counterpart of the macOS build's
/// `Info.plist` keys.
pub const RELEASES_API: &str = match option_env!("RELEASES_API") {
    Some(url) => url,
    None => "https://api.github.com/repos/kiineld/moonlightvpn_windows/releases",
};
pub const TELEGRAM_BOT_URL: &str = match option_env!("TELEGRAM_BOT_URL") {
    Some(url) => url,
    None => "https://t.me/moonlightvpn_bot",
};
pub const TELEGRAM_CHANNEL_URL: &str = match option_env!("TELEGRAM_CHANNEL_URL") {
    Some(url) => url,
    None => "https://t.me/moonlightvpn",
};
pub const SUPPORT_URL: &str = match option_env!("SUPPORT_URL") {
    Some(url) => url,
    None => "https://t.me/moonlightvpn_support",
};

/// The controller's channels, handed over to the subscription once.
///
/// A `static` because iced's `Subscription::run` takes a plain function: there
/// is nowhere to capture a receiver, so it is parked here at construction and
/// taken on the first poll.
type Events = Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<Event>>>;
static EVENTS: OnceLock<Events> = OnceLock::new();
static COMMANDS: OnceLock<tokio::sync::mpsc::UnboundedSender<Command>> = OnceLock::new();

/// The most log lines kept in memory.
///
/// A connected core writes steadily, and an unbounded list is a leak measured
/// in hours rather than a screen that scrolls a long way.
const LOG_LIMIT: usize = 2_000;

fn main() -> iced::Result {
    // Every callback below is a `fn` item, and the whole builder is one
    // unbroken expression. Both are load-bearing:
    //
    // - A closure with an un-annotated reference parameter — `.theme(|_| …)` —
    //   is inferred at a single lifetime, where iced's `view` bound is
    //   higher-ranked. The mismatch surfaces as `implementation of FnOnce is
    //   not general enough` pointing at the whole chain, which names neither
    //   the closure nor the reason. A `fn` item is higher-ranked and resolves
    //   it.
    // - `iced::application` returns an opaque `Application<impl Program>`.
    //   Binding it to a `mut` local to register fonts in a loop — or threading
    //   it through a `fold` — pins that opaque type at one lifetime and
    //   reproduces the same error.
    //
    // A face the fetch script has not downloaded is staged as an empty file by
    // build.rs; the font database rejects it and text falls back to the system
    // font, which is the intended degraded state.
    iced::application(Moonlight::new, Moonlight::update, Moonlight::view)
        .title(Moonlight::title)
        .subscription(Moonlight::subscription)
        .theme(Moonlight::iced_theme)
        .window_size((1240.0, 820.0))
        .centered()
        .font(moonlight_design::FONT_BYTES[0])
        .font(moonlight_design::FONT_BYTES[1])
        .font(moonlight_design::FONT_BYTES[2])
        .font(moonlight_design::FONT_BYTES[3])
        .default_font(moonlight_design::ui(moonlight_design::typography::BODY))
        .run()
}

/// Which screen is showing.
///
/// Page changes **do not cross-fade**. Every transition tried — a crossfade, or
/// an identity removal — keeps the outgoing screen in the tree for the length of
/// the animation, so the previous page shows *through* the new one and reads as
/// a blink. The page swaps at once and the incoming screen plays its own
/// entrance, which starts only after the old one is gone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Connect,
    Subscription,
    Apps,
    Settings,
    Import,
    Logs,
    Connections,
}

impl Page {
    /// Logs and Connections are reached from Settings, not from the rail — they
    /// are diagnostics, not destinations. Import is reached from Subscription.
    pub const SIDEBAR: [Page; 4] = [
        Page::Connect,
        Page::Subscription,
        Page::Apps,
        Page::Settings,
    ];

    pub fn title(self) -> S {
        match self {
            Page::Connect => S::NavConnect,
            Page::Subscription => S::NavSubscription,
            Page::Apps => S::NavApps,
            Page::Settings => S::NavSettings,
            Page::Import => S::ImportTitle,
            Page::Logs => S::NavLogs,
            Page::Connections => S::NavConnections,
        }
    }

    pub fn subtitle(self) -> S {
        match self {
            Page::Connect => S::ConnectSubtitle,
            Page::Subscription => S::SubscriptionSubtitle,
            Page::Apps => S::AppsSubtitle,
            Page::Settings => S::SettingsSubtitle,
            Page::Import => S::ImportSubtitle,
            Page::Logs => S::LogsSubtitle,
            Page::Connections => S::ConnectionsSubtitle,
        }
    }

    pub fn icon(self) -> moonlight_design::Icon {
        use moonlight_design::Icon;
        match self {
            Page::Connect => Icon::Power,
            Page::Subscription => Icon::Sparkles,
            Page::Apps => Icon::Layers,
            Page::Settings => Icon::Settings,
            Page::Import => Icon::Plus,
            Page::Logs => Icon::Activity,
            Page::Connections => Icon::Globe,
        }
    }

    /// Which sidebar item is lit while this page is showing.
    ///
    /// Import has no rail entry of its own, but arriving there from the
    /// Subscription screen and watching the sidebar go dark reads as having
    /// left the app.
    pub fn rail_item(self) -> Page {
        match self {
            Page::Import => Page::Subscription,
            Page::Logs | Page::Connections => Page::Settings,
            other => other,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    Navigate(Page),
    ToggleSidebar,
    CycleAppearance,
    SetLocale(AppLocale),

    ToggleConnection,
    SelectNode(String),
    Ping,
    Refresh,

    ImportChanged(String),
    ImportSubmit,
    ImportPasted(Option<String>),
    PasteFromClipboard,
    RemoveSubscription,

    SetMode(TunnelMode),
    SetSplitMode(SplitMode),
    ToggleApp(String),
    ToggleRule(uuid::Uuid),
    DeleteRule(uuid::Uuid),
    RuleKindChanged(Kind),
    RuleValueChanged(String),
    RuleSubmit,
    AppSearchChanged(String),
    AppsScanned(Vec<AppEntry>),
    RunningScanned(Vec<String>),

    InstallHelper,
    RemoveHelper,
    HelperChanged(bool),
    CheckForUpdates,
    UpdateChecked(String),
    OpenUrl(&'static str),

    LogFilterLevel(u8),
    LogFilterText(String),
    ClearLogs,
    CloseConnection(String),
    CloseAllConnections,

    /// From the controller.
    Controller(Event),
    /// One animation frame; drives the dial's sweep.
    Tick(Instant),
}

pub struct Moonlight {
    page: Page,
    preferences: Preferences,
    state: ConnectionState,
    transition_started: Option<Instant>,

    nodes: Vec<Node>,
    info: SubscriptionInfo,
    source: Option<Source>,
    uptime_seconds: i64,
    rates: (i64, i64),
    session: (i64, i64),
    pending_probes: Vec<String>,
    is_pinging: bool,
    is_refreshing: bool,
    last_error: Option<String>,

    apps: Vec<AppEntry>,
    running: Vec<String>,
    app_search: String,
    rule_kind: Kind,
    rule_value: String,
    rule_error: Option<String>,

    import_field: String,
    helper_installed: bool,
    update_status: Option<String>,

    logs: Vec<LogEntry>,
    log_level: u8,
    log_filter: String,
    connections: Vec<Connection>,

    sidebar_collapsed: bool,
    /// Off in tests, so preference changes stay in memory.
    persist: bool,
}

impl Moonlight {
    fn new() -> (Self, Task<Message>) {
        let preferences = Preferences::load();
        let app = Moonlight::with_preferences(preferences.clone());

        // The controller owns everything below the UI and runs in its own task.
        let (command_tx, command_rx) = tokio::sync::mpsc::unbounded_channel();
        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
        let _ = COMMANDS.set(command_tx);
        let _ = EVENTS.set(Mutex::new(Some(event_rx)));

        tokio::spawn(async move {
            Controller::new(preferences, event_tx).run(command_rx).await;
        });
        send(Command::Start);

        (
            app,
            Task::batch([
                Task::perform(scan_apps(), Message::AppsScanned),
                Task::perform(check_helper(), Message::HelperChanged),
            ]),
        )
    }

    /// Injectable so the tests never read or write the real preferences file —
    /// `Preferences::save` writes to `%APPDATA%`, and a test suite that touches
    /// it both leaks state between runs and edits the user's own settings.
    fn with_preferences(preferences: Preferences) -> Self {
        let sidebar_collapsed = preferences.sidebar_collapsed;
        Moonlight {
            page: Page::Connect,
            preferences,
            state: ConnectionState::Disconnected,
            transition_started: None,
            nodes: Vec::new(),
            info: SubscriptionInfo::default(),
            source: None,
            uptime_seconds: 0,
            rates: (0, 0),
            session: (0, 0),
            pending_probes: Vec::new(),
            is_pinging: false,
            is_refreshing: false,
            last_error: None,
            apps: Vec::new(),
            running: Vec::new(),
            app_search: String::new(),
            rule_kind: Kind::DomainSuffix,
            rule_value: String::new(),
            rule_error: None,
            import_field: String::new(),
            helper_installed: false,
            update_status: None,
            logs: Vec::new(),
            log_level: 1,
            log_filter: String::new(),
            connections: Vec::new(),
            sidebar_collapsed,
            persist: true,
        }
    }

    fn title(&self) -> String {
        // The state goes in the window title so a user with the window behind
        // something else can still read it from the taskbar.
        let state = match &self.state {
            ConnectionState::Connected => t(S::StateConnected, self.locale()),
            ConnectionState::Connecting => t(S::Connecting, self.locale()),
            ConnectionState::Disconnecting => t(S::Disconnecting, self.locale()),
            ConnectionState::Failed(_) => t(S::StateFailed, self.locale()),
            ConnectionState::Disconnected => t(S::StateDisconnected, self.locale()),
        };
        format!("{APP_NAME} · {state}")
    }

    fn iced_theme(&self) -> iced::Theme {
        iced::Theme::Dark
    }

    fn locale(&self) -> AppLocale {
        self.preferences.locale
    }

    fn palette(&self) -> Palette {
        let appearance = match self.preferences.appearance.as_deref() {
            Some("dark") => Appearance::Dark,
            Some("light") => Appearance::Light,
            _ => Appearance::System,
        };
        appearance.palette(system_prefers_dark())
    }

    fn save(&self) {
        if self.persist {
            let _ = self.preferences.save();
        }
    }

    /// 0…1 through the current transition.
    fn transition_progress(&self) -> f32 {
        let Some(started) = self.transition_started else {
            return 1.0;
        };
        moonlight_design::motion::progress(started.elapsed(), moonlight_design::dur::SLIDE)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Navigate(page) => self.page = page,
            Message::ToggleSidebar => {
                self.sidebar_collapsed = !self.sidebar_collapsed;
                self.preferences.sidebar_collapsed = self.sidebar_collapsed;
                self.save();
            }
            Message::CycleAppearance => {
                // System → dark → light → system, which is the order the macOS
                // client's sun button cycles in.
                self.preferences.appearance = match self.preferences.appearance.as_deref() {
                    None => Some("dark".into()),
                    Some("dark") => Some("light".into()),
                    _ => None,
                };
                self.save();
            }
            Message::SetLocale(locale) => {
                self.preferences.locale = locale;
                self.save();
            }

            Message::ToggleConnection => {
                if self.state.is_busy() {
                    return Task::none();
                }
                self.last_error = None;
                self.transition_started = Some(Instant::now());
                send(if self.state.is_connected() {
                    Command::Disconnect
                } else {
                    Command::Connect
                });
            }
            Message::SelectNode(name) => {
                self.preferences.selected_node = if name.is_empty() {
                    None
                } else {
                    Some(name.clone())
                };
                self.preferences.auto_select = name.is_empty();
                send(Command::SelectNode(name));
            }
            Message::Ping => send(Command::Ping),
            Message::Refresh => send(Command::Refresh),

            Message::ImportChanged(value) => self.import_field = value,
            Message::ImportSubmit => {
                let url = self.import_field.trim().to_string();
                if !url.is_empty() {
                    send(Command::ImportSubscription(url));
                    self.import_field.clear();
                    self.page = Page::Subscription;
                }
            }
            Message::PasteFromClipboard => {
                return iced::clipboard::read().map(Message::ImportPasted);
            }
            Message::ImportPasted(value) => {
                if let Some(value) = value {
                    self.import_field = value.trim().to_string();
                }
            }
            Message::RemoveSubscription => {
                send(Command::RemoveSubscription);
                self.page = Page::Connect;
            }

            Message::SetMode(mode) => {
                self.preferences.mode = mode;
                self.save();
                send(Command::SetMode(mode));
            }
            Message::SetSplitMode(mode) => {
                self.preferences.split_mode = mode;
                self.save();
                send(Command::SetSplitMode(mode));
            }
            Message::ToggleApp(executable) => {
                self.preferences.toggle_app(&executable);
                self.save();
                send(Command::SetSplitRules(self.preferences.split_rules.clone()));
            }
            Message::ToggleRule(id) => {
                if let Some(rule) = self.preferences.split_rules.iter_mut().find(|r| r.id == id) {
                    rule.enabled = !rule.enabled;
                }
                self.save();
                send(Command::SetSplitRules(self.preferences.split_rules.clone()));
            }
            Message::DeleteRule(id) => {
                self.preferences.split_rules.retain(|r| r.id != id);
                self.save();
                send(Command::SetSplitRules(self.preferences.split_rules.clone()));
            }
            Message::RuleKindChanged(kind) => {
                self.rule_kind = kind;
                self.rule_error = None;
            }
            Message::RuleValueChanged(value) => {
                self.rule_value = value;
                self.rule_error = None;
            }
            Message::RuleSubmit => {
                // Validated before it can be added, because a bad rule does not
                // fail alone: the core refuses the whole config, so the tunnel
                // stops rather than the rule being skipped.
                match split_rule::validate(self.rule_kind, &self.rule_value) {
                    Some(invalid) => self.rule_error = Some(invalid.to_string()),
                    None => {
                        self.preferences
                            .split_rules
                            .push(SplitRule::new(self.rule_kind, self.rule_value.trim()));
                        self.rule_value.clear();
                        self.rule_error = None;
                        self.save();
                        send(Command::SetSplitRules(self.preferences.split_rules.clone()));
                    }
                }
            }
            Message::AppSearchChanged(value) => self.app_search = value,
            Message::AppsScanned(apps) => self.apps = apps,
            Message::RunningScanned(running) => self.running = running,

            Message::InstallHelper => {
                return Task::perform(install_helper(true), Message::HelperChanged)
            }
            Message::RemoveHelper => {
                return Task::perform(install_helper(false), Message::HelperChanged)
            }
            Message::HelperChanged(installed) => {
                self.helper_installed = installed;
                // TUN without the service cannot work, and leaving the mode set
                // to it would fail on every connect with the same message.
                if !installed && self.preferences.mode == TunnelMode::Tun {
                    self.preferences.mode = TunnelMode::SystemProxy;
                    self.save();
                    send(Command::SetMode(TunnelMode::SystemProxy));
                }
            }
            Message::CheckForUpdates => {
                self.update_status = Some(t(S::Checking, self.locale()).to_string());
                return Task::perform(check_updates(self.locale()), Message::UpdateChecked);
            }
            Message::UpdateChecked(status) => self.update_status = Some(status),
            Message::OpenUrl(url) => open_url(url),

            Message::LogFilterLevel(level) => self.log_level = level,
            Message::LogFilterText(value) => self.log_filter = value,
            Message::ClearLogs => self.logs.clear(),
            Message::CloseConnection(id) => send(Command::CloseConnection(id)),
            Message::CloseAllConnections => send(Command::CloseAllConnections),

            Message::Controller(event) => return self.apply(event),
            Message::Tick(_) => {
                if self.transition_progress() >= 1.0 {
                    self.transition_started = None;
                }
                if self.page == Page::Connections {
                    send(Command::RefreshConnections);
                }
                if self.page == Page::Apps {
                    return Task::perform(scan_running(), Message::RunningScanned);
                }
            }
        }
        Task::none()
    }

    /// Everything the controller reports.
    fn apply(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::State(state) => {
                // The transition ends when the controller says so, not when a
                // timer runs out — a connect that takes eight seconds must not
                // show a settled dial after four.
                if !state.is_busy() {
                    self.transition_started = None;
                }
                if let ConnectionState::Failed(why) = &state {
                    self.last_error = Some(why.clone());
                }
                self.state = state;
            }
            Event::Nodes(nodes) => self.nodes = nodes,
            Event::Info(info) => self.info = info,
            Event::Source(source) => self.source = source,
            Event::Uptime(seconds) => self.uptime_seconds = seconds,
            Event::Rates { up, down } => self.rates = (up, down),
            Event::Session { up, down } => self.session = (up, down),
            Event::Latency { node, ms } => {
                // Applied as each node answers, so the fast ones — which are the
                // ones being chosen between — appear straight away instead of
                // behind the slowest entry in the list.
                self.pending_probes.retain(|n| *n != node);
                if let Some(entry) = self.nodes.iter_mut().find(|n| n.name == node) {
                    entry.latency = ms;
                }
                self.preferences.record_latency(&node, ms);
            }
            Event::PingStarted(names) => {
                self.pending_probes = names;
                self.is_pinging = true;
            }
            Event::PingFinished => {
                self.pending_probes.clear();
                self.is_pinging = false;
            }
            Event::Refreshing(on) => self.is_refreshing = on,
            Event::Connections(connections) => self.connections = connections,
            Event::Log(entry) => {
                self.logs.push(entry);
                if self.logs.len() > LOG_LIMIT {
                    let excess = self.logs.len() - LOG_LIMIT;
                    self.logs.drain(..excess);
                }
            }
            Event::Error(why) => self.last_error = Some(why),
            Event::PreferencesChanged(preferences) => {
                // The controller owns the parts of preferences it changes —
                // latencies, the proxy snapshot — so its copy wins for those,
                // while the view settings the UI edits are already correct here
                // and must not be undone by a report that crossed with them.
                let sidebar = self.preferences.sidebar_collapsed;
                let appearance = self.preferences.appearance.clone();
                let locale = self.preferences.locale;
                self.preferences = *preferences;
                self.preferences.sidebar_collapsed = sidebar;
                self.preferences.appearance = appearance;
                self.preferences.locale = locale;
            }
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = vec![Subscription::run(controller_events)];

        // Only while something is moving. A window sitting disconnected has no
        // reason to wake the GPU sixty times a second.
        match Cadence::for_state(&self.state, self.page) {
            Cadence::Idle => {}
            Cadence::Frame => {
                subscriptions.push(iced::time::every(Duration::from_millis(16)).map(Message::Tick))
            }
            Cadence::Second => {
                subscriptions.push(iced::time::every(Duration::from_secs(1)).map(Message::Tick))
            }
        }
        Subscription::batch(subscriptions)
    }

    fn view(&self) -> Element<'_, Message> {
        let palette = self.palette();
        let locale = self.locale();

        let body = match self.page {
            Page::Connect => screens::connect::view(self),
            Page::Subscription => screens::subscription::view(self),
            Page::Apps => screens::apps::view(self),
            Page::Settings => screens::settings::view(self),
            Page::Import => screens::import::view(self),
            Page::Logs => screens::logs::view(self),
            Page::Connections => screens::connections::view(self),
        };

        let content = row![
            screens::sidebar::view(
                palette,
                locale,
                self.page.rail_item(),
                self.sidebar_collapsed,
                &self.preferences,
                &self.info,
            ),
            container(
                column![
                    screens::header::view(self),
                    scrollable(body)
                        .height(Length::Fill)
                        .style(move |theme, _| theme::scroller(palette, theme)),
                ]
                .spacing(18)
            )
            .padding(24)
            .width(Length::Fill),
        ]
        .height(Length::Fill);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| theme::page(palette))
            .into()
    }
}

/// The stream of controller events, taken once.
fn controller_events() -> impl iced::futures::Stream<Item = Message> {
    let receiver = EVENTS
        .get()
        .and_then(|slot| slot.lock().ok().and_then(|mut slot| slot.take()));

    iced::futures::stream::unfold(receiver, |mut receiver| async move {
        let Some(rx) = receiver.as_mut() else {
            // The receiver was already taken — park forever rather than ending
            // the stream, which iced would treat as a subscription to restart
            // in a loop.
            std::future::pending::<()>().await;
            unreachable!()
        };
        let event = rx.recv().await?;
        Some((Message::Controller(event), receiver))
    })
}

fn send(command: Command) {
    if let Some(sender) = COMMANDS.get() {
        let _ = sender.send(command);
    }
}

async fn scan_apps() -> Vec<AppEntry> {
    // The walk takes hundreds of milliseconds, which is a visible stall on the
    // UI thread.
    tokio::task::spawn_blocking(moonlight_core::app_inventory::scan)
        .await
        .unwrap_or_default()
}

async fn scan_running() -> Vec<String> {
    tokio::task::spawn_blocking(moonlight_core::app_inventory::running_executables)
        .await
        .unwrap_or_default()
}

async fn check_helper() -> bool {
    tokio::task::spawn_blocking(moonlight_core::helper::is_installed)
        .await
        .unwrap_or(false)
}

/// Installing the service needs elevation, so it is a UAC prompt on the helper's
/// own `--install`, not something this process can do.
async fn install_helper(install: bool) -> bool {
    let flag = if install { "--install" } else { "--uninstall" };
    let _ = tokio::task::spawn_blocking(move || elevate(flag)).await;
    tokio::time::sleep(Duration::from_millis(600)).await;
    check_helper().await
}

#[cfg(windows)]
fn elevate(argument: &str) {
    // The `runas` verb is what raises the UAC prompt; a plain spawn fails with
    // ERROR_ELEVATION_REQUIRED and no dialog, which reads as nothing happening.
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let Some(helper) = exe.parent().map(|d| d.join("moonlight-helper.exe")) else {
        return;
    };
    let _ = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &format!(
                "Start-Process -FilePath '{}' -ArgumentList '{argument}' -Verb RunAs -Wait",
                helper.display()
            ),
        ])
        .output();
}

#[cfg(not(windows))]
fn elevate(_argument: &str) {}

async fn check_updates(locale: AppLocale) -> String {
    use moonlight_core::updater::{self, Outcome};
    match updater::check(RELEASES_API, VERSION).await {
        Err(error) => error.to_string(),
        Ok(Outcome::UpToDate { current }) => match locale {
            AppLocale::Ru => format!("Установлена последняя версия ({current})"),
            AppLocale::En => format!("You are on the latest version ({current})"),
        },
        Ok(Outcome::Available(release)) => {
            let version = release.version.clone();
            let temporary = std::env::temp_dir().join("moonlight-update.zip");
            match updater::download(&release.download_url, &temporary).await {
                Err(error) => error.to_string(),
                Ok(()) => match updater::launch_swap(&temporary) {
                    Err(error) => error.to_string(),
                    // The script waits for this process to exit before it
                    // touches anything, so leaving is what completes the update.
                    Ok(()) => match locale {
                        AppLocale::Ru => {
                            format!("Обновление до {version}. Приложение перезапустится.")
                        }
                        AppLocale::En => format!("Updating to {version}. The app will restart."),
                    },
                },
            }
        }
    }
}

fn open_url(url: &str) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/c", "start", "", url])
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(url).spawn();
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
}

/// How often the window needs to be woken.
///
/// Split out from [`Moonlight::subscription`] because it is the part worth
/// asserting on: a `Subscription` is opaque, so a test can only check the
/// decision that produced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cadence {
    Idle,
    Frame,
    Second,
}

impl Cadence {
    pub fn for_state(state: &ConnectionState, page: Page) -> Cadence {
        if state.is_busy() {
            Cadence::Frame
        } else if state.is_connected()
            // These two poll for their own content, so they tick even when the
            // tunnel is down.
            || page == Page::Connections
            || page == Page::Apps
        {
            Cadence::Second
        } else {
            Cadence::Idle
        }
    }
}

/// Whether Windows is in dark mode.
///
/// `AppsUseLightTheme` under `HKCU` is the value the Settings app writes; 0
/// means the apps theme is dark. Cached, because this is consulted on every
/// draw and a wrong answer costs a theme rather than a tunnel.
fn system_prefers_dark() -> bool {
    #[cfg(windows)]
    {
        static CACHE: OnceLock<bool> = OnceLock::new();
        *CACHE.get_or_init(|| {
            let output = std::process::Command::new("reg")
                .args([
                    "query",
                    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
                    "/v",
                    "AppsUseLightTheme",
                ])
                .output();
            match output {
                Ok(output) => !String::from_utf8_lossy(&output.stdout).contains("0x1"),
                Err(_) => true,
            }
        })
    }
    #[cfg(not(windows))]
    {
        true
    }
}

/// Spacers. iced 0.14's `Space` is built rather than constructed, and the two
/// axes are asked for often enough to be worth naming.
pub fn vspace(height: Length) -> Space {
    Space::new().height(height)
}

pub fn hspace(width: Length) -> Space {
    Space::new().width(width)
}

/// Read by the screens, which take `&Moonlight` rather than a dozen arguments.
impl Moonlight {
    pub fn page(&self) -> Page {
        self.page
    }
    pub fn preferences(&self) -> &Preferences {
        &self.preferences
    }
    pub fn state(&self) -> &ConnectionState {
        &self.state
    }
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }
    pub fn info(&self) -> &SubscriptionInfo {
        &self.info
    }
    pub fn source(&self) -> Option<Source> {
        self.source
    }
    pub fn uptime(&self) -> i64 {
        self.uptime_seconds
    }
    pub fn rates(&self) -> (i64, i64) {
        self.rates
    }
    pub fn session(&self) -> (i64, i64) {
        self.session
    }
    pub fn is_probing(&self, node: &str) -> bool {
        self.pending_probes.iter().any(|n| n == node)
    }
    pub fn is_pinging(&self) -> bool {
        self.is_pinging
    }
    pub fn is_refreshing(&self) -> bool {
        self.is_refreshing
    }
    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }
    pub fn apps(&self) -> &[AppEntry] {
        &self.apps
    }
    pub fn is_running(&self, executable: &str) -> bool {
        self.running
            .iter()
            .any(|r| r.eq_ignore_ascii_case(executable))
    }
    pub fn app_search(&self) -> &str {
        &self.app_search
    }
    pub fn rule_kind(&self) -> Kind {
        self.rule_kind
    }
    pub fn rule_value(&self) -> &str {
        &self.rule_value
    }
    pub fn rule_error(&self) -> Option<&str> {
        self.rule_error.as_deref()
    }
    pub fn import_field(&self) -> &str {
        &self.import_field
    }
    pub fn helper_installed(&self) -> bool {
        self.helper_installed
    }
    pub fn update_status(&self) -> Option<&str> {
        self.update_status.as_deref()
    }
    pub fn logs(&self) -> &[LogEntry] {
        &self.logs
    }
    pub fn log_level(&self) -> u8 {
        self.log_level
    }
    pub fn log_filter(&self) -> &str {
        &self.log_filter
    }
    pub fn connections(&self) -> &[Connection] {
        &self.connections
    }
    pub fn locale_of(&self) -> AppLocale {
        self.locale()
    }
    pub fn palette_of(&self) -> Palette {
        self.palette()
    }
    pub fn progress(&self) -> f32 {
        self.transition_progress()
    }

    /// Connections grouped by the process that opened them.
    ///
    /// That is the question people actually bring to this screen: is *this
    /// program* going through the tunnel.
    pub fn connections_by_process(&self) -> Vec<(String, Vec<&Connection>)> {
        let mut grouped: HashMap<&str, Vec<&Connection>> = HashMap::new();
        for connection in &self.connections {
            grouped
                .entry(connection.process.as_str())
                .or_default()
                .push(connection);
        }
        let mut out: Vec<(String, Vec<&Connection>)> = grouped
            .into_iter()
            .map(|(process, list)| (process.to_string(), list))
            .collect();
        // Busiest first: the process with the most open connections is the one
        // the question is usually about.
        out.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(&b.0)));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> Moonlight {
        let mut app = Moonlight::with_preferences(Preferences::default());
        app.persist = false;
        app
    }

    #[test]
    fn every_page_has_a_title_a_subtitle_and_an_icon() {
        for page in [
            Page::Connect,
            Page::Subscription,
            Page::Apps,
            Page::Settings,
            Page::Import,
            Page::Logs,
            Page::Connections,
        ] {
            assert!(!t(page.title(), AppLocale::Ru).is_empty());
            assert!(!t(page.subtitle(), AppLocale::Ru).is_empty());
            assert!(!page.icon().paths().is_empty());
        }
    }

    #[test]
    fn the_sidebar_carries_exactly_the_four_primary_pages() {
        assert_eq!(Page::SIDEBAR.len(), 4);
        for page in [Page::Logs, Page::Connections, Page::Import] {
            assert!(!Page::SIDEBAR.contains(&page));
        }
    }

    #[test]
    fn a_page_off_the_rail_still_lights_its_parent() {
        // Watching the sidebar go dark on the Import screen reads as having
        // left the app.
        assert_eq!(Page::Import.rail_item(), Page::Subscription);
        assert_eq!(Page::Logs.rail_item(), Page::Settings);
        assert_eq!(Page::Connections.rail_item(), Page::Settings);
        assert_eq!(Page::Connect.rail_item(), Page::Connect);
    }

    #[test]
    fn pressing_connect_while_busy_does_nothing() {
        let mut app = app();
        app.state = ConnectionState::Connecting;
        let before = app.transition_started;
        let _ = app.update(Message::ToggleConnection);
        assert_eq!(app.transition_started, before);
    }

    #[test]
    fn the_controller_ends_the_transition_not_a_timer() {
        // A connect that takes eight seconds must not show a settled dial after
        // the animation's four.
        let mut app = app();
        app.transition_started = Some(Instant::now());
        let _ = app.apply(Event::State(ConnectionState::Connected));
        assert!(app.transition_started.is_none());
        assert_eq!(app.state, ConnectionState::Connected);
    }

    #[test]
    fn a_failure_is_kept_where_the_screens_can_show_it() {
        let mut app = app();
        let _ = app.apply(Event::State(ConnectionState::Failed("no route".into())));
        assert_eq!(app.last_error(), Some("no route"));
    }

    #[test]
    fn a_latency_lands_on_its_node_as_it_arrives() {
        let mut app = app();
        app.nodes = vec![Node::new("A", "vless"), Node::new("B", "vless")];
        let _ = app.apply(Event::PingStarted(vec!["A".into(), "B".into()]));
        assert!(app.is_probing("A") && app.is_probing("B"));

        let _ = app.apply(Event::Latency {
            node: "A".into(),
            ms: Some(37),
        });
        assert_eq!(app.nodes[0].latency, Some(37));
        // A still-pending node keeps its spinner while a finished one loses it.
        assert!(!app.is_probing("A"));
        assert!(app.is_probing("B"));
    }

    #[test]
    fn an_unreachable_node_is_cleared_rather_than_left_stale() {
        let mut app = app();
        app.nodes = vec![Node::new("A", "vless")];
        app.nodes[0].latency = Some(37);
        let _ = app.apply(Event::Latency {
            node: "A".into(),
            ms: None,
        });
        assert_eq!(app.nodes[0].latency, None);
    }

    #[test]
    fn the_log_is_bounded() {
        let mut app = app();
        for i in 0..(LOG_LIMIT + 500) {
            let _ = app.apply(Event::Log(LogEntry::app("INFO", format!("line {i}"))));
        }
        assert_eq!(app.logs().len(), LOG_LIMIT);
        // And keeps the newest, which are the ones that say why.
        assert!(app
            .logs()
            .last()
            .unwrap()
            .message
            .ends_with(&format!("{}", LOG_LIMIT + 499)));
    }

    #[test]
    fn picking_a_node_turns_auto_select_off() {
        let mut app = app();
        assert!(app.preferences.auto_select);
        let _ = app.update(Message::SelectNode("Node A".into()));
        assert!(!app.preferences.auto_select);
        assert_eq!(app.preferences.selected_node.as_deref(), Some("Node A"));
    }

    #[test]
    fn the_auto_row_clears_the_selection_rather_than_naming_a_node() {
        let mut app = app();
        let _ = app.update(Message::SelectNode("Node A".into()));
        let _ = app.update(Message::SelectNode(String::new()));
        assert!(app.preferences.auto_select);
        assert_eq!(app.preferences.selected_node, None);
    }

    #[test]
    fn a_bad_rule_is_refused_with_its_reason_rather_than_added() {
        // The core refuses the whole config for one bad rule, so the tunnel
        // stops rather than the rule being skipped.
        let mut app = app();
        app.rule_kind = Kind::IpCidr;
        app.rule_value = "999.1.1.1/24".into();
        let _ = app.update(Message::RuleSubmit);

        assert!(app.preferences.split_rules.is_empty());
        assert!(app.rule_error().is_some());
        // And the value is kept, so the user can correct it rather than retype.
        assert_eq!(app.rule_value(), "999.1.1.1/24");
    }

    #[test]
    fn a_good_rule_is_added_and_the_field_cleared() {
        let mut app = app();
        app.rule_kind = Kind::DomainSuffix;
        app.rule_value = "openai.com".into();
        let _ = app.update(Message::RuleSubmit);

        assert_eq!(app.preferences.split_rules.len(), 1);
        assert_eq!(app.rule_value(), "");
        assert!(app.rule_error().is_none());
    }

    #[test]
    fn removing_the_helper_takes_tun_mode_with_it() {
        // TUN without the service fails on every connect with the same message.
        let mut app = app();
        app.preferences.mode = TunnelMode::Tun;
        let _ = app.update(Message::HelperChanged(false));
        assert_eq!(app.preferences.mode, TunnelMode::SystemProxy);
    }

    #[test]
    fn appearance_cycles_system_dark_light() {
        let mut app = app();
        assert_eq!(app.preferences.appearance, None);
        let _ = app.update(Message::CycleAppearance);
        assert_eq!(app.preferences.appearance.as_deref(), Some("dark"));
        let _ = app.update(Message::CycleAppearance);
        assert_eq!(app.preferences.appearance.as_deref(), Some("light"));
        let _ = app.update(Message::CycleAppearance);
        assert_eq!(app.preferences.appearance, None);
    }

    #[test]
    fn connections_group_by_process_busiest_first() {
        let mut app = app();
        let make = |id: &str, process: &str| Connection {
            id: id.into(),
            chains: vec!["Node A".into()],
            rule: String::new(),
            rule_payload: String::new(),
            network: "TCP".into(),
            host: "example.com:443".into(),
            process: process.into(),
            process_path: String::new(),
            upload: 0,
            download: 0,
            start: time::OffsetDateTime::now_utc(),
        };
        app.connections = vec![
            make("1", "chrome.exe"),
            make("2", "telegram.exe"),
            make("3", "chrome.exe"),
        ];

        let grouped = app.connections_by_process();
        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped[0].0, "chrome.exe");
        assert_eq!(grouped[0].1.len(), 2);
    }

    #[test]
    fn an_idle_window_is_never_woken() {
        assert_eq!(
            Cadence::for_state(&ConnectionState::Disconnected, Page::Connect),
            Cadence::Idle
        );
    }

    #[test]
    fn the_polling_screens_tick_even_when_the_tunnel_is_down() {
        // They poll for their own content, not for the tunnel's.
        for page in [Page::Connections, Page::Apps] {
            assert_eq!(
                Cadence::for_state(&ConnectionState::Disconnected, page),
                Cadence::Second
            );
        }
    }

    #[test]
    fn only_a_transition_asks_for_frames() {
        assert_eq!(
            Cadence::for_state(&ConnectionState::Connecting, Page::Connect),
            Cadence::Frame
        );
        assert_eq!(
            Cadence::for_state(&ConnectionState::Connected, Page::Connect),
            Cadence::Second
        );
    }

    #[test]
    fn the_window_title_carries_the_connection_state() {
        let mut app = app();
        assert!(app.title().contains(t(S::StateDisconnected, AppLocale::Ru)));
        app.state = ConnectionState::Connected;
        assert!(app.title().contains(t(S::StateConnected, AppLocale::Ru)));
    }

    /// Builds every screen's widget tree.
    ///
    /// A `view` that panics — on an empty list, a missing subscription, a
    /// `FillPortion(0)` — does so at the moment the user navigates to it, which
    /// is the worst place to find out. This walks all seven with both an empty
    /// state and a populated one.
    #[test]
    fn every_screen_builds_in_both_the_empty_and_the_populated_state() {
        for populated in [false, true] {
            let mut app = app();
            if populated {
                app.preferences.subscription_url = Some("https://panel/sub".into());
                app.nodes = vec![Node::new("\u{1F1F8}\u{1F1EA} Stockholm", "vless")];
                app.nodes[0].latency = Some(37);
                app.info = SubscriptionInfo {
                    title: Some("Luna".into()),
                    download: Some(1024),
                    total: Some(1024 * 1024),
                    expire: Some(time::OffsetDateTime::now_utc().unix_timestamp() + 86_400),
                    device_limit: Some(5),
                    devices_used: Some(2),
                    ..Default::default()
                };
                app.apps = vec![AppEntry {
                    name: "Google Chrome".into(),
                    executable: "chrome.exe".into(),
                    path: r"C:\chrome.exe".into(),
                }];
                app.running = vec!["chrome.exe".into()];
                app.preferences.split_rules =
                    vec![SplitRule::new(Kind::DomainSuffix, "openai.com")];
                app.logs = vec![LogEntry::app("ERROR", "boom")];
                app.connections = vec![Connection {
                    id: "1".into(),
                    chains: vec!["Node A".into()],
                    rule: "GeoSite".into(),
                    rule_payload: "google".into(),
                    network: "TCP".into(),
                    host: "example.com:443".into(),
                    process: "chrome.exe".into(),
                    process_path: String::new(),
                    upload: 10,
                    download: 20,
                    start: time::OffsetDateTime::now_utc(),
                }];
                app.last_error = Some("something went wrong".into());
                app.update_status = Some("checking".into());
            }

            for page in [
                Page::Connect,
                Page::Subscription,
                Page::Apps,
                Page::Settings,
                Page::Import,
                Page::Logs,
                Page::Connections,
            ] {
                app.page = page;
                // Dropped immediately; building it is the assertion.
                let _ = app.view();
            }
        }
    }

    /// The same, in the other palette and the other language.
    #[test]
    fn every_screen_builds_in_light_mode_and_in_english() {
        let mut app = app();
        app.preferences.appearance = Some("light".into());
        app.preferences.locale = AppLocale::En;
        app.sidebar_collapsed = true;

        for page in [
            Page::Connect,
            Page::Subscription,
            Page::Apps,
            Page::Settings,
            Page::Import,
            Page::Logs,
            Page::Connections,
        ] {
            app.page = page;
            let _ = app.view();
        }
    }

    #[test]
    fn the_ui_keeps_its_own_view_settings_when_the_controller_reports_back() {
        // The controller owns latencies and the proxy snapshot; it must not
        // reach back and undo a sidebar collapse or a theme change the user
        // made while it was working.
        let mut app = app();
        app.preferences.sidebar_collapsed = true;
        app.preferences.appearance = Some("light".into());
        app.preferences.locale = AppLocale::En;

        let mut from_controller = Preferences::default();
        from_controller.record_latency("A", Some(12));
        let _ = app.apply(Event::PreferencesChanged(Box::new(from_controller)));

        assert!(app.preferences.sidebar_collapsed);
        assert_eq!(app.preferences.appearance.as_deref(), Some("light"));
        assert_eq!(app.preferences.locale, AppLocale::En);
        assert_eq!(app.preferences.latency("A"), Some(12));
    }
}
