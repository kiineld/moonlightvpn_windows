//! The connect screen: the dial on the left, the server list on the right.

use iced::widget::{button, canvas, column, container, row, text};
use iced::{Alignment, Border, Element, Length};

use moonlight_core::{format, AppLocale, ConnectionState, Node};
use moonlight_design::motion::radii;
use moonlight_design::typography::{scale, EMPHATIC};
use moonlight_design::{icon, Icon};

use crate::components;
use crate::dial::Dial;
use crate::localization::{t, S};
use crate::{hspace, theme, vspace, Message, Moonlight};

/// The dial's drawn size, from the design.
const DIAL: f32 = 300.0;

pub fn view(app: &Moonlight) -> Element<'_, Message> {
    row![
        container(dial_column(app))
            .padding(28)
            .width(Length::FillPortion(3))
            .style({
                let palette = app.palette_of();
                move |_| theme::panel(palette)
            }),
        container(server_column(app))
            .padding(22)
            .width(Length::FillPortion(2))
            .style({
                let palette = app.palette_of();
                move |_| theme::panel(palette)
            }),
    ]
    .spacing(20)
    .into()
}

fn dial_column(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let state = app.state();

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

    let face = column![
        row![
            // The status dot is the accent-line role — a thin mark, not a fill.
            container(vspace(Length::Fixed(6.0)))
                .width(Length::Fixed(6.0))
                .style(move |_| container::Style {
                    background: Some(iced::Background::Color(ink)),
                    border: Border {
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
        text(format::duration(app.uptime()))
            .font(moonlight_design::mono())
            .size(scale::BODY)
            .color(palette.text_muted),
    ]
    .spacing(6)
    .align_x(Alignment::Center);

    let ring = canvas(Dial::new(state.clone(), palette, app.progress()))
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

    let can_press = !state.is_busy() && app.preferences().subscription_url.is_some();
    let press = button(disc)
        .on_press_maybe(can_press.then_some(Message::ToggleConnection))
        .padding(0)
        .style(move |_, status| {
            let mut style = theme::row_button(palette, false, status);
            style.border.radius = iced::border::Radius::from(DIAL / 2.0);
            style
        });

    let hint: Element<'_, Message> = match app.last_error() {
        // A failure replaces the hint rather than sitting beside it: the hint
        // says "press to connect", which is exactly what has just not worked.
        Some(error) => text(error.to_string())
            .size(scale::META)
            .color(palette.danger)
            .into(),
        None if app.preferences().subscription_url.is_none() => text(t(S::NoSubscription, locale))
            .size(scale::META)
            .color(palette.text_muted)
            .into(),
        None => text(t(S::PressToConnect, locale))
            .size(scale::META)
            .color(palette.text_muted)
            .into(),
    };

    column![
        vspace(Length::Fixed(18.0)),
        press,
        vspace(Length::Fixed(14.0)),
        hint,
        vspace(Length::Fixed(22.0)),
        stats(app),
    ]
    .align_x(Alignment::Center)
    .width(Length::Fill)
    .into()
}

/// The three figures under the dial are what is **left** — traffic and time —
/// rather than what the session has spent. Session byte counters are the least
/// actionable numbers on the screen; how much plan remains is what people open
/// the app to check.
fn stats(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let info = app.info();
    let (up, down) = app.session();

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

    // Session bytes only while there is a session; a "0 B" pair on a
    // disconnected screen is two numbers that mean nothing.
    let connected = app.state().is_connected();
    let downloaded = if connected {
        format::bytes(Some(down), locale)
    } else {
        format::bytes(None, locale)
    };
    let uploaded = if connected {
        format::bytes(Some(up), locale)
    } else {
        format::bytes(None, locale)
    };

    container(
        row![
            cell(S::Downloaded, downloaded),
            divider(app),
            cell(S::Uploaded, uploaded),
            divider(app),
            cell(S::Remaining, format::time_left(info.expire, locale)),
        ]
        .spacing(22)
        .align_y(Alignment::Center),
    )
    .padding([16, 26])
    .style(move |_| theme::card(palette))
    .into()
}

fn divider(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    container(hspace(Length::Fixed(1.0)))
        .height(Length::Fixed(34.0))
        .style(move |_| container::Style {
            background: Some(iced::Background::Color(palette.hairline)),
            ..Default::default()
        })
        .into()
}

fn server_column(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let nodes = app.nodes();

    let heading = row![
        components::overline(t(S::Servers, locale), palette),
        hspace(Length::Fill),
        text(format!("{} {}", nodes.len(), t(S::Nodes, locale)))
            .size(scale::META)
            .color(palette.text_muted),
    ]
    .align_y(Alignment::Center);

    let mut list = column![
        heading,
        vspace(Length::Fixed(14.0)),
        auto_row(app),
        vspace(Length::Fixed(8.0)),
        components::divider(palette),
        vspace(Length::Fixed(8.0)),
    ];

    if nodes.is_empty() {
        let message = if app.preferences().subscription_url.is_none() {
            match locale {
                AppLocale::Ru => "Импортируйте подписку, чтобы увидеть узлы",
                AppLocale::En => "Import a subscription to see nodes",
            }
        } else {
            match locale {
                AppLocale::Ru => "Узлы загружаются…",
                AppLocale::En => "Loading nodes…",
            }
        };
        list = list.push(components::empty_state(message, palette));
    } else {
        for node in nodes {
            // A panel that already offers a url-test picker makes the app's own
            // Авто row redundant, so only one of the two is shown.
            if node.is_auto_picker() {
                continue;
            }
            let selected = app.preferences().selected_node.as_deref() == Some(node.name.as_str());
            list = list.push(node_row(app, node, selected));
        }
    }

    list.width(Length::Fill).into()
}

/// "Авто" is the app's own latency picker.
fn auto_row(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let selected = app.preferences().auto_select;

    // The tile's wash and the row's selection wash are the same 13% lime, and
    // stacking them turns the square olive. On a selected row the tile sits on
    // a plain surface instead, so the bolt stays the only accent in the group.
    let tile_fill = if selected {
        palette.surface2
    } else {
        palette.accent_quiet
    };

    let content = row![
        container(icon(Icon::Zap, 18.0, palette.accent_ink))
            .padding(9)
            .style(move |_| container::Style {
                background: Some(iced::Background::Color(tile_fill)),
                border: Border {
                    radius: iced::border::Radius::from(radii::TILE),
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

fn node_row<'a>(app: &'a Moonlight, node: &'a Node, selected: bool) -> Element<'a, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();

    let flag: Element<'a, Message> = match node.flag() {
        Some(flag) => text(flag).size(scale::LEAD).into(),
        // A cross-country balancer has no flag, and inventing one would be a lie
        // about where the traffic goes.
        None => icon(Icon::Globe, 18.0, palette.text_muted),
    };

    // A node still being measured shows a spinner rather than its old number,
    // so a stale figure is never mistaken for a fresh one.
    let latency: Element<'a, Message> = if app.is_probing(&node.name) {
        icon(Icon::LoaderCircle, 14.0, palette.text_muted)
    } else {
        match node.latency {
            Some(ms) => text(format::latency(Some(ms)))
                .size(scale::META)
                .color(palette.ping_color(ms))
                .into(),
            None => text(format::latency(None))
                .size(scale::META)
                .color(palette.text_muted)
                .into(),
        }
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
