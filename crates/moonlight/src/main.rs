//! Moonlight VPN for Windows.
//!
//! `windows_subsystem = "windows"` so launching the app does not also open a
//! console behind it. It is left on for debug builds, where a `println!` in a
//! panic handler is worth more than a clean taskbar.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod dial;
mod localization;
mod screens;
mod theme;

use std::time::{Duration, Instant};

use iced::widget::{column, container, row, scrollable, text, Space};
use iced::{Element, Length, Subscription, Task};

use moonlight_core::preferences::Preferences;
use moonlight_core::{AppLocale, ConnectionState};
use moonlight_design::{Appearance, Palette};

use localization::{t, S};

pub const APP_NAME: &str = "Moonlight";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> iced::Result {
    // Every callback below is a `fn` item, and the whole builder is one
    // unbroken expression. Both are load-bearing:
    //
    // - A closure with an un-annotated reference parameter — `.theme(|_| …)` —
    //   is inferred at a single lifetime rather than as `for<'a>`, and iced's
    //   `view` bound is higher-ranked. The mismatch surfaces as `implementation
    //   of FnOnce is not general enough` pointing at the whole chain, which
    //   names neither the closure nor the reason. A `fn` item is higher-ranked
    //   and resolves it.
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
        .window_size((1180.0, 780.0))
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
    Logs,
    Connections,
}

impl Page {
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
            Page::Logs => S::NavLogs,
            Page::Connections => S::NavConnections,
        }
    }

    pub fn subtitle(self) -> S {
        match self {
            Page::Connect => S::ConnectSubtitle,
            Page::Subscription => S::OfTraffic,
            Page::Apps => S::AppsSubtitle,
            Page::Settings => S::SettingsSubtitle,
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
            Page::Logs => Icon::Activity,
            Page::Connections => Icon::Globe,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    Navigate(Page),
    ToggleSidebar,
    ToggleConnection,
    SelectNode(String),
    /// One animation frame; drives the dial's sweep and the entrance stagger.
    Tick(Instant),
}

pub struct Moonlight {
    page: Page,
    preferences: Preferences,
    state: ConnectionState,
    /// When the current transition started, so the dial can be sampled against
    /// it. `None` when nothing is animating.
    transition_started: Option<Instant>,
    uptime_seconds: i64,
    sidebar_collapsed: bool,
    /// Off in tests, so preference changes stay in memory.
    persist: bool,
}

impl Moonlight {
    fn new() -> (Self, Task<Message>) {
        (
            Moonlight::with_preferences(Preferences::load()),
            Task::none(),
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
            uptime_seconds: 0,
            sidebar_collapsed,
            persist: true,
        }
    }

    fn title(&self) -> String {
        // The macOS client puts the state in the window title so a user with the
        // window behind something else can still read it from the taskbar.
        let state = match &self.state {
            ConnectionState::Connected => t(S::StateConnected, self.locale()),
            ConnectionState::Connecting => t(S::Connecting, self.locale()),
            ConnectionState::Disconnecting => t(S::Disconnecting, self.locale()),
            ConnectionState::Failed(_) => t(S::StateFailed, self.locale()),
            ConnectionState::Disconnected => t(S::StateDisconnected, self.locale()),
        };
        format!("{APP_NAME} · {state}")
    }

    fn save(&self) {
        if self.persist {
            let _ = self.preferences.save();
        }
    }

    fn locale(&self) -> AppLocale {
        self.preferences.locale
    }

    /// iced's own theme, which this app uses only for the text caret and
    /// selection colours — every other colour comes from [`Palette`].
    fn iced_theme(&self) -> iced::Theme {
        iced::Theme::Dark
    }

    fn palette(&self) -> Palette {
        let appearance = match self.preferences.appearance.as_deref() {
            Some("dark") => Appearance::Dark,
            Some("light") => Appearance::Light,
            _ => Appearance::System,
        };
        // Resolved against the OS on every draw, so a user switching Windows to
        // light mode mid-session is followed rather than needing a restart.
        appearance.palette(system_prefers_dark())
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
            Message::Navigate(page) => {
                self.page = page;
                Task::none()
            }
            Message::ToggleSidebar => {
                self.sidebar_collapsed = !self.sidebar_collapsed;
                self.preferences.sidebar_collapsed = self.sidebar_collapsed;
                self.save();
                Task::none()
            }
            Message::ToggleConnection => {
                // The controller that actually drives the core lands here; this
                // is the state machine it will report into.
                let next = match self.state {
                    ConnectionState::Disconnected | ConnectionState::Failed(_) => {
                        ConnectionState::Connecting
                    }
                    ConnectionState::Connected => ConnectionState::Disconnecting,
                    // Already in flight. Restamping the clock here would restart
                    // the sweep from zero on a double-click, so a press during a
                    // transition is ignored rather than absorbed into it.
                    ref busy => busy.clone(),
                };
                if next != self.state {
                    self.state = next;
                    self.transition_started = Some(Instant::now());
                }
                Task::none()
            }
            Message::SelectNode(name) => {
                self.preferences.selected_node = Some(name);
                self.preferences.auto_select = false;
                self.save();
                Task::none()
            }
            Message::Tick(_) => {
                if self.transition_progress() >= 1.0 {
                    // A transition that has run its course settles into the
                    // state it was heading for.
                    self.state = match self.state {
                        ConnectionState::Connecting => ConnectionState::Connected,
                        ConnectionState::Disconnecting => ConnectionState::Disconnected,
                        ref settled => settled.clone(),
                    };
                    if self.transition_started.take().is_some() {
                        self.uptime_seconds = 0;
                    }
                } else if self.state.is_connected() {
                    self.uptime_seconds += 1;
                }
                Task::none()
            }
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        match Cadence::for_state(&self.state) {
            Cadence::Idle => Subscription::none(),
            Cadence::Frame => iced::time::every(Duration::from_millis(16)).map(Message::Tick),
            Cadence::Second => iced::time::every(Duration::from_secs(1)).map(Message::Tick),
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let palette = self.palette();
        let locale = self.locale();

        let body = match self.page {
            Page::Connect => screens::connect::view(
                palette,
                locale,
                &self.state,
                self.transition_progress(),
                self.uptime_seconds,
                &self.preferences,
            ),
            other => screens::placeholder::view(palette, locale, other),
        };

        let content = row![
            screens::sidebar::view(
                palette,
                locale,
                self.page,
                self.sidebar_collapsed,
                &self.preferences
            ),
            container(
                column![
                    screens::header::view(palette, locale, self.page),
                    scrollable(body)
                        .height(Length::Fill)
                        .style(move |theme, _| theme::scroller(palette, theme)),
                ]
                .spacing(20)
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

/// How often the window needs to be woken.
///
/// Split out from [`Moonlight::subscription`] because it is the part worth
/// asserting on: a `Subscription` is opaque, so a test can only check the
/// decision that produced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cadence {
    /// Nothing is moving. A window sitting disconnected has no reason to wake
    /// the GPU sixty times a second.
    Idle,
    /// A transition is animating.
    Frame,
    /// Connected: the uptime timer ticks, nothing else.
    Second,
}

impl Cadence {
    pub fn for_state(state: &ConnectionState) -> Cadence {
        if state.is_busy() {
            Cadence::Frame
        } else if state.is_connected() {
            Cadence::Second
        } else {
            Cadence::Idle
        }
    }
}

/// Whether Windows is in dark mode.
///
/// `AppsUseLightTheme` under `HKCU` is the value the Settings app writes; 0
/// means the apps theme is dark. On a non-Windows developer build the answer is
/// dark, which is the palette the design is drawn in.
fn system_prefers_dark() -> bool {
    #[cfg(windows)]
    {
        use std::process::Command;
        // Read through the shell rather than a fifth registry helper: this is
        // consulted once per draw of a window that redraws on interaction, and
        // a wrong answer costs a theme rather than a tunnel.
        static CACHE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *CACHE.get_or_init(|| {
            let output = Command::new("reg")
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

/// Body text in the palette's secondary ink.
pub fn muted(
    content: impl text::IntoFragment<'static>,
    palette: Palette,
) -> Element<'static, Message> {
    text(content)
        .size(moonlight_design::typography::scale::BODY_SM)
        .color(palette.text2)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh app on default preferences, with persistence off.
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
            Page::Logs,
            Page::Connections,
        ] {
            assert!(!t(page.title(), AppLocale::Ru).is_empty());
            assert!(!t(page.subtitle(), AppLocale::Ru).is_empty());
            // Icons are an enum, so this only checks it draws something.
            assert!(!page.icon().paths().is_empty());
        }
    }

    #[test]
    fn the_sidebar_carries_exactly_the_four_primary_pages() {
        // Logs and Connections are reached from Settings, not from the rail —
        // they are diagnostics, not destinations.
        assert_eq!(Page::SIDEBAR.len(), 4);
        assert!(!Page::SIDEBAR.contains(&Page::Logs));
        assert!(!Page::SIDEBAR.contains(&Page::Connections));
    }

    #[test]
    fn pressing_connect_moves_through_connecting_rather_than_straight_to_connected() {
        let mut app = app();
        assert_eq!(app.state, ConnectionState::Disconnected);

        let _ = app.update(Message::ToggleConnection);
        assert_eq!(app.state, ConnectionState::Connecting);
        assert!(app.transition_started.is_some());
    }

    #[test]
    fn pressing_connect_while_already_connecting_does_nothing() {
        // Otherwise a double-click starts a second connect over the first.
        let mut app = app();
        let _ = app.update(Message::ToggleConnection);
        let started = app.transition_started;

        let _ = app.update(Message::ToggleConnection);
        assert_eq!(app.state, ConnectionState::Connecting);
        assert_eq!(app.transition_started, started);
    }

    #[test]
    fn a_finished_transition_settles_into_the_state_it_was_heading_for() {
        let mut app = app();
        app.state = ConnectionState::Connecting;
        // A transition that started long enough ago has run its course.
        app.transition_started = Some(Instant::now() - Duration::from_secs(5));

        let _ = app.update(Message::Tick(Instant::now()));
        assert_eq!(app.state, ConnectionState::Connected);
        assert!(app.transition_started.is_none());
    }

    #[test]
    fn picking_a_node_turns_auto_select_off() {
        // Otherwise the next refresh silently moves the user off the node they
        // just chose.
        let mut app = app();
        assert!(app.preferences.auto_select);

        let _ = app.update(Message::SelectNode("Node A".into()));
        assert!(!app.preferences.auto_select);
        assert_eq!(app.preferences.selected_node.as_deref(), Some("Node A"));
    }

    #[test]
    fn an_idle_window_is_never_woken() {
        // A disconnected window has no reason to wake the GPU at all.
        assert_eq!(
            Cadence::for_state(&ConnectionState::Disconnected),
            Cadence::Idle
        );
        assert_eq!(
            Cadence::for_state(&ConnectionState::Failed("x".into())),
            Cadence::Idle
        );
    }

    #[test]
    fn only_a_transition_asks_for_frames() {
        assert_eq!(
            Cadence::for_state(&ConnectionState::Connecting),
            Cadence::Frame
        );
        assert_eq!(
            Cadence::for_state(&ConnectionState::Disconnecting),
            Cadence::Frame
        );
        // A connected tunnel only has to tick a clock.
        assert_eq!(
            Cadence::for_state(&ConnectionState::Connected),
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
}
