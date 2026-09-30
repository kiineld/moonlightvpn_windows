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
use moonlight_core::rules::{self, Kind, Priority, RoutingRule};
use moonlight_core::subscription::Source;
use moonlight_core::{
    AppEntry, AppLocale, ConnectionState, Issue, Node, SubscriptionInfo, TunnelMode,
};
use moonlight_design::motion::{self, dur, Curve};
use moonlight_design::{Appearance, Palette};

use localization::{t, S};

/// The brand, as everything a user sees writes it: lower case.
pub const APP_NAME: &str = "moonlight";
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
    None => "https://t.me/the_moonlight_vpn_bot",
};
pub const TELEGRAM_CHANNEL_URL: &str = match option_env!("TELEGRAM_CHANNEL_URL") {
    Some(url) => url,
    None => "https://t.me/moonlight_vpn_channel",
};
pub const SUPPORT_URL: &str = match option_env!("SUPPORT_URL") {
    Some(url) => url,
    None => "https://t.me/moonlight_vps",
};
/// The personal account: devices and the plan, on the service's own site —
/// a different host from the one subscriptions are served from.
pub const CABINET_URL: &str = match option_env!("CABINET_URL") {
    Some(url) => url,
    None => "https://cabinetofficial.rustafield.site",
};

/// The controller's channels, handed over to the subscription once.
///
/// A `static` because iced's `Subscription::run` takes a plain function: there
/// is nowhere to capture a receiver, so it is parked here at construction and
/// taken on the first poll.
type Events = Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<Event>>>;
static EVENTS: OnceLock<Events> = OnceLock::new();
static COMMANDS: OnceLock<tokio::sync::mpsc::UnboundedSender<Command>> = OnceLock::new();

/// The page's scroller, so an update can bring its progress into view.
const PAGE_SCROLL: &str = "page";

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
    //
    // One copy at a time: a second launch hands its request to the first and
    // leaves before it has touched anything — see `moonlight_core::instance`.
    if !moonlight_core::instance::claim() {
        let request = moonlight_core::instance::request_from_args(std::env::args());
        moonlight_core::instance::forward(&request);
        return Ok(());
    }

    // A daemon rather than an application: the app outlives its window (it
    // closes to the tray) and has a second one, the tray panel.
    iced::daemon(Moonlight::new, Moonlight::update, Moonlight::view)
        .title(Moonlight::title)
        .subscription(Moonlight::subscription)
        .theme(Moonlight::iced_theme)
        // The window is cleared to nothing, not to iced's theme colour: the
        // canvas paints itself, and over Mica it paints translucent — an opaque
        // clear underneath would hide the material the canvas lets through.
        .style(Moonlight::clear)
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
    Rules,
    Settings,
    Import,
    Logs,
    Connections,
}

impl Page {
    /// The rail, in the macOS client's order. Connections sits on it — "what is
    /// going through the tunnel right now" is something people open the app to
    /// look at, not a diagnostic buried under Settings. Logs is the diagnostic
    /// and stays there; Import is reached from Subscription.
    pub const SIDEBAR: [Page; 5] = [
        Page::Connect,
        Page::Subscription,
        Page::Rules,
        Page::Connections,
        Page::Settings,
    ];

    pub fn title(self) -> S {
        match self {
            Page::Connect => S::NavConnect,
            Page::Subscription => S::NavSubscription,
            Page::Rules => S::NavRules,
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
            Page::Rules => S::RulesSubtitle,
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
            Page::Rules => Icon::Route,
            Page::Settings => Icon::Settings,
            Page::Import => Icon::Plus,
            Page::Logs => Icon::CircleAlert,
            Page::Connections => Icon::Activity,
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
            Page::Logs => Page::Settings,
            other => other,
        }
    }
}

/// Where an update has got to. One for the whole app: the launch check, the
/// banner and Settings all read and drive this.
#[derive(Debug, Clone, PartialEq)]
pub enum UpdateState {
    Idle,
    Checking,
    UpToDate,
    Available(moonlight_core::updater::Release),
    Downloading {
        release: moonlight_core::updater::Release,
        received: u64,
        total: Option<u64>,
    },
    Verifying(moonlight_core::updater::Release),
    /// Checked and handed over: the app is leaving so Setup can run.
    Installing(String),
    /// Something went wrong, in the app's words; the detail is in the log.
    Failed(String),
}

impl UpdateState {
    pub fn is_busy(&self) -> bool {
        matches!(
            self,
            UpdateState::Checking
                | UpdateState::Downloading { .. }
                | UpdateState::Verifying(_)
                | UpdateState::Installing(_)
        )
    }
}

/// Which list the rules page shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RulesTab {
    Mine,
    Subscription,
}

/// The rule being added or changed.
#[derive(Debug, Clone, PartialEq)]
pub struct RuleEditor {
    /// The rule being changed, or `None` for a new one.
    pub editing: Option<uuid::Uuid>,
    pub kind: Kind,
    pub value: String,
    pub target: String,
    pub priority: Priority,
    pub error: Option<rules::Invalid>,
    /// The program list for a process rule is open.
    pub picker: bool,
    pub picker_filter: String,
}

/// A rule being dragged into a new place.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Drag {
    pub from: usize,
    /// Where the pointer was when the drag began, once it has first moved.
    pub origin: Option<f32>,
    pub offset: f32,
}

impl Drag {
    /// The place the lifted rule would land in a list of `len`.
    fn to(&self, len: usize) -> usize {
        let moved = (self.offset / screens::rules::ROW_HEIGHT).round() as isize;
        (self.from as isize + moved).clamp(0, len.saturating_sub(1) as isize) as usize
    }
}

/// Where the question a `moonlight://` link asks has got to.
#[derive(Debug, Clone, PartialEq)]
pub enum LinkPrompt {
    /// Asking whether to add this subscription.
    Ask(String),
    /// Adding it.
    Adding(String),
    /// It did not load, and why.
    Failed(String, Issue),
    /// The link carried nothing this app can add.
    Invalid,
}

impl LinkPrompt {
    fn for_link(link: &str) -> LinkPrompt {
        match moonlight_core::deeplink::subscription_to_add(link) {
            Some(subscription) => LinkPrompt::Ask(subscription),
            None => LinkPrompt::Invalid,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    Navigate(Page),
    ToggleSidebar,
    ToggleLaunchAtLogin,
    CycleAppearance,
    SetLocale(AppLocale),

    ToggleConnection,
    SelectNode(String),
    Ping,
    Refresh,
    /// Every five minutes: asks the controller to refresh if one is due.
    AutoUpdateTick,
    /// Hours between automatic refreshes; 0 is off.
    SetAutoUpdate(u32),
    /// Put the service's current announcement away.
    DismissAnnounce,
    /// The refresh note at the foot of the connect page goes, if it is still
    /// the one with this id.
    HideRefreshNote(u64),
    /// The service's own support contact when it sent one, else the app's.
    OpenSupport,

    /// The tray icon was clicked, at this point on the screen.
    TrayClicked(moonlight_core::tray::Click),
    /// A second launch asked for something: `show`, or a link.
    Request(String),
    WindowEvent(iced::window::Id, iced::window::Event),
    /// Bring the main window up, opening a new one if it was closed.
    OpenMain,
    /// The window, on this page.
    OpenPage(Page),
    TogglePin,
    TraySearch(String),
    SetRoutingMode(moonlight_core::RoutingMode),
    PingNode(String),
    ToggleNotifications,
    /// Leave: put the machine back, then exit.
    Quit,
    /// A window's native handle, once it exists, to ask for its backdrop.
    WindowHandle(iced::window::Id, isize),
    /// Add the subscription the link carried — or try again.
    LinkAdd,
    LinkDismiss,

    ImportChanged(String),
    ImportSubmit,
    ImportPasted(Option<String>),
    PasteFromClipboard,
    RemoveSubscription,

    SetMode(TunnelMode),
    RulesTab(RulesTab),
    RulesFilter(String),
    RuleNew,
    RuleEdit(uuid::Uuid),
    RuleToggle(uuid::Uuid),
    RuleDelete(uuid::Uuid),
    RulesApply,
    RulesReset,
    EditorKind(Kind),
    EditorValue(String),
    EditorTarget(String),
    EditorPriority(Priority),
    EditorPicker,
    EditorPickerFilter(String),
    EditorPickApp(String),
    EditorSave,
    EditorCancel,
    /// A rule's grip was pressed.
    DragStart(usize),
    /// The pointer is at this height while a rule is lifted.
    DragMove(f32),
    DragEnd,
    AppsScanned(Vec<AppEntry>),
    /// Executable → its own icon, decoded off the UI thread.
    IconsLoaded(Vec<(String, moonlight_core::app_icon::Rgba)>),
    RunningScanned(Vec<String>),

    InstallHelper,
    RemoveHelper,
    HelperChanged(bool),
    HelperAttempted(Result<bool, String>),
    CheckForUpdates,
    /// What a check found; `quiet` for the one at launch, which says nothing
    /// unless there is something to install.
    UpdateFound {
        quiet: bool,
        result: Result<moonlight_core::updater::Outcome, String>,
    },
    /// Download, check and install what was found.
    StartUpdate,
    UpdateDownloaded(Result<std::path::PathBuf, String>),
    UpdateVerified(Result<std::path::PathBuf, String>),
    /// The banner was clicked: to Settings, and install.
    OpenUpdate,
    /// The banner's cross: gone until the next launch.
    HideUpdateBanner,
    /// Bytes received, and the total when the server says.
    UpdateProgress(u64, Option<u64>),
    OpenUrl(&'static str),

    LogFilterLevel(u8),
    LogFilterSource(screens::logs::LogFilter),
    LogFilterText(String),
    ClearLogs,
    CloseConnection(String),
    /// Close everything one program holds open, without unfolding it first.
    CloseProcessConnections(String),
    CloseAllConnections,
    /// Unfold or fold one process's connections.
    ToggleConnectionProcess(String),
    /// A row that exists only to highlight on hover, with no action of its own.
    Ignore,
    ConnectionFilterChanged(String),

    DragWindow,
    ResizeWindow(iced::window::Direction),
    /// The window closes on this, once the controller has finished putting the
    /// machine back — or once the backstop timer runs out.
    ForceClose,
    MinimiseWindow,
    MaximiseWindow,
    CloseWindow,

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
    /// Why the last subscription refresh failed, for the subscription page.
    refresh_issue: Option<Issue>,
    /// A refresh the user asked for is in flight, so its outcome is reported
    /// on the connect page; a scheduled one only goes to the log.
    refresh_asked: bool,
    /// What the last asked-for refresh came to, shown briefly at the foot of
    /// the connect page. The id lets a newer note outlive an older's timer.
    refresh_note: Option<(u64, Result<(), Issue>)>,

    apps: Vec<AppEntry>,
    /// Executable → the programme's own icon, keyed the same way the app list
    /// and the split rules are.
    app_icons: std::collections::HashMap<String, iced::widget::image::Handle>,
    running: Vec<String>,
    /// The user's rules as the rules page has them: a draft until Apply.
    rules_draft: Vec<RoutingRule>,
    rules_tab: RulesTab,
    rules_filter: String,
    rule_editor: Option<RuleEditor>,
    rule_drag: Option<Drag>,
    /// The subscription's groups, which a rule can target, and its own rules.
    routing_groups: Vec<String>,
    profile_rules: Vec<String>,
    rules_applying: bool,
    rules_issue: Option<String>,

    import_field: String,
    /// A subscription is being fetched from the Import screen, and the screen is
    /// waiting to be told it worked.
    importing: bool,
    /// It worked, and the Import screen is showing the confirmation rather than
    /// the form.
    import_done: bool,
    helper_installed: bool,
    update: UpdateState,
    update_banner_hidden: bool,
    /// The checked installer, run once the app has put the machine back.
    pending_installer: Option<std::path::PathBuf>,

    /// When the process started, for the halo's breath.
    started: Instant,
    logs: Vec<LogEntry>,
    log_level: u8,
    log_source: screens::logs::LogFilter,
    log_filter: String,
    connections: Vec<Connection>,
    /// Which processes have their connections unfolded beneath them. The macOS
    /// client expands in place rather than pushing a second screen, so several
    /// programs can be open side by side and compared.
    expanded_processes: std::collections::HashSet<String>,
    connection_filter: String,

    sidebar_collapsed: bool,
    /// When the current screen appeared, for its entrance. Set by anything that
    /// replaces the content wholesale — a page change, a language switch, and
    /// the first frame after launch.
    page_started: Option<Instant>,
    /// When the rail last started opening or closing, for its width glide.
    sidebar_started: Option<Instant>,
    /// When the theme last changed, and the colours it was showing at the time.
    theme_started: Option<Instant>,
    previous_palette: Option<Palette>,
    /// The alpha-2 codes there is a flag picture for, read once from the
    /// `flags/` directory beside the executable. A set rather than a `exists()`
    /// per row per frame.
    flags: std::collections::HashSet<String>,
    /// Off in tests, so preference changes stay in memory.
    persist: bool,

    /// The main window, while it is open. Closing it leaves the app in the
    /// tray; `OpenMain` makes a new one.
    main_window: Option<iced::window::Id>,
    /// The tray panel, while it is open.
    tray_window: Option<iced::window::Id>,
    /// Whether there is a tray icon at all. Without one, closing the window
    /// must quit, or the app would be left running with no way back to it.
    has_tray: bool,
    tray_pinned: bool,
    tray_search: String,
    /// When the panel last closed because it lost focus. A click on the tray
    /// icon takes focus from the panel first, so without this the click that
    /// meant "close" closed it and then opened it again.
    tray_blurred: Option<Instant>,
    /// A `moonlight://` link's question, while it is on screen.
    link_prompt: Option<LinkPrompt>,
    /// The main window's handle, once Windows gave it Mica — the canvas is
    /// painted translucent over it, and a theme change re-tints it. None on
    /// Windows 10, where the canvas stays solid.
    backdrop: Option<isize>,
}

impl Moonlight {
    fn new() -> (Self, Task<Message>) {
        let mut preferences = Preferences::load();
        // The Run key is the fact; the stored flag only mirrors it. A user who
        // removed the entry with msconfig while the app was shut must not come
        // back to a switch that still reads "on".
        preferences.launch_at_login = moonlight_core::autostart::is_enabled();
        // So a link from the bot opens this copy, wherever it now lives.
        moonlight_core::deeplink::register();
        // Rewritten so an entry from an older version, or one pointing at a
        // copy that has since moved, carries this executable and its flag.
        if preferences.launch_at_login {
            moonlight_core::autostart::set_enabled(true);
        }
        let mut app = Moonlight::with_preferences(preferences.clone());

        // The controller owns everything below the UI and runs in its own task.
        let (command_tx, command_rx) = tokio::sync::mpsc::unbounded_channel();
        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
        let _ = COMMANDS.set(command_tx);
        let _ = EVENTS.set(Mutex::new(Some(event_rx)));

        tokio::spawn(async move {
            Controller::new(preferences, event_tx).run(command_rx).await;
        });
        send(Command::Start);

        // The tray icon and the pipe a second launch talks to both feed one
        // stream of messages into the app.
        let (outside_tx, outside_rx) = tokio::sync::mpsc::unbounded_channel();
        let _ = OUTSIDE.set(Mutex::new(Some(outside_rx)));
        if let Some(mut clicks) = moonlight_core::tray::spawn(&app.title_text()) {
            app.has_tray = true;
            let outside = outside_tx.clone();
            tokio::spawn(async move {
                while let Some(click) = clicks.recv().await {
                    let _ = outside.send(Message::TrayClicked(click));
                }
            });
        }
        let (request_tx, mut request_rx) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(moonlight_core::instance::serve(request_tx));
        tokio::spawn(async move {
            while let Some(request) = request_rx.recv().await {
                let _ = outside_tx.send(Message::Request(request));
            }
        });

        // A sign-in launch starts in the tray, as the setting promises — unless
        // there is no tray to start in.
        // Started by a link: it asks as soon as the window is up.
        app.link_prompt = std::env::args()
            .nth(1)
            .filter(|arg| moonlight_core::deeplink::is_link(arg))
            .map(|link| LinkPrompt::for_link(&link));
        let at_sign_in = app.link_prompt.is_none()
            && std::env::args().any(|a| a == moonlight_core::autostart::AUTOSTART_FLAG);
        let window = if at_sign_in && app.has_tray {
            Task::none()
        } else {
            app.open_main()
        };

        // Once per launch, quietly: a failed check goes to the log, and only a
        // version to install reaches the screen.
        let check = Task::perform(check_release(), |result| Message::UpdateFound {
            quiet: true,
            result,
        });

        (
            app,
            Task::batch([
                window,
                check,
                Task::perform(scan_apps(), Message::AppsScanned),
                // Started, not merely checked: the service is on-demand now, so
                // it comes up with the app and goes down with it.
                Task::perform(start_helper(), Message::HelperChanged),
            ]),
        )
    }

    /// Injectable so the tests never read or write the real preferences file —
    /// `Preferences::save` writes to `%APPDATA%`, and a test suite that touches
    /// it both leaks state between runs and edits the user's own settings.
    fn with_preferences(preferences: Preferences) -> Self {
        let sidebar_collapsed = preferences.sidebar_collapsed;
        let rules_draft = preferences.routing_rules.clone();
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
            refresh_issue: None,
            refresh_asked: false,
            refresh_note: None,
            apps: Vec::new(),
            app_icons: std::collections::HashMap::new(),
            running: Vec::new(),
            rules_draft,
            rules_tab: RulesTab::Mine,
            rules_filter: String::new(),
            rule_editor: None,
            rule_drag: None,
            routing_groups: Vec::new(),
            profile_rules: Vec::new(),
            rules_applying: false,
            rules_issue: None,
            import_field: String::new(),
            importing: false,
            import_done: false,
            helper_installed: false,
            update: UpdateState::Idle,
            update_banner_hidden: false,
            pending_installer: None,
            started: Instant::now(),
            logs: Vec::new(),
            log_level: 1,
            log_source: screens::logs::LogFilter::default(),
            log_filter: String::new(),
            connections: Vec::new(),
            expanded_processes: std::collections::HashSet::new(),
            connection_filter: String::new(),
            sidebar_collapsed,
            // Non-None from the start, so the first screen rises in rather than
            // being simply present when the window appears.
            page_started: Some(Instant::now()),
            sidebar_started: None,
            theme_started: None,
            previous_palette: None,
            flags: available_flags(),
            persist: true,
            main_window: None,
            tray_window: None,
            has_tray: false,
            tray_pinned: false,
            tray_search: String::new(),
            tray_blurred: None,
            link_prompt: None,
            backdrop: None,
        }
    }

    fn title(&self, _window: iced::window::Id) -> String {
        self.title_text()
    }

    fn title_text(&self) -> String {
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

    fn clear(&self, _theme: &iced::Theme) -> iced::theme::Style {
        iced::theme::Style {
            background_color: iced::Color::TRANSPARENT,
            text_color: self.palette().text,
        }
    }

    fn iced_theme(&self, _window: iced::window::Id) -> iced::Theme {
        iced::Theme::Dark
    }

    fn locale(&self) -> AppLocale {
        self.preferences.locale
    }

    /// The palette the app is currently painting with — part-way between the
    /// old and new themes while a switch is in flight.
    fn palette(&self) -> Palette {
        let target = self.target_palette();
        let Some(started) = self.theme_started else {
            return target;
        };
        let Some(previous) = self.previous_palette else {
            return target;
        };
        let linear = motion::progress(started.elapsed(), dur::PAINT);
        Palette::lerp(&previous, &target, Curve::EASE.at(linear))
    }

    /// Whether the theme is the dark one, whichever way it was chosen.
    fn is_dark(&self) -> bool {
        match self.preferences.appearance.as_deref() {
            Some("dark") => true,
            Some("light") => false,
            _ => system_prefers_dark(),
        }
    }

    /// Where the theme is heading, ignoring any fade in progress.
    fn target_palette(&self) -> Palette {
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
    /// How far a screen is through its entrance, eased.
    ///
    /// The design's `ml-rise` is an 18px lift paired with a fade. iced has no
    /// opacity for an arbitrary element — only images and SVGs carry one — so
    /// this is the lift alone rather than a fade faked by threading an alpha
    /// through every colour on every screen.
    fn page_rise(&self) -> f32 {
        const TRAVEL: f32 = 18.0;
        let Some(started) = self.page_started else {
            return 0.0;
        };
        let linear = motion::progress(started.elapsed(), dur::ENTER);
        TRAVEL * (1.0 - Curve::RISE.at(linear))
    }

    /// The rail's width part-way through opening or closing.
    fn sidebar_width(&self) -> f32 {
        use moonlight_design::motion::metrics;
        let (from, to) = if self.sidebar_collapsed {
            (metrics::RAIL, metrics::RAIL_COLLAPSED)
        } else {
            (metrics::RAIL_COLLAPSED, metrics::RAIL)
        };
        let Some(started) = self.sidebar_started else {
            return to;
        };
        let linear = motion::progress(started.elapsed(), dur::SLIDE);
        from + (to - from) * Curve::SLIDE.at(linear)
    }

    /// Whether anything is mid-animation, so the subscription knows to keep
    /// asking for frames.
    fn is_animating(&self) -> bool {
        let running = |started: Option<Instant>, duration: Duration| {
            started.is_some_and(|at| at.elapsed() < duration)
        };
        running(self.page_started, dur::ENTER)
            || running(self.sidebar_started, dur::SLIDE)
            || running(self.theme_started, dur::PAINT)
    }

    fn transition_progress(&self) -> f32 {
        let Some(started) = self.transition_started else {
            return 1.0;
        };
        moonlight_design::motion::progress(started.elapsed(), moonlight_design::dur::SLIDE)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Navigate(page) => {
                // Re-entering the page you are already on should not replay the
                // entrance: it reads as the app having lost its place.
                if self.page != page {
                    self.page = page;
                    self.page_started = Some(Instant::now());
                    if page != Page::Import {
                        self.importing = false;
                        self.import_done = false;
                    }
                }
            }
            Message::ToggleSidebar => {
                self.sidebar_collapsed = !self.sidebar_collapsed;
                self.preferences.sidebar_collapsed = self.sidebar_collapsed;
                self.sidebar_started = Some(Instant::now());
                self.save();
            }
            Message::CycleAppearance => {
                // Where the colours are *now*, which is not necessarily the old
                // theme: pressing the button twice quickly has to fade on from
                // the half-blended palette rather than snapping back first.
                self.previous_palette = Some(self.palette());
                // System → dark → light → system, which is the order the macOS
                // client's sun button cycles in.
                self.preferences.appearance = match self.preferences.appearance.as_deref() {
                    None => Some("dark".into()),
                    Some("dark") => Some("light".into()),
                    _ => None,
                };
                // The material has its own dark and light tint, and keeps the
                // one it was given until told otherwise.
                if let Some(hwnd) = self.backdrop {
                    moonlight_core::backdrop::apply(
                        hwnd,
                        moonlight_core::backdrop::Material::Mica,
                        self.is_dark(),
                    );
                }
                self.theme_started = Some(Instant::now());
                self.save();
            }
            Message::SetLocale(locale) => {
                if self.preferences.locale != locale {
                    self.preferences.locale = locale;
                    // Every string on screen has just been replaced, so the
                    // screen replays its entrance rather than swapping the words
                    // underneath the reader.
                    self.page_started = Some(Instant::now());
                    self.save();
                }
            }
            Message::ToggleLaunchAtLogin => {
                let wanted = !self.preferences.launch_at_login;
                // The registry is the fact, the preference only mirrors it. If
                // the write is refused the switch stays where it was rather than
                // showing a state Windows will not honour.
                if moonlight_core::autostart::set_enabled(wanted) {
                    self.preferences.launch_at_login = wanted;
                    self.save();
                } else {
                    self.last_error = Some(t(S::AutostartFailed, self.locale()).to_string());
                }
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
            Message::Refresh => {
                self.refresh_asked = true;
                send(Command::Refresh);
            }
            Message::AutoUpdateTick => send(Command::RefreshIfDue),
            Message::SetAutoUpdate(hours) => {
                self.preferences.auto_update_hours = Some(hours);
                self.save();
                send(Command::SetAutoUpdate(hours));
            }
            Message::DismissAnnounce => {
                self.preferences.dismissed_announce = self.info.announce.clone();
                self.save();
            }
            Message::HideRefreshNote(id) => {
                if self
                    .refresh_note
                    .as_ref()
                    .is_some_and(|(shown, _)| *shown == id)
                {
                    self.refresh_note = None;
                }
            }
            Message::OpenSupport => {
                let url = self.info.support_url.clone();
                open_url(url.as_deref().unwrap_or(SUPPORT_URL));
            }

            Message::ImportChanged(value) => self.import_field = value,
            Message::ImportSubmit => {
                let url = self.import_field.trim().to_string();
                if !url.is_empty() {
                    send(Command::ImportSubscription(url));
                    self.import_field.clear();
                    self.last_error = None;
                    // Stays on this screen: the import is worth confirming where
                    // it was asked for, rather than dropping the user on the
                    // subscription page to work out whether it took.
                    self.importing = true;
                    self.import_done = false;
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
            Message::RulesTab(tab) => self.rules_tab = tab,
            Message::RulesFilter(value) => self.rules_filter = value,
            Message::RuleNew => {
                self.rule_editor = Some(RuleEditor {
                    editing: None,
                    kind: Kind::DomainSuffix,
                    value: String::new(),
                    target: rules::DIRECT.to_string(),
                    priority: Priority::Override,
                    error: None,
                    picker: false,
                    picker_filter: String::new(),
                });
            }
            Message::RuleEdit(id) => {
                if let Some(rule) = self.rules_draft.iter().find(|r| r.id == id) {
                    self.rule_editor = Some(RuleEditor {
                        editing: Some(id),
                        kind: rule.kind,
                        value: rule.value.clone(),
                        target: rule.target.clone(),
                        priority: rule.priority,
                        error: None,
                        picker: false,
                        picker_filter: String::new(),
                    });
                }
            }
            Message::RuleToggle(id) => {
                if let Some(rule) = self.rules_draft.iter_mut().find(|r| r.id == id) {
                    rule.enabled = !rule.enabled;
                }
            }
            Message::RuleDelete(id) => self.rules_draft.retain(|r| r.id != id),
            Message::RulesReset => {
                self.rules_draft = self.preferences.routing_rules.clone();
                self.rules_issue = None;
            }
            Message::RulesApply => {
                self.rules_applying = true;
                self.rules_issue = None;
                send(Command::ApplyRules(self.rules_draft.clone()));
            }
            Message::EditorKind(kind) => {
                if let Some(editor) = &mut self.rule_editor {
                    editor.kind = kind;
                    editor.error = None;
                    editor.picker &= kind.needs_process_matching();
                }
            }
            Message::EditorValue(value) => {
                if let Some(editor) = &mut self.rule_editor {
                    editor.value = value;
                    editor.error = None;
                }
            }
            Message::EditorTarget(target) => {
                if let Some(editor) = &mut self.rule_editor {
                    editor.target = target;
                }
            }
            Message::EditorPriority(priority) => {
                if let Some(editor) = &mut self.rule_editor {
                    editor.priority = priority;
                }
            }
            Message::EditorPicker => {
                if let Some(editor) = &mut self.rule_editor {
                    editor.picker = !editor.picker;
                    if editor.picker {
                        return Task::perform(scan_running(), Message::RunningScanned);
                    }
                }
            }
            Message::EditorPickerFilter(value) => {
                if let Some(editor) = &mut self.rule_editor {
                    editor.picker_filter = value;
                }
            }
            Message::EditorPickApp(value) => {
                if let Some(editor) = &mut self.rule_editor {
                    editor.value = value;
                    editor.error = None;
                    editor.picker = false;
                }
            }
            Message::EditorCancel => self.rule_editor = None,
            Message::EditorSave => {
                // Checked before it is kept, against the grammar the core
                // applies: one bad rule and the core refuses the whole config.
                if let Some(editor) = &mut self.rule_editor {
                    if let Some(invalid) = rules::validate(editor.kind, &editor.value) {
                        editor.error = Some(invalid);
                        return Task::none();
                    }
                    let rule = RoutingRule {
                        id: editor.editing.unwrap_or_else(uuid::Uuid::new_v4),
                        kind: editor.kind,
                        value: editor.value.trim().to_string(),
                        target: editor.target.clone(),
                        priority: editor.priority,
                        enabled: true,
                    };
                    match self
                        .rules_draft
                        .iter_mut()
                        .find(|r| Some(r.id) == editor.editing)
                    {
                        Some(existing) => {
                            let enabled = existing.enabled;
                            *existing = RoutingRule { enabled, ..rule };
                        }
                        None => self.rules_draft.push(rule),
                    }
                    self.rule_editor = None;
                }
            }
            Message::DragStart(index) => {
                self.rule_drag = Some(Drag {
                    from: index,
                    origin: None,
                    offset: 0.0,
                });
            }
            Message::DragMove(y) => {
                if let Some(drag) = &mut self.rule_drag {
                    let origin = *drag.origin.get_or_insert(y);
                    drag.offset = y - origin;
                }
            }
            Message::DragEnd => {
                if let Some(drag) = self.rule_drag.take() {
                    let to = drag.to(self.rules_draft.len());
                    if to != drag.from && drag.from < self.rules_draft.len() {
                        let rule = self.rules_draft.remove(drag.from);
                        self.rules_draft.insert(to, rule);
                    }
                }
            }
            Message::AppsScanned(apps) => {
                let executables: Vec<(String, String)> = apps
                    .iter()
                    .map(|entry| (entry.executable.clone(), entry.path.clone()))
                    .collect();
                self.apps = apps;
                // Decoding several hundred icons is GDI work measured in whole
                // seconds; it does not belong on the frame the list first draws.
                //
                // Split into batches rather than run as one job: each batch
                // reports as it lands, so the icons fill in over the first
                // second instead of the whole list staying lettered until the
                // last executable has been read.
                return Task::batch(
                    executables
                        .chunks(ICON_BATCH)
                        .map(|batch| {
                            Task::perform(load_icons(batch.to_vec()), Message::IconsLoaded)
                        })
                        .collect::<Vec<_>>(),
                );
            }
            Message::IconsLoaded(icons) => {
                for (executable, rgba) in icons {
                    self.app_icons.insert(
                        executable,
                        iced::widget::image::Handle::from_rgba(
                            rgba.width,
                            rgba.height,
                            rgba.pixels,
                        ),
                    );
                }
            }
            Message::RunningScanned(running) => self.running = running,

            Message::InstallHelper => {
                return Task::perform(install_helper(true), Message::HelperAttempted)
            }
            Message::RemoveHelper => {
                return Task::perform(install_helper(false), Message::HelperAttempted)
            }
            Message::HelperAttempted(result) => {
                let installed = match result {
                    Ok(installed) => {
                        self.last_error = None;
                        installed
                    }
                    Err(reason) => {
                        let key = match reason.as_str() {
                            HELPER_MISSING => S::HelperMissingBinary,
                            _ => S::HelperInstallFailed,
                        };
                        self.last_error = Some(t(key, self.locale()).to_string());
                        self.helper_installed
                    }
                };
                return self.update(Message::HelperChanged(installed));
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
                if self.update.is_busy() {
                    return Task::none();
                }
                self.update = UpdateState::Checking;
                return Task::perform(check_release(), |result| Message::UpdateFound {
                    quiet: false,
                    result,
                });
            }
            Message::UpdateFound { quiet, result } => {
                use moonlight_core::updater::Outcome;
                self.update = match result {
                    Ok(Outcome::Available(release)) => UpdateState::Available(release),
                    Ok(Outcome::UpToDate { .. }) if quiet => UpdateState::Idle,
                    Ok(Outcome::UpToDate { .. }) => UpdateState::UpToDate,
                    Err(detail) => {
                        self.logs.push(LogEntry::app(
                            "WARNING",
                            format!("Update check failed: {detail}"),
                        ));
                        if quiet {
                            UpdateState::Idle
                        } else {
                            UpdateState::Failed(t(S::UpdateCheckFailed, self.locale()).to_string())
                        }
                    }
                };
            }
            Message::OpenUpdate => {
                self.page = Page::Settings;
                self.page_started = Some(Instant::now());
                self.update_banner_hidden = true;
                return Task::batch([self.update(Message::StartUpdate), self.open_main()]);
            }
            Message::HideUpdateBanner => self.update_banner_hidden = true,
            Message::StartUpdate => {
                let UpdateState::Available(release) = self.update.clone() else {
                    return Task::none();
                };
                self.update = UpdateState::Downloading {
                    total: (release.size > 0).then_some(release.size),
                    release: release.clone(),
                    received: 0,
                };
                // The progress sits at the foot of Settings; brought into view,
                // or the wait happens somewhere nobody is looking.
                return Task::batch([
                    Task::run(download_stream(release), |message| message),
                    iced::widget::operation::snap_to_end(PAGE_SCROLL),
                ]);
            }
            Message::UpdateProgress(received, total) => {
                if let UpdateState::Downloading {
                    received: got,
                    total: size,
                    ..
                } = &mut self.update
                {
                    *got = received;
                    if total.is_some() {
                        *size = total;
                    }
                }
            }
            Message::UpdateDownloaded(result) => {
                let UpdateState::Downloading { release, .. } = self.update.clone() else {
                    return Task::none();
                };
                match result {
                    Err(detail) => {
                        self.logs.push(LogEntry::app(
                            "WARNING",
                            format!("Update download failed: {detail}"),
                        ));
                        self.update = UpdateState::Failed(
                            t(S::UpdateDownloadFailed, self.locale()).to_string(),
                        );
                    }
                    Ok(path) => {
                        self.update = UpdateState::Verifying(release.clone());
                        return Task::perform(
                            async move {
                                moonlight_core::updater::verify(&release, &path)
                                    .await
                                    .map(|()| path)
                                    .map_err(|e| e.to_string())
                            },
                            Message::UpdateVerified,
                        );
                    }
                }
            }
            Message::UpdateVerified(result) => {
                let UpdateState::Verifying(release) = self.update.clone() else {
                    return Task::none();
                };
                match result {
                    // A damaged download changes nothing: the running version
                    // stays, and says so.
                    Err(detail) => {
                        self.logs.push(LogEntry::app(
                            "WARNING",
                            format!("Update rejected: {detail}"),
                        ));
                        self.update =
                            UpdateState::Failed(t(S::UpdateCorrupt, self.locale()).to_string());
                    }
                    // Setup replaces this very binary, so the app goes first —
                    // through the ordinary quit, which puts the proxy back and
                    // stops the helper — and Setup starts as it leaves.
                    Ok(path) => {
                        self.pending_installer = Some(path);
                        self.update = UpdateState::Installing(release.version);
                        return self.update(Message::Quit);
                    }
                }
            }
            Message::OpenUrl(url) => open_url(url),

            Message::LogFilterLevel(level) => self.log_level = level,
            Message::LogFilterSource(source) => self.log_source = source,
            Message::LogFilterText(value) => self.log_filter = value,
            Message::ClearLogs => self.logs.clear(),
            Message::Ignore => {}
            Message::ToggleConnectionProcess(process) => {
                if !self.expanded_processes.remove(&process) {
                    self.expanded_processes.insert(process);
                }
            }
            Message::ConnectionFilterChanged(value) => self.connection_filter = value,
            Message::CloseConnection(id) => send(Command::CloseConnection(id)),
            Message::CloseProcessConnections(process) => {
                for connection in self.connections.iter().filter(|c| c.process == process) {
                    send(Command::CloseConnection(connection.id.clone()));
                }
            }
            Message::CloseAllConnections => send(Command::CloseAllConnections),

            // The window is undecorated, so moving, minimising, maximising and
            // closing it are all this app's job.
            Message::DragWindow => return self.on_main(iced::window::drag),
            Message::ResizeWindow(direction) => {
                return self.on_main(move |id| iced::window::drag_resize(id, direction))
            }
            Message::MinimiseWindow => return self.on_main(|id| iced::window::minimize(id, true)),
            Message::MaximiseWindow => return self.on_main(iced::window::toggle_maximize),

            Message::TrayClicked(click) => return self.toggle_tray(click),
            Message::Request(request) => {
                // A link asks its question in the window; anything else a
                // second launch wants is the window itself.
                if moonlight_core::deeplink::is_link(&request) {
                    self.link_prompt = Some(LinkPrompt::for_link(&request));
                }
                return self.update(Message::OpenMain);
            }
            Message::LinkAdd => {
                if let Some(LinkPrompt::Ask(link) | LinkPrompt::Failed(link, _)) =
                    self.link_prompt.clone()
                {
                    send(Command::ImportSubscription(link.clone()));
                    self.link_prompt = Some(LinkPrompt::Adding(link));
                }
            }
            Message::LinkDismiss => self.link_prompt = None,
            Message::WindowEvent(id, event) => return self.window_event(id, event),
            Message::OpenMain => {
                let close_tray = match (self.tray_window, self.tray_pinned) {
                    (Some(id), false) => {
                        self.tray_window = None;
                        iced::window::close(id)
                    }
                    _ => Task::none(),
                };
                return Task::batch([close_tray, self.open_main()]);
            }
            Message::OpenPage(page) => {
                self.page = page;
                self.page_started = Some(Instant::now());
                return self.update(Message::OpenMain);
            }
            Message::TogglePin => self.tray_pinned = !self.tray_pinned,
            Message::WindowHandle(id, hwnd) => {
                if hwnd != 0 && Some(id) == self.main_window {
                    let took = moonlight_core::backdrop::apply(
                        hwnd,
                        moonlight_core::backdrop::Material::Mica,
                        self.is_dark(),
                    );
                    self.backdrop = took.then_some(hwnd);
                    self.logs.push(LogEntry::app(
                        "INFO",
                        format!("Window backdrop: {}", if took { "Mica" } else { "none" }),
                    ));
                }
            }
            Message::TraySearch(value) => self.tray_search = value,
            Message::SetRoutingMode(mode) => {
                self.preferences.routing_mode = mode;
                send(Command::SetRoutingMode(mode));
            }
            Message::PingNode(name) => send(Command::PingNode(name)),
            Message::ToggleNotifications => {
                self.preferences.notifications = !self.preferences.notifications;
                self.save();
                self.check_alerts();
            }
            Message::CloseWindow => {
                if !self.has_tray {
                    return self.update(Message::Quit);
                }
                if let Some(id) = self.main_window.take() {
                    return iced::window::close(id);
                }
            }
            Message::Quit => {
                // Put the machine's proxy settings back before the app goes.
                // Closing without this leaves every browser pointed at a core
                // that is about to exit.
                //
                // The close waits for the controller to answer rather than
                // racing it: `send` only queues the command, so closing straight
                // afterwards could kill the process before the proxy was
                // restored or the helper's core stopped. The timer is the
                // backstop for a controller that never answers — a window that
                // will not close is worse than a missed restore.
                send(Command::Shutdown);
                return Task::perform(tokio::time::sleep(Duration::from_secs(6)), |()| {
                    Message::ForceClose
                });
            }
            // The backstop for a controller that never answered. Not
            // `iced::exit()`: that still waits for the runtime, which is
            // exactly what is stuck.
            Message::ForceClose => {
                self.launch_pending_installer();
                moonlight_core::tray::remove();
                std::process::exit(0);
            }

            Message::Controller(event) => return self.apply(event),
            Message::Tick(_) => {
                if self.transition_progress() >= 1.0 {
                    self.transition_started = None;
                }
                if self.page == Page::Connections {
                    send(Command::RefreshConnections);
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
                if let ConnectionState::Failed(issue) = &state {
                    self.last_error = Some(localization::issue(issue, self.locale()));
                }
                self.state = state;
                moonlight_core::tray::set_tooltip(&self.title_text());
            }
            Event::Nodes(nodes) => self.nodes = nodes,
            Event::Info(info) => {
                self.info = info;
                self.check_alerts();
            }
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
                    // The probe has now happened, whatever it found. Without
                    // this a node that answered nothing kept reading as a dash —
                    // "not measured" — when it had in fact been asked and stayed
                    // silent, which is exactly the `n/a` case.
                    entry.probed = true;
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
            Event::Error(issue) => {
                self.last_error = Some(localization::issue(&issue, self.locale()));
            }
            Event::Refreshed(outcome) => {
                // A link that did not load says nothing about the subscription
                // in use, so only a success touches its status.
                if let Some(LinkPrompt::Adding(link)) = self.link_prompt.clone() {
                    self.link_prompt = match &outcome {
                        Ok(()) => {
                            self.refresh_issue = None;
                            self.page = Page::Connect;
                            self.page_started = Some(Instant::now());
                            None
                        }
                        Err(issue) => Some(LinkPrompt::Failed(link, issue.clone())),
                    };
                    return Task::none();
                }
                if !self.importing || outcome.is_ok() {
                    self.refresh_issue = outcome.as_ref().err().cloned();
                }
                if self.importing {
                    // A failed import goes back to the form with the reason on
                    // it, rather than sitting on a spinner that never resolves.
                    self.importing = false;
                    match &outcome {
                        Ok(()) => self.import_done = true,
                        Err(issue) => {
                            self.last_error = Some(localization::issue(issue, self.locale()))
                        }
                    }
                }
                if std::mem::take(&mut self.refresh_asked) {
                    let id = self.refresh_note.as_ref().map_or(0, |(id, _)| id + 1);
                    // Long enough to read; a failure carries a reason, so it
                    // stays longer.
                    let shown_for = if outcome.is_ok() { 3200 } else { 6000 };
                    self.refresh_note = Some((id, outcome));
                    return Task::perform(
                        tokio::time::sleep(Duration::from_millis(shown_for)),
                        move |()| Message::HideRefreshNote(id),
                    );
                }
            }
            Event::RoutingInputs { groups, rules } => {
                self.routing_groups = groups;
                self.profile_rules = rules;
            }
            Event::RulesApplied(outcome) => {
                self.rules_applying = false;
                self.rules_issue = outcome
                    .err()
                    .map(|issue| localization::issue(&issue, self.locale()));
            }
            Event::ShutdownComplete => {
                self.launch_pending_installer();
                moonlight_core::tray::remove();
                return iced::exit();
            }
            Event::PreferencesChanged(preferences) => {
                // The controller owns the parts of preferences it changes —
                // latencies, the proxy snapshot — so its copy wins for those,
                // while the view settings the UI edits are already correct here
                // and must not be undone by a report that crossed with them.
                // A draft nobody has touched follows what was applied — the
                // carry-over from the apps screen lands here, and so does Apply.
                let clean = self.rules_draft == self.preferences.routing_rules;
                let sidebar = self.preferences.sidebar_collapsed;
                let appearance = self.preferences.appearance.clone();
                let locale = self.preferences.locale;
                let dismissed = self.preferences.dismissed_announce.clone();
                let notifications = self.preferences.notifications;
                let sent_alerts = std::mem::take(&mut self.preferences.sent_alerts);
                self.preferences = *preferences;
                self.preferences.sidebar_collapsed = sidebar;
                self.preferences.appearance = appearance;
                self.preferences.locale = locale;
                self.preferences.dismissed_announce = dismissed;
                if clean {
                    self.rules_draft = self.preferences.routing_rules.clone();
                }
                self.preferences.notifications = notifications;
                self.preferences.sent_alerts = sent_alerts;
            }
        }
        Task::none()
    }

    /// Runs the checked installer, if an update is waiting on the app to leave.
    fn launch_pending_installer(&mut self) {
        let Some(installer) = self.pending_installer.take() else {
            return;
        };
        let arguments =
            moonlight_core::updater::installer_arguments(self.locale() == AppLocale::Ru);
        let _ = moonlight_core::updater::launch_installer(&installer, &arguments);
    }

    /// A window operation on the main window, if it is open.
    fn on_main(&self, operation: impl FnOnce(iced::window::Id) -> Task<Message>) -> Task<Message> {
        self.main_window.map_or_else(Task::none, operation)
    }

    /// Brings the main window forward, or opens a new one if it was closed.
    fn open_main(&mut self) -> Task<Message> {
        if let Some(id) = self.main_window {
            return Task::batch([
                iced::window::set_mode(id, iced::window::Mode::Windowed),
                iced::window::gain_focus(id),
            ]);
        }
        let (id, opened) = iced::window::open(iced::window::Settings {
            size: iced::Size::new(1240.0, 820.0),
            position: iced::window::Position::Centered,
            decorations: false,
            // Transparent, so Windows 11's Mica can show through the canvas.
            transparent: true,
            // Alt+F4 arrives as a request, and goes to the tray like the ×.
            exit_on_close_request: false,
            ..Default::default()
        });
        self.main_window = Some(id);
        self.page_started = Some(Instant::now());
        Task::batch([
            opened.map(|_| Message::Ignore),
            iced::window::run(id, native_handle).map(move |hwnd| Message::WindowHandle(id, hwnd)),
        ])
    }

    /// Opens the tray panel above the click, or closes it.
    fn toggle_tray(&mut self, click: moonlight_core::tray::Click) -> Task<Message> {
        if let Some(id) = self.tray_window.take() {
            return iced::window::close(id);
        }
        if self
            .tray_blurred
            .is_some_and(|at| at.elapsed() < Duration::from_millis(400))
        {
            return Task::none();
        }
        let (width, height) = (screens::tray::WIDTH, screens::tray::HEIGHT);
        // Placed in physical pixels against the work area — beside the click,
        // above a taskbar at the bottom or below one at the top — and handed
        // to the window in logical ones.
        let position =
            moonlight_core::tray::work_area().map(|(left, top, right, bottom, scale)| {
                let (w, h, gap) = (width * scale, height * scale, 12.0 * scale);
                let x = (click.x as f32 - w / 2.0).clamp(left as f32 + gap, right as f32 - w - gap);
                let y = if (click.y - top) < (bottom - click.y) {
                    top as f32 + gap
                } else {
                    bottom as f32 - h - gap
                };
                iced::Point::new(x / scale, y / scale)
            });
        let (id, opened) = iced::window::open(iced::window::Settings {
            size: iced::Size::new(width, height),
            position: position.map_or(
                iced::window::Position::Default,
                iced::window::Position::Specific,
            ),
            decorations: false,
            resizable: false,
            level: iced::window::Level::AlwaysOnTop,
            exit_on_close_request: false,
            platform_specific: iced::window::settings::PlatformSpecific {
                skip_taskbar: true,
                ..Default::default()
            },
            ..Default::default()
        });
        self.tray_window = Some(id);
        Task::batch([
            opened.map(|_| Message::Ignore),
            iced::window::gain_focus(id),
        ])
    }

    fn window_event(&mut self, id: iced::window::Id, event: iced::window::Event) -> Task<Message> {
        use iced::window::Event;
        match event {
            Event::CloseRequested if Some(id) == self.main_window => {
                self.update(Message::CloseWindow)
            }
            Event::CloseRequested => iced::window::close(id),
            // The panel goes when the user looks elsewhere, unless pinned.
            Event::Unfocused if Some(id) == self.tray_window && !self.tray_pinned => {
                self.tray_window = None;
                self.tray_blurred = Some(Instant::now());
                iced::window::close(id)
            }
            Event::Closed => {
                if Some(id) == self.main_window {
                    self.main_window = None;
                }
                if Some(id) == self.tray_window {
                    self.tray_window = None;
                }
                Task::none()
            }
            _ => Task::none(),
        }
    }

    /// Expiry and traffic warnings, each sent once, and only with the switch on.
    ///
    /// Warned three days out and each day after, once the plan has ended, when
    /// less than a tenth of the quota is left, and when none is. A top-up makes
    /// the next shortfall news again.
    fn check_alerts(&mut self) {
        if !self.preferences.notifications || self.preferences.subscription_url.is_none() {
            return;
        }
        let locale = self.locale();
        let info = &self.info;
        let mut due: Vec<(String, S, String)> = Vec::new();

        if let (Some(expire), Some(days)) = (info.expire, info.days_left()) {
            if days == 0 {
                due.push((
                    format!("expired-{expire}"),
                    S::NotifyExpiredTitle,
                    t(S::NotifyExpiredBody, locale).to_string(),
                ));
            } else if days <= 3 {
                due.push((
                    format!("expiring-{expire}-{days}"),
                    S::NotifyExpiringTitle,
                    t(S::NotifyExpiringBody, locale).replace(
                        "{days}",
                        &moonlight_core::format::time_left(Some(expire), locale),
                    ),
                ));
            }
        }
        if let (Some(total), Some(used)) = (info.total.filter(|t| *t > 0), info.used()) {
            let left = (total - used).max(0);
            if left == 0 {
                due.push((
                    format!("traffic-out-{total}"),
                    S::NotifyTrafficOutTitle,
                    t(S::NotifyTrafficOutBody, locale).to_string(),
                ));
            } else if (left as f64) < total as f64 * 0.1 {
                due.push((
                    format!("traffic-low-{total}"),
                    S::NotifyTrafficLowTitle,
                    t(S::NotifyTrafficLowBody, locale)
                        .replace("{left}", &moonlight_core::format::bytes(Some(left), locale))
                        .replace(
                            "{total}",
                            &moonlight_core::format::bytes(Some(total), locale),
                        ),
                ));
            } else {
                // Topped up: the next time it runs low is a new warning.
                self.preferences
                    .sent_alerts
                    .retain(|id| !id.starts_with("traffic-"));
            }
        }

        let mut sent_any = false;
        for (id, title, body) in due {
            if self.preferences.sent_alerts.contains(&id) {
                continue;
            }
            moonlight_core::tray::notify(t(title, locale), &body);
            self.preferences.sent_alerts.push(id);
            sent_any = true;
        }
        if sent_any {
            let excess = self.preferences.sent_alerts.len().saturating_sub(40);
            self.preferences.sent_alerts.drain(..excess);
        }
        self.save();
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = vec![
            Subscription::run(controller_events),
            Subscription::run(outside_events),
            iced::window::events().map(|(id, event)| Message::WindowEvent(id, event)),
            // The controller decides whether anything is due; this only asks.
            // A timer rather than a wake notification: after sleep the missed
            // tick fires at once, which is the catch-up a wake handler would do.
            iced::time::every(Duration::from_secs(300)).map(|_| Message::AutoUpdateTick),
        ];
        // While a rule is lifted, the pointer anywhere in the window moves it.
        if self.rule_drag.is_some() {
            subscriptions.push(iced::event::listen_with(drag_events));
        }

        // An entrance or a rail glide needs frames regardless of what the
        // tunnel is doing, and both are short.
        if self.is_animating() {
            subscriptions.push(iced::time::every(Duration::from_millis(16)).map(Message::Tick));
            return Subscription::batch(subscriptions);
        }

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

    fn view(&self, window: iced::window::Id) -> Element<'_, Message> {
        if Some(window) == self.tray_window {
            return screens::tray::view(self);
        }
        self.main_view()
    }

    fn main_view(&self) -> Element<'_, Message> {
        let palette = self.palette();
        let locale = self.locale();

        // The connect screen is laid out to *fit* — the composition gives it
        // `height:100%` and no scroller — so it is placed directly. Wrapping it
        // in a scrollable gives its column an unbounded height, and every
        // `Length::Fill` inside then expands into that infinity and squeezes the
        // dial out of the layout entirely.
        let scrolls = self.page != Page::Connect;

        let body = match self.page {
            Page::Connect => screens::connect::view(self),
            Page::Subscription => screens::subscription::view(self),
            Page::Rules => screens::rules::view(self),
            Page::Settings => screens::settings::view(self),
            Page::Import => screens::import::view(self),
            Page::Logs => screens::logs::view(self),
            Page::Connections => screens::connections::view(self),
        };

        // The rail's contents swap to icon-only the moment it is collapsed, but
        // its *width* glides — so the labels do not linger in a box too narrow
        // to hold them.
        let shell = row![
            screens::sidebar::view(
                palette,
                locale,
                self.page.rail_item(),
                self.sidebar_width(),
                &self.preferences,
                &self.info,
            ),
            screens::sidebar::rule(palette),
            column![
                screens::header::view(self),
                screens::header::rule(palette),
                container(column![
                    // The entrance: the screen starts 18px low and settles.
                    vspace(Length::Fixed(self.page_rise())),
                    if scrolls {
                        Element::from(
                            scrollable(
                                // The bar is drawn *inside* the scrollable's
                                // bounds, over whatever is beneath it — cards on
                                // Settings ran under it and the update button
                                // came out half-covered. Reserving the gutter on
                                // the content is what keeps them clear of it.
                                container(body).padding(iced::Padding {
                                    right: SCROLLBAR_GUTTER,
                                    ..iced::Padding::ZERO
                                }),
                            )
                            .id(PAGE_SCROLL)
                            .direction(iced::widget::scrollable::Direction::Vertical(
                                iced::widget::scrollable::Scrollbar::new()
                                    .width(SCROLLBAR_WIDTH)
                                    .scroller_width(SCROLLBAR_WIDTH)
                                    .margin(SCROLLBAR_MARGIN),
                            ))
                            .height(Length::Fill)
                            .style(move |theme, _| theme::scroller(palette, theme)),
                        )
                    } else {
                        body
                    }
                ])
                .height(Length::Fill)
                // 20 above, 24 the rest of the way round, from the composition.
                .padding(iced::Padding {
                    top: 20.0,
                    right: 24.0,
                    bottom: 24.0,
                    left: 24.0,
                }),
            ]
            .width(Length::Fill),
        ]
        .height(Length::Fill);

        // The rail's toggle floats over the seam, vertically centred, rather
        // than living in a column of its own: the tab straddles the boundary,
        // half on the rail and half over the page, and only the tab itself
        // catches clicks — the rest of the overlay lets them fall through.
        let toggle = container(
            row![
                // The circle is centred on the rail's right edge, so its left
                // half is hidden behind the rail and its right half bulges out.
                hspace(Length::Fixed(
                    self.sidebar_width() - screens::sidebar::TAB_OVERLAP,
                )),
                screens::sidebar::edge_toggle(palette, self.sidebar_collapsed),
            ]
            .align_y(iced::Alignment::Center),
        )
        .center_y(Length::Fill);

        let shell = iced::widget::stack![shell, toggle].height(Length::Fill);

        let window = container(
            column![
                screens::titlebar::view(self),
                screens::titlebar::rule(palette),
                shell,
            ]
            .height(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .style({
            let backdrop = self.backdrop.is_some();
            move |_| theme::canvas(palette, backdrop)
        });

        // The resize edges go on last, over everything: an undecorated window
        // has no non-client area for Windows to hit-test, so the app owns its
        // own borders.
        let window: Element<'_, Message> = match self.update_banner() {
            Some(version) => iced::widget::stack![
                window,
                // The bottom corner: the header's actions sit in the top one.
                container(screens::update::banner(self, version))
                    .align_right(Length::Fill)
                    .align_bottom(Length::Fill)
                    .padding(24),
            ]
            .into(),
            None => window.into(),
        };
        let framed = screens::resize::frame(window);
        match (&self.link_prompt, &self.rule_editor) {
            (Some(prompt), _) => {
                iced::widget::stack![framed, screens::link::view(self, prompt)].into()
            }
            (None, Some(editor)) => {
                iced::widget::stack![framed, screens::rule_editor::view(self, editor)].into()
            }
            (None, None) => framed,
        }
    }
}

fn drag_events(
    event: iced::Event,
    _status: iced::event::Status,
    _window: iced::window::Id,
) -> Option<Message> {
    use iced::mouse;
    match event {
        iced::Event::Mouse(mouse::Event::CursorMoved { position }) => {
            Some(Message::DragMove(position.y))
        }
        iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
            Some(Message::DragEnd)
        }
        _ => None,
    }
}

/// A window's HWND, or 0 where there is none to be had.
fn native_handle(window: &dyn iced::window::Window) -> isize {
    use iced::window::raw_window_handle::RawWindowHandle;
    match window.window_handle().map(|handle| handle.as_raw()) {
        Ok(RawWindowHandle::Win32(handle)) => handle.hwnd.get(),
        _ => 0,
    }
}

/// Tray clicks and second launches, taken once.
type Outside = Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<Message>>>;
static OUTSIDE: OnceLock<Outside> = OnceLock::new();

fn outside_events() -> impl iced::futures::Stream<Item = Message> {
    let receiver = OUTSIDE
        .get()
        .and_then(|slot| slot.lock().ok().and_then(|mut slot| slot.take()));
    iced::futures::stream::unfold(receiver, |mut receiver| async move {
        let Some(rx) = receiver.as_mut() else {
            std::future::pending::<()>().await;
            unreachable!()
        };
        let message = rx.recv().await?;
        Some((message, receiver))
    })
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

/// How many executables one icon-decoding job takes.
///
/// Small enough that the first icons land almost immediately, large enough that
/// a machine with a thousand programmes does not queue a thousand tasks.
const ICON_BATCH: usize = 48;

/// The scrollbar, and the gutter reserved for it.
///
/// A scrollable's bar is painted over its contents, not beside them, so the
/// content has to be told to keep out of the way. The gutter is the bar plus
/// both its margins.
const SCROLLBAR_WIDTH: f32 = 6.0;
const SCROLLBAR_MARGIN: f32 = 3.0;
const SCROLLBAR_GUTTER: f32 = SCROLLBAR_WIDTH + SCROLLBAR_MARGIN * 2.0;

/// Decodes every programme's icon off the UI thread.
///
/// Executables with no icon resource are simply absent from the result rather
/// than stored as `None`, so the lookup in the view stays a plain `get`.
async fn load_icons(
    executables: Vec<(String, String)>,
) -> Vec<(String, moonlight_core::app_icon::Rgba)> {
    tokio::task::spawn_blocking(move || {
        executables
            .into_iter()
            .filter_map(|(executable, path)| {
                moonlight_core::app_icon::load(&path).map(|icon| (executable, icon))
            })
            .collect()
    })
    .await
    .unwrap_or_default()
}

/// Where the flag pictures live: `flags/` beside the executable, laid out there
/// by the build script and by the installer.
fn flags_directory() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    Some(exe.parent()?.join("flags"))
}

/// Which flags are actually on disk. Read once — a missing directory is not an
/// error, it just means every node falls back to the globe.
fn available_flags() -> std::collections::HashSet<String> {
    let Some(directory) = flags_directory() else {
        return Default::default();
    };
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Default::default();
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            (path.extension()? == "png")
                .then(|| path.file_stem()?.to_str().map(str::to_lowercase))
                .flatten()
        })
        .collect()
}

async fn scan_running() -> Vec<String> {
    tokio::task::spawn_blocking(moonlight_core::app_inventory::running_executables)
        .await
        .unwrap_or_default()
}

/// TUN needs the service **running**, not merely registered: the pipe only
/// exists while it is up, and a stopped service produced a connect that failed
/// with a bare "cannot find the file" from the pipe open.
async fn check_helper() -> bool {
    tokio::task::spawn_blocking(moonlight_core::helper::is_running)
        .await
        .unwrap_or(false)
}

/// Brings the service up alongside the app.
///
/// It is registered on-demand and the signed-in user is granted start rights at
/// install, so this needs no prompt. A service that is not registered at all
/// simply reports false, which is the "press Установить службу" state.
async fn start_helper() -> bool {
    tokio::task::spawn_blocking(|| {
        if moonlight_core::helper::is_installed() {
            moonlight_core::helper::start()
        } else {
            false
        }
    })
    .await
    .unwrap_or(false)
}

/// Installing the service needs elevation, so it is a UAC prompt on the helper's
/// own `--install`, not something this process can do.
///
/// Returns the resulting installed state, and a message when the attempt itself
/// failed. Every one of these paths used to be swallowed: a missing helper, a
/// dismissed prompt and a service that refused to register all looked exactly
/// like pressing a button that does nothing.
async fn install_helper(install: bool) -> Result<bool, String> {
    let flag = if install { "--install" } else { "--uninstall" };
    let outcome = tokio::task::spawn_blocking(move || elevate(flag))
        .await
        .unwrap_or_else(|_| Err(HELPER_FAILED.to_string()));
    // The service control manager finishes registering a moment after the
    // elevated process exits, so the state is read after a beat either way.
    tokio::time::sleep(Duration::from_millis(600)).await;
    let installed = check_helper().await;
    match outcome {
        // A refused prompt with the service already in the wanted state is not
        // worth a message — the user got what they asked for another way.
        Err(message) if installed != install => Err(message),
        _ => Ok(installed),
    }
}

/// Shown when the elevated step could not even be attempted.
const HELPER_FAILED: &str = "helper-failed";
/// Shown when `moonlight-helper.exe` is not beside the app.
const HELPER_MISSING: &str = "helper-missing";

#[cfg(windows)]
fn elevate(argument: &str) -> Result<(), String> {
    // The `runas` verb is what raises the UAC prompt; a plain spawn fails with
    // ERROR_ELEVATION_REQUIRED and no dialog, which reads as nothing happening.
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let helper = exe
        .parent()
        .map(|d| d.join("moonlight-helper.exe"))
        .ok_or_else(|| HELPER_MISSING.to_string())?;

    // The commonest way this fails is the bare `Moonlight.exe` from the release
    // page, downloaded on its own: TUN needs the service, the service is a
    // second binary, and without it the button had nothing to elevate.
    if !helper.is_file() {
        return Err(HELPER_MISSING.to_string());
    }

    let status = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &format!(
                "$p = Start-Process -FilePath '{}' -ArgumentList '{argument}' \
                 -Verb RunAs -Wait -PassThru; exit $p.ExitCode",
                helper.display()
            ),
        ])
        .status()
        .map_err(|e| e.to_string())?;

    if status.success() {
        Ok(())
    } else {
        Err(HELPER_FAILED.to_string())
    }
}

#[cfg(not(windows))]
fn elevate(_argument: &str) -> Result<(), String> {
    Err(HELPER_FAILED.to_string())
}

/// Asks GitHub whether there is anything to install, with the reason kept for
/// the log when it cannot say.
async fn check_release() -> Result<moonlight_core::updater::Outcome, String> {
    moonlight_core::updater::check(RELEASES_API, VERSION)
        .await
        .map_err(|e| e.to_string())
}

/// Downloads the release's installer, reporting as it goes.
fn download_stream(
    release: moonlight_core::updater::Release,
) -> impl futures_util::Stream<Item = Message> {
    iced::stream::channel(
        16,
        move |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
            use futures_util::SinkExt;
            // Named for the version, so a stale download of another one is
            // never mistaken for this.
            let target =
                std::env::temp_dir().join(format!("moonlight-setup-{}.exe", release.version));
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(u64, Option<u64>)>();
            let mut forwarding = output.clone();
            let pump = tokio::spawn(async move {
                while let Some((received, total)) = rx.recv().await {
                    let _ = forwarding
                        .send(Message::UpdateProgress(received, total))
                        .await;
                }
            });
            let outcome = moonlight_core::updater::download_with_progress(
                &release.download_url,
                &target,
                move |received, total| {
                    let _ = tx.send((received, total));
                },
            )
            .await;
            pump.abort();
            let _ = output
                .send(Message::UpdateDownloaded(
                    outcome.map(|()| target).map_err(|e| e.to_string()),
                ))
                .await;
        },
    )
}

fn open_url(url: &str) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let _ = std::process::Command::new("cmd")
            .args(["/c", "start", "", url])
            .creation_flags(CREATE_NO_WINDOW)
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
            // `reg.exe` is a console program, and spawning it flashed a black
            // window on every launch — this runs once, at the first draw, to
            // read the system theme. CREATE_NO_WINDOW keeps it invisible.
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            let output = std::process::Command::new("reg")
                .args([
                    "query",
                    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
                    "/v",
                    "AppsUseLightTheme",
                ])
                .creation_flags(CREATE_NO_WINDOW)
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
    /// "Обновлено 36 мин назад", from the last refresh that worked.
    pub fn last_updated(&self) -> Option<String> {
        let last = self.preferences.last_refresh?;
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        Some(format!(
            "{} {} {}",
            t(S::LastUpdated, self.locale()),
            moonlight_core::format::age(now - last, self.locale()),
            t(S::Ago, self.locale())
        ))
    }
    pub fn refresh_issue(&self) -> Option<&Issue> {
        self.refresh_issue.as_ref()
    }
    pub fn refresh_note(&self) -> Option<(u64, &Result<(), Issue>)> {
        self.refresh_note
            .as_ref()
            .map(|(id, outcome)| (*id, outcome))
    }
    pub fn tray_pinned(&self) -> bool {
        self.tray_pinned
    }
    pub fn tray_search(&self) -> &str {
        &self.tray_search
    }
    /// The service's announcement, unless the user has put this one away.
    pub fn announce(&self) -> Option<&str> {
        let announce = self.info.announce.as_deref()?;
        (self.preferences.dismissed_announce.as_deref() != Some(announce)).then_some(announce)
    }
    pub fn apps(&self) -> &[AppEntry] {
        &self.apps
    }
    /// The programme's own icon, once it has been decoded. `None` while the scan
    /// is still running, or for an executable that carries no icon resource —
    /// the row falls back to a lettered tile.
    pub fn app_icon(&self, executable: &str) -> Option<&iced::widget::image::Handle> {
        self.app_icons.get(executable)
    }
    /// The flag picture for a node's region, if one shipped with the build.
    pub fn flag_image(&self, code: &str) -> Option<iced::widget::image::Handle> {
        let code = code.to_lowercase();
        if !self.flags.contains(&code) {
            return None;
        }
        let path = flags_directory()?.join(format!("{code}.png"));
        Some(iced::widget::image::Handle::from_path(path))
    }
    pub fn is_running(&self, executable: &str) -> bool {
        self.running
            .iter()
            .any(|r| r.eq_ignore_ascii_case(executable))
    }
    pub fn running(&self) -> &[String] {
        &self.running
    }
    pub fn rules_draft(&self) -> &[RoutingRule] {
        &self.rules_draft
    }
    /// The draft in the order it would have if the rule being dragged were
    /// dropped where the pointer is now.
    pub fn rules_in_order(&self) -> Vec<&RoutingRule> {
        let mut rules: Vec<&RoutingRule> = self.rules_draft.iter().collect();
        if let Some(drag) = self.rule_drag {
            if drag.from < rules.len() {
                let to = drag.to(rules.len());
                let lifted = rules.remove(drag.from);
                rules.insert(to, lifted);
            }
        }
        rules
    }
    pub fn dragging_rule(&self) -> Option<uuid::Uuid> {
        let drag = self.rule_drag?;
        self.rules_draft.get(drag.from).map(|r| r.id)
    }
    pub fn rules_tab(&self) -> RulesTab {
        self.rules_tab
    }
    pub fn rules_filter(&self) -> &str {
        &self.rules_filter
    }
    pub fn rules_dirty(&self) -> bool {
        self.rules_draft != self.preferences.routing_rules
    }
    pub fn rules_applying(&self) -> bool {
        self.rules_applying
    }
    pub fn rules_issue(&self) -> Option<&str> {
        self.rules_issue.as_deref()
    }
    pub fn routing_groups(&self) -> &[String] {
        &self.routing_groups
    }
    pub fn profile_rules(&self) -> &[String] {
        &self.profile_rules
    }
    pub fn is_importing(&self) -> bool {
        self.importing
    }
    pub fn import_done(&self) -> bool {
        self.import_done
    }
    pub fn import_field(&self) -> &str {
        &self.import_field
    }
    pub fn helper_installed(&self) -> bool {
        self.helper_installed
    }
    pub fn update_state(&self) -> &UpdateState {
        &self.update
    }
    /// The version the corner banner offers, while it has one to offer.
    pub fn update_banner(&self) -> Option<&str> {
        match &self.update {
            UpdateState::Available(release) if !self.update_banner_hidden => Some(&release.version),
            _ => None,
        }
    }
    pub fn logs(&self) -> &[LogEntry] {
        &self.logs
    }
    pub fn log_level(&self) -> u8 {
        self.log_level
    }
    /// The download's progress while one is running, or `None` when nothing is
    /// downloading. The inner `None` means the size is unknown.
    pub fn log_source(&self) -> screens::logs::LogFilter {
        self.log_source
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

    /// 0…1 through the halo's breath cycle.
    ///
    /// Keyed off wall time rather than a counter, so the phase does not jump
    /// when the tick cadence changes between one-per-second and one-per-frame.
    pub fn breath(&self) -> f32 {
        const CYCLE: f32 = 4.2;
        let elapsed = self.started.elapsed().as_secs_f32();
        (elapsed % CYCLE) / CYCLE
    }

    /// What Auto actually settled on — "Helsinki · 37 ms".
    ///
    /// `None` until a node has been picked, so the row falls back to describing
    /// what Auto is *for* rather than claiming a choice it has not made.
    pub fn auto_choice(&self) -> Option<String> {
        if !self.preferences.auto_select {
            return None;
        }
        // The lowest measured latency is the node Auto would land on, which is
        // the same rule the injected url-test group follows.
        let best = self
            .nodes
            .iter()
            .filter(|n| !n.is_group)
            .filter_map(|n| n.latency.map(|ms| (n, ms)))
            .min_by_key(|(_, ms)| *ms)?;
        Some(format!(
            "{} {} · {}",
            t(S::AutoPicked, self.locale()),
            best.0.title(),
            moonlight_core::format::latency(Some(best.1), true)
        ))
    }

    /// Connections grouped by the process that opened them.
    ///
    /// That is the question people actually bring to this screen: is *this
    /// program* going through the tunnel.
    pub fn is_process_expanded(&self, process: &str) -> bool {
        self.expanded_processes.contains(process)
    }
    pub fn connection_filter(&self) -> &str {
        &self.connection_filter
    }
    /// The node a chain name refers to, so a connection can be shown with the
    /// country it actually left through rather than the group's name.
    /// The country a chain name leaves through, in the reader's language —
    /// "Германия" rather than the node's own "Russia -> Germany".
    pub fn node_country(&self, name: &str) -> Option<String> {
        self.nodes
            .iter()
            .find(|n| n.name == name)
            .and_then(|n| n.country(self.locale()))
            .map(str::to_string)
    }
    pub fn node_region(&self, name: &str) -> Option<String> {
        self.nodes
            .iter()
            .find(|n| n.name == name)
            .and_then(|n| n.region_code())
    }
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
            Page::Rules,
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
    fn the_sidebar_carries_the_five_destinations_the_macos_client_does() {
        assert_eq!(
            Page::SIDEBAR,
            [
                Page::Connect,
                Page::Subscription,
                Page::Rules,
                Page::Connections,
                Page::Settings,
            ]
        );
        // Logs is the diagnostic and stays under Settings; Import is reached
        // from Subscription.
        for page in [Page::Logs, Page::Import] {
            assert!(!Page::SIDEBAR.contains(&page));
        }
    }

    #[test]
    fn every_rail_entry_lights_itself() {
        // Connections used to light Settings, from when it lived under it.
        for page in Page::SIDEBAR {
            assert_eq!(page.rail_item(), page);
        }
    }

    #[test]
    fn a_page_off_the_rail_still_lights_its_parent() {
        // Watching the sidebar go dark on the Import screen reads as having
        // left the app.
        assert_eq!(Page::Import.rail_item(), Page::Subscription);
        assert_eq!(Page::Logs.rail_item(), Page::Settings);
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
        let _ = app.apply(Event::State(ConnectionState::Failed(Issue::RoutesTaken)));
        assert_eq!(
            app.last_error(),
            Some(localization::t(S::IssueRoutesTaken, app.locale()))
        );
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
        // Asked and silent, which is what `n/a` means. A node left unprobed
        // reads as a dash instead, and the two must not be confused.
        assert!(app.nodes[0].probed);
        assert_eq!(
            moonlight_core::format::latency(app.nodes[0].latency, app.nodes[0].probed),
            "n/a"
        );
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
    fn an_update_goes_from_banner_to_download_and_a_damaged_one_changes_nothing() {
        use moonlight_core::updater::{Outcome, Release};
        let release = Release {
            version: "9.9.9".into(),
            notes: String::new(),
            download_url: "https://example/setup.exe".into(),
            asset_name: "Moonlight-Setup.exe".into(),
            size: 1000,
            checksums_url: Some("https://example/sums".into()),
        };
        let mut app = app();

        // The launch check says nothing when it fails, only to the log.
        let _ = app.update(Message::UpdateFound {
            quiet: true,
            result: Err("offline".into()),
        });
        assert_eq!(app.update, UpdateState::Idle);
        assert!(app.logs.iter().any(|l| l.message.contains("offline")));

        // A version to install gets the banner, until its cross.
        let _ = app.update(Message::UpdateFound {
            quiet: true,
            result: Ok(Outcome::Available(release.clone())),
        });
        assert_eq!(app.update_banner(), Some("9.9.9"));
        let _ = app.update(Message::HideUpdateBanner);
        assert_eq!(app.update_banner(), None);

        let _ = app.update(Message::StartUpdate);
        assert!(matches!(
            app.update,
            UpdateState::Downloading {
                received: 0,
                total: Some(1000),
                ..
            }
        ));
        let _ = app.update(Message::UpdateProgress(400, Some(1000)));
        assert!(matches!(
            app.update,
            UpdateState::Downloading { received: 400, .. }
        ));

        let _ = app.update(Message::UpdateDownloaded(Ok("setup.exe".into())));
        assert!(matches!(app.update, UpdateState::Verifying(_)));
        let _ = app.update(Message::UpdateVerified(Err("checksum".into())));
        assert!(matches!(app.update, UpdateState::Failed(_)));
        assert!(
            app.pending_installer.is_none(),
            "a damaged download is never run"
        );
    }

    #[test]
    fn a_bad_rule_is_refused_with_its_reason_and_the_editor_stays_open() {
        // The core refuses the whole config for one bad rule, so the tunnel
        // stops rather than the rule being skipped.
        let mut app = app();
        let _ = app.update(Message::RuleNew);
        let _ = app.update(Message::EditorKind(Kind::IpCidr));
        let _ = app.update(Message::EditorValue("999.1.1.1/24".into()));
        let _ = app.update(Message::EditorSave);
        assert!(app.rules_draft.is_empty());
        let editor = app.rule_editor.as_ref().expect("still open");
        assert_eq!(editor.error, Some(rules::Invalid::BadCidr));
        // The value is kept, so it can be corrected rather than retyped.
        assert_eq!(editor.value, "999.1.1.1/24");
    }

    #[test]
    fn a_rule_is_a_draft_until_applied() {
        let mut app = app();
        let _ = app.update(Message::RuleNew);
        let _ = app.update(Message::EditorValue("openai.com".into()));
        let _ = app.update(Message::EditorSave);
        assert_eq!(app.rules_draft.len(), 1);
        assert!(app.rule_editor.is_none());
        assert!(app.rules_dirty(), "added, not applied");
        let _ = app.update(Message::RulesReset);
        assert!(app.rules_draft.is_empty() && !app.rules_dirty());
    }

    #[test]
    fn a_dragged_rule_lands_where_it_was_dropped() {
        let mut app = app();
        for value in ["a.com", "b.com", "c.com"] {
            app.rules_draft.push(RoutingRule::new(
                Kind::Domain,
                value,
                rules::DIRECT,
                Priority::Override,
            ));
        }
        let _ = app.update(Message::DragStart(0));
        let _ = app.update(Message::DragMove(100.0));
        // Two rows down.
        let _ = app.update(Message::DragMove(100.0 + 2.0 * screens::rules::ROW_HEIGHT));
        let preview: Vec<&str> = app
            .rules_in_order()
            .iter()
            .map(|r| r.value.as_str())
            .collect();
        assert_eq!(
            preview,
            ["b.com", "c.com", "a.com"],
            "the list shows where it will land"
        );
        let _ = app.update(Message::DragEnd);
        let order: Vec<&str> = app.rules_draft.iter().map(|r| r.value.as_str()).collect();
        assert_eq!(order, ["b.com", "c.com", "a.com"]);
        assert!(app.rule_drag.is_none());
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
        assert_eq!(
            Cadence::for_state(&ConnectionState::Disconnected, Page::Connections),
            Cadence::Second
        );
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
        assert!(app
            .title_text()
            .contains(t(S::StateDisconnected, AppLocale::Ru)));
        app.state = ConnectionState::Connected;
        assert!(app
            .title_text()
            .contains(t(S::StateConnected, AppLocale::Ru)));
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
                app.rules_draft = vec![RoutingRule::new(
                    Kind::DomainSuffix,
                    "openai.com",
                    "Gone group",
                    Priority::Extend,
                )];
                app.profile_rules =
                    vec!["AND,((DOMAIN,x),(NETWORK,udp)),P".into(), "MATCH,P".into()];
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
                app.update = UpdateState::Downloading {
                    release: moonlight_core::updater::Release {
                        version: "0.12.0".into(),
                        notes: String::new(),
                        download_url: String::new(),
                        asset_name: "Moonlight-Setup.exe".into(),
                        size: 30_000_000,
                        checksums_url: None,
                    },
                    received: 12_000_000,
                    total: Some(30_000_000),
                };
            }

            for page in [
                Page::Connect,
                Page::Subscription,
                Page::Rules,
                Page::Settings,
                Page::Import,
                Page::Logs,
                Page::Connections,
            ] {
                app.page = page;
                // Dropped immediately; building it is the assertion.
                let _ = app.main_view();
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
            Page::Rules,
            Page::Settings,
            Page::Import,
            Page::Logs,
            Page::Connections,
        ] {
            app.page = page;
            let _ = app.main_view();
        }
        // And the tray panel, with a search that matches nothing.
        app.tray_search = "zzz".into();
        let _ = screens::tray::view(&app);
        // And every state of a link's question.
        for prompt in [
            LinkPrompt::Ask("https://example.com/sub/x".into()),
            LinkPrompt::Adding("https://example.com/sub/x".into()),
            LinkPrompt::Failed("https://example.com/sub/x".into(), Issue::LinkRejected),
            LinkPrompt::Invalid,
        ] {
            app.link_prompt = Some(prompt);
            let _ = app.main_view();
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
