//! The connect screen: the dial on the left, the server list on the right.

use iced::widget::{button, canvas, column, container, row, text};
use iced::{Alignment, Element, Length};

use moonlight_core::preferences::Preferences;
use moonlight_core::{format, AppLocale, ConnectionState, Node};
use moonlight_design::typography::{scale, EMPHATIC};
use moonlight_design::{icon, Icon, Palette};

use crate::dial::Dial;
use crate::localization::{t, S};
use crate::{hspace, theme, vspace, Message};

/// The dial's drawn size, from the design.
const DIAL: f32 = 300.0;

pub fn view<'a>(
    palette: Palette,
    locale: AppLocale,
    state: &ConnectionState,
    progress: f32,
    uptime_seconds: i64,
    preferences: &'a Preferences,
) -> Element<'a, Message> {
    row![
        container(dial_column(
            palette,
            locale,
            state,
            progress,
            uptime_seconds
        ))
        .padding(28)
        .width(Length::FillPortion(3))
        .style(move |_| theme::panel(palette)),
        container(server_column(palette, locale, preferences))
            .padding(22)
            .width(Length::FillPortion(2))
            .style(move |_| theme::panel(palette)),
    ]
    .spacing(20)
    .height(Length::Fixed(660.0))
    .into()
}

fn dial_column<'a>(
    palette: Palette,
    locale: AppLocale,
    state: &ConnectionState,
    progress: f32,
    uptime_seconds: i64,
) -> Element<'a, Message> {
    let (label, ink) = match state {
        ConnectionState::Connected => (S::StateConnected, palette.status_secure),
        ConnectionState::Connecting => (S::Connecting, palette.text2),
        ConnectionState::Disconnecting => (S::Disconnecting, palette.text2),
        ConnectionState::Failed(_) => (S::StateFailed, palette.danger),
        ConnectionState::Disconnected => (S::StateDisconnected, palette.text_muted),
    };

    let action = match state {
        ConnectionState::Connected => S::Disconnect,
        ConnectionState::Connecting => S::Connecting,
        ConnectionState::Disconnecting => S::Disconnecting,
        _ => S::Connect,
    };

    // The ring is drawn behind the label stack rather than around it, so the
    // whole disc is one press target.
    let face = column![
        row![
            // The status dot is the accent-line role — a thin mark, not a fill.
            container(hspace(Length::Fixed(6.0)))
                .height(Length::Fixed(6.0))
                .style(move |_| container::Style {
                    background: Some(iced::Background::Color(ink)),
                    border: iced::Border {
                        radius: iced::border::Radius::from(3.0),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
            text(t(label, locale))
                .size(scale::MICRO)
                .font(moonlight_design::ui(EMPHATIC))
                .color(ink),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        text(t(action, locale))
            .font(moonlight_design::display())
            .size(scale::HERO)
            .color(palette.text),
        text(format::duration(uptime_seconds))
            .font(moonlight_design::mono())
            .size(scale::BODY)
            .color(palette.text_muted),
    ]
    .spacing(6)
    .align_x(Alignment::Center);

    let ring = canvas(Dial::new(state.clone(), palette, progress))
        .width(Length::Fixed(DIAL))
        .height(Length::Fixed(DIAL));

    let disc = iced::widget::stack![
        container(ring)
            .center_x(Length::Fixed(DIAL))
            .center_y(Length::Fixed(DIAL)),
        container(face)
            .center_x(Length::Fixed(DIAL))
            .center_y(Length::Fixed(DIAL)),
    ];

    let press = button(disc)
        .on_press_maybe((!state.is_busy()).then_some(Message::ToggleConnection))
        .padding(0)
        .style(move |_, status| {
            let mut style = theme::row_button(palette, false, status);
            style.border.radius = iced::border::Radius::from(DIAL / 2.0);
            style
        });

    column![
        vspace(Length::Fixed(24.0)),
        press,
        vspace(Length::Fixed(14.0)),
        text(t(S::PressToConnect, locale))
            .size(scale::META)
            .color(palette.text_muted),
        vspace(Length::Fixed(22.0)),
        stats(palette, locale),
    ]
    .align_x(Alignment::Center)
    .width(Length::Fill)
    .into()
}

/// The three figures under the dial are what is **left** — traffic and time —
/// rather than what the session has spent. Session byte counters are the least
/// actionable numbers on the screen; how much plan remains is what people open
/// the app to check.
fn stats<'a>(palette: Palette, locale: AppLocale) -> Element<'a, Message> {
    let cell = |label: S, value: String| {
        column![
            text(t(label, locale))
                .size(scale::MICRO)
                .color(palette.text_muted),
            text(value)
                .font(moonlight_design::display())
                .size(scale::LEAD)
                .color(palette.text),
        ]
        .spacing(4)
        .align_x(Alignment::Center)
    };

    container(
        row![
            cell(S::Downloaded, format::bytes(None, locale)),
            divider(palette),
            cell(S::Uploaded, format::bytes(None, locale)),
            divider(palette),
            cell(S::Remaining, format::days(None, locale)),
        ]
        .spacing(22)
        .align_y(Alignment::Center),
    )
    .padding([16, 26])
    .style(move |_| theme::card(palette))
    .into()
}

fn divider<'a>(palette: Palette) -> Element<'a, Message> {
    container(hspace(Length::Fixed(1.0)))
        .height(Length::Fixed(34.0))
        .style(move |_| container::Style {
            background: Some(iced::Background::Color(palette.hairline)),
            ..Default::default()
        })
        .into()
}

fn server_column<'a>(
    palette: Palette,
    locale: AppLocale,
    preferences: &'a Preferences,
) -> Element<'a, Message> {
    let heading = row![
        text(t(S::Servers, locale))
            .size(scale::MICRO)
            .font(moonlight_design::ui(EMPHATIC))
            .color(palette.text_muted),
        hspace(Length::Fill),
        text(format!("0 {}", t(S::Nodes, locale)))
            .size(scale::META)
            .color(palette.text_muted),
    ]
    .align_y(Alignment::Center);

    column![
        heading,
        vspace(Length::Fixed(14.0)),
        auto_row(palette, locale, preferences.auto_select),
        vspace(Length::Fixed(8.0)),
        container(vspace(Length::Fixed(1.0)))
            .width(Length::Fill)
            .style(move |_| container::Style {
                background: Some(iced::Background::Color(palette.hairline)),
                ..Default::default()
            }),
        vspace(Length::Fixed(8.0)),
        text(match locale {
            AppLocale::Ru => "Импортируйте подписку, чтобы увидеть узлы",
            AppLocale::En => "Import a subscription to see nodes",
        })
        .size(scale::BODY_SM)
        .color(palette.text_muted),
    ]
    .width(Length::Fill)
    .into()
}

/// "Авто" is the app's own latency picker. When a panel already offers a
/// `url-test` group there is no reason to show both — see
/// [`moonlight_core::Node::is_auto_picker`].
fn auto_row<'a>(palette: Palette, locale: AppLocale, selected: bool) -> Element<'a, Message> {
    // The tile's wash and the row's selection wash are the same 13% lime, and
    // stacking them turns the square olive. On a selected row the tile sits on
    // a plain surface instead, so the bolt stays the only accent in the group.
    let tile = if selected {
        palette.surface2
    } else {
        palette.accent_quiet
    };

    let content = row![
        container(icon(Icon::Zap, 18.0, palette.accent_ink))
            .padding(9)
            .style(move |_| container::Style {
                background: Some(iced::Background::Color(tile)),
                border: iced::Border {
                    radius: iced::border::Radius::from(moonlight_design::motion::radii::TILE),
                    ..Default::default()
                },
                ..Default::default()
            }),
        column![
            text(t(S::Auto, locale))
                .size(scale::BODY)
                .font(moonlight_design::ui(EMPHATIC))
                .color(palette.text),
            text(t(S::AutoSubtitle, locale))
                .size(scale::META)
                .color(palette.text_muted),
        ]
        .spacing(1),
    ]
    .spacing(12)
    .align_y(Alignment::Center);

    button(content)
        .on_press(Message::SelectNode(String::new()))
        .padding([10, 12])
        .width(Length::Fill)
        .style(move |_, status| theme::row_button(palette, selected, status))
        .into()
}

/// A node row. Kept here rather than inlined so the server list has one shape
/// whether it is drawn from the API or from a cached subscription.
// Unused until the tunnel controller supplies nodes; written now because the
// row's shape is what the server list is built around.
#[allow(dead_code)]
pub fn node_row<'a>(
    palette: Palette,
    locale: AppLocale,
    node: &'a Node,
    selected: bool,
) -> Element<'a, Message> {
    let flag: Element<'a, Message> = match node.flag() {
        Some(flag) => text(flag).size(scale::LEAD).into(),
        // A cross-country balancer or an auto-picker has no flag, and inventing
        // one would be a lie about where the traffic goes.
        None => icon(Icon::Globe, 18.0, palette.text_muted),
    };

    let latency = match node.latency {
        Some(ms) => text(format::latency(Some(ms)))
            .size(scale::META)
            .color(palette.ping_color(ms)),
        None => text(format::latency(None))
            .size(scale::META)
            .color(palette.text_muted),
    };

    let content = row![
        container(flag).width(Length::Fixed(34.0)),
        column![
            text(node.title())
                .size(scale::BODY)
                .font(moonlight_design::ui(EMPHATIC))
                .color(palette.text),
            text(node.subtitle(locale))
                .size(scale::META)
                .color(palette.text_muted),
        ]
        .spacing(1),
        hspace(Length::Fill),
        latency,
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    button(content)
        .on_press(Message::SelectNode(node.name.clone()))
        .padding([10, 12])
        .width(Length::Fill)
        .style(move |_, status| theme::row_button(palette, selected, status))
        .into()
}
