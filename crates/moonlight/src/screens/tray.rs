//! The tray panel: the app's state and its servers, one click from the
//! notification area, without the window.
//!
//! The macOS menu-bar popover, laid out the same way: the service's message,
//! the state and live speeds, rules / global / direct, a searchable server list
//! with each server's transport and the service's note on it, a floating
//! connect button, and the way into the window.

use iced::widget::{button, canvas, column, container, row, scrollable, stack, text, text_input};
use iced::{Alignment, Background, Border, Element, Length};

use moonlight_core::{format, ConnectionState, Node, RoutingMode};
use moonlight_design::motion::radii;
use moonlight_design::typography::{scale, EMPHATIC, ROW_TITLE};
use moonlight_design::{icon, Icon};

use crate::localization::{t, S};
use crate::logo::Logo;
use crate::screens::connect;
use crate::{components, hspace, theme, vspace, Message, Moonlight, Page};

pub const WIDTH: f32 = 380.0;
pub const HEIGHT: f32 = 620.0;

pub fn view(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();

    let mut top = column![header(app), status(app), routing(app), search(app)].spacing(12);
    if let Some(message) = app.announce() {
        top = top.push(components::announce_banner(
            message,
            Message::DismissAnnounce,
            palette,
        ));
    }

    // The connect button floats over the foot of the list, as on macOS, so
    // the list keeps its whole height to scroll through.
    let list = stack![
        scrollable(column![servers(app), vspace(Length::Fixed(64.0))].padding([0, 4]))
            .height(Length::Fill)
            .style(move |theme, _| theme::scroller(palette, theme)),
        container(connect_button(app))
            .center_x(Length::Fill)
            .align_bottom(Length::Fill)
            .padding(12),
    ]
    .height(Length::Fill);

    container(
        column![
            container(top).padding(iced::Padding {
                top: 16.0,
                right: 16.0,
                bottom: 8.0,
                left: 16.0,
            }),
            list,
            components::divider(palette),
            container(footer(app)).padding([10, 14]),
        ]
        .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style({
        let backdrop = app.tray_backdrop();
        move |_| theme::canvas(palette, backdrop)
    })
    .into()
}

fn header(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let pinned = app.tray_pinned();
    let pin = button(icon(
        Icon::Pin,
        16.0,
        if pinned {
            palette.text
        } else {
            palette.text_muted
        },
    ))
    .on_press(Message::TogglePin)
    .padding(8)
    .style(move |_, status| theme::row_button(palette, pinned, status));

    row![
        canvas(Logo::with_radius(palette, 9.0))
            .width(Length::Fixed(30.0))
            .height(Length::Fixed(30.0)),
        text("moonlight")
            .font(moonlight_design::display())
            .size(19.0)
            .color(palette.text),
        hspace(Length::Fill),
        pin,
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .into()
}

fn status(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let (label, dot) = match app.state() {
        ConnectionState::Connected => (S::TrayStateOn, palette.st_up),
        ConnectionState::Connecting => (S::Connecting, palette.text_muted),
        ConnectionState::Disconnecting => (S::Disconnecting, palette.text_muted),
        ConnectionState::Failed(_) => (S::TrayStateFailed, palette.danger),
        ConnectionState::Disconnected => (S::TrayStateOff, palette.text_muted),
    };
    let (up, down) = app.rates();
    let speeds = format!(
        "↓ {}   ↑ {}",
        format::rate(Some(down), locale),
        format::rate(Some(up), locale)
    );
    row![
        container(vspace(Length::Fixed(8.0)))
            .width(Length::Fixed(8.0))
            .style(move |_| container::Style {
                background: Some(Background::Color(dot)),
                border: Border {
                    radius: iced::border::Radius::from(radii::PILL),
                    ..Default::default()
                },
                ..Default::default()
            }),
        text(t(label, locale))
            .size(14.0)
            .font(moonlight_design::ui(EMPHATIC))
            .color(palette.text),
        hspace(Length::Fill),
        text(speeds)
            .font(moonlight_design::mono())
            .size(12.5)
            .color(palette.text2),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .into()
}

fn routing(app: &Moonlight) -> Element<'_, Message> {
    let locale = app.locale_of();
    let options: Vec<(RoutingMode, &str)> = RoutingMode::ALL
        .into_iter()
        .map(|mode| {
            let label = match mode {
                RoutingMode::Rule => S::TrayRules,
                RoutingMode::Global => S::TrayGlobal,
                RoutingMode::Direct => S::TrayDirect,
            };
            (mode, t(label, locale))
        })
        .collect();
    components::segmented_fill(
        &options,
        app.preferences().routing_mode,
        Message::SetRoutingMode,
        app.palette_of(),
    )
}

fn search(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let field = text_input(t(S::SearchServers, locale), app.tray_search())
        .on_input(Message::TraySearch)
        .padding([9, 14])
        .size(13.5)
        .style(move |_, status| theme::field(palette, status));
    let ping = button(
        row![
            icon(Icon::Zap, 14.0, palette.text),
            text(t(S::PingAll, locale))
                .size(13.0)
                .font(moonlight_design::ui(EMPHATIC))
                .color(palette.text),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .on_press_maybe((!app.is_pinging()).then_some(Message::Ping))
    .padding([9, 14])
    .style(move |_, status| theme::header_button(palette, status));
    row![field.width(Length::Fill), ping]
        .spacing(8)
        .align_y(Alignment::Center)
        .into()
}

fn servers(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let query = app.tray_search().trim().to_lowercase();
    let selected = app.preferences().selected_node.as_deref();

    let rows: Vec<Element<'_, Message>> = app
        .nodes()
        .iter()
        .filter(|node| {
            query.is_empty()
                || node.name.to_lowercase().contains(&query)
                || node
                    .description
                    .as_deref()
                    .is_some_and(|d| d.to_lowercase().contains(&query))
        })
        .map(|node| {
            let chosen = if node.is_auto_picker() {
                app.preferences().auto_select
            } else {
                !app.preferences().auto_select && selected == Some(node.name.as_str())
            };
            server_row(app, node, chosen)
        })
        .collect();

    if rows.is_empty() {
        return container(
            text(t(S::NothingFound, locale))
                .size(scale::BODY_SM)
                .color(palette.text_muted),
        )
        .center_x(Length::Fill)
        .padding(24)
        .into();
    }
    column(rows).spacing(2).into()
}

fn badge<'a>(label: String, app: &Moonlight) -> Element<'a, Message> {
    let palette = app.palette_of();
    container(
        text(label)
            .size(11.5)
            .font(moonlight_design::ui(EMPHATIC))
            .color(palette.text2),
    )
    .padding([2, 7])
    .style(move |_| container::Style {
        background: Some(Background::Color(palette.surface2)),
        border: Border {
            radius: iced::border::Radius::from(6.0),
            ..Default::default()
        },
        ..Default::default()
    })
    .into()
}

fn server_row<'a>(app: &'a Moonlight, node: &'a Node, chosen: bool) -> Element<'a, Message> {
    let palette = app.palette_of();

    let mut badges = row![].spacing(6).align_y(Alignment::Center);
    if let Some(label) = node
        .protocol_label
        .clone()
        .or_else(|| (!node.is_group && !node.kind.is_empty()).then(|| node.kind.to_uppercase()))
    {
        badges = badges.push(badge(label.to_uppercase(), app));
    }
    if let Some(description) = node.description.clone() {
        badges = badges.push(badge(description, app));
    }

    let latency: Element<'a, Message> = if app.is_probing(&node.name) {
        text("…").size(scale::META).color(palette.text_muted).into()
    } else {
        let color = match node.latency {
            Some(ms) => palette.ping_color(ms),
            None if node.probed => palette.danger,
            None => palette.text_muted,
        };
        text(format::latency(node.latency, node.probed))
            .font(moonlight_design::mono())
            .size(13.0)
            .color(color)
            .into()
    };

    let ping = button(icon(Icon::Zap, 14.0, palette.text_muted))
        .on_press(Message::PingNode(node.name.clone()))
        .padding(6)
        .style(move |_, status| theme::row_button(palette, false, status));

    let content = row![
        connect::flag(app, node),
        column![
            text(node.title())
                .size(14.0)
                .font(moonlight_design::ui(ROW_TITLE))
                .color(palette.text),
            badges,
        ]
        .spacing(4)
        .width(Length::Fill),
        ping,
        container(latency)
            .width(Length::Fixed(58.0))
            .align_right(Length::Fixed(58.0)),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    button(content)
        .on_press(Message::SelectNode(if node.is_auto_picker() {
            String::new()
        } else {
            node.name.clone()
        }))
        .padding([9, 10])
        .width(Length::Fill)
        .style(move |_, status| theme::row_button(palette, chosen, status))
        .into()
}

fn connect_button(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let state = app.state();
    let label = match state {
        ConnectionState::Connected => S::DisconnectVerb,
        ConnectionState::Connecting => S::Connecting,
        ConnectionState::Disconnecting => S::Disconnecting,
        _ => S::ConnectNow,
    };
    let can_press = !state.is_busy() && app.preferences().subscription_url.is_some();
    button(
        row![
            icon(Icon::Power, 16.0, palette.text_on_accent),
            text(t(label, locale))
                .size(15.0)
                .font(moonlight_design::ui(EMPHATIC))
                .color(palette.text_on_accent),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .on_press_maybe(can_press.then_some(Message::ToggleConnection))
    .padding([13, 28])
    .style(move |_, status| theme::accent_button(palette, status))
    .into()
}

fn footer(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let open = button(
        row![
            icon(Icon::Monitor, 15.0, palette.text),
            text(t(S::OpenWindow, locale))
                .size(13.5)
                .font(moonlight_design::ui(EMPHATIC))
                .color(palette.text),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .on_press(Message::OpenMain)
    .padding([9, 14])
    .style(move |_, status| theme::header_button(palette, status));

    let plan = button(
        row![
            icon(Icon::Sparkles, 14.0, palette.text2),
            text(
                app.info()
                    .title
                    .clone()
                    .unwrap_or_else(|| t(S::NavSubscription, locale).to_string())
            )
            .size(13.0)
            .color(palette.text2),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .on_press(Message::OpenPage(Page::Subscription))
    .padding([9, 14])
    .style(move |_, status| theme::header_button(palette, status));

    let quit = button(icon(Icon::LogOut, 15.0, palette.text_muted))
        .on_press(Message::Quit)
        .padding(9)
        .style(move |_, status| theme::header_button(palette, status));

    row![open, hspace(Length::Fill), plan, quit]
        .spacing(8)
        .align_y(Alignment::Center)
        .into()
}
