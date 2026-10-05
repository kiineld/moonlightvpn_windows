//! The tray panel: the app's state and its servers, one click from the
//! notification area, without the window.
//!
//! The macOS menu-bar popover, laid out the same way: the service's message,
//! the state and live speeds, rules / global / direct, a searchable server list
//! with each server's transport and the service's note on it, a floating
//! connect button, and the way into the window.

use iced::widget::{button, canvas, column, container, row, scrollable, stack, text, text_input};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};

use moonlight_core::{format, ConnectionState, Node, RoutingMode};
use moonlight_design::motion::radii;
use moonlight_design::typography::{EMPHATIC, MEDIUM, ROW_TITLE};
use moonlight_design::{icon, icon_thin, Icon, Palette};

use crate::localization::{t, S};
use crate::logo::Logo;
use crate::screens::connect;
use crate::{components, hspace, theme, vspace, Message, Moonlight, Page};

pub const WIDTH: f32 = 384.0;
pub const HEIGHT: f32 = 620.0;
/// The server list's scroller, so each opening starts at its head.
pub const LIST_SCROLL: &str = "tray-servers";

/// A row's corner, and the room a description is given before it is cut.
const ROW_RADIUS: f32 = 14.0;
const NOTE_ROOM: usize = 30;

pub fn view(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();

    let mut top = column![].spacing(0);
    if let Some(message) = app.announce() {
        top = top.push(
            container(components::announce_banner(
                message,
                app.announce_openness(),
                WIDTH - 24.0,
                Message::ToggleAnnounce,
                palette,
            ))
            .padding(Padding {
                top: 12.0,
                right: 12.0,
                bottom: 0.0,
                left: 12.0,
            }),
        );
    }
    top = top
        .push(container(header(app)).padding(Padding {
            top: 14.0,
            right: 16.0,
            bottom: 0.0,
            left: 16.0,
        }))
        .push(container(routing(app)).padding(Padding {
            top: 14.0,
            right: 12.0,
            bottom: 0.0,
            left: 12.0,
        }))
        .push(container(search(app)).padding(Padding {
            top: 10.0,
            right: 12.0,
            bottom: 0.0,
            left: 12.0,
        }));

    // The connect button floats over the foot of the list, as on macOS, so
    // the list keeps its whole height to scroll through.
    let list = stack![
        servers(app),
        container(connect_button(app))
            .center_x(Length::Fill)
            .align_bottom(Length::Fill)
            .padding(Padding {
                bottom: 11.0,
                ..Padding::ZERO
            }),
    ]
    .height(Length::Fill);

    container(column![top, list, footer(app)].height(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .style({
            // Acrylic lets far more of the desktop through than the window's
            // Mica does, so the canvas is laid heavier over it: a bright
            // wallpaper under three quarters turned the panel to mud.
            let strength = if app.tray_backdrop() { 0.88 } else { 1.0 };
            move |_| theme::canvas(palette, strength)
        })
        .into()
}

fn header(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let pinned = app.tray_pinned();
    let pin = button(
        container(icon_thin(
            Icon::Pin,
            14.0,
            if pinned {
                palette.text_on_accent
            } else {
                palette.text2
            },
            2.2,
        ))
        .center(Length::Fill),
    )
    .on_press(Message::TogglePin)
    .width(Length::Fixed(30.0))
    .height(Length::Fixed(30.0))
    .padding(0)
    .style(move |_, status| {
        // Pinned, the glass takes the accent — a state is a tint on the
        // glass, as the macOS client has it.
        let mut style = theme::icon_button(palette, status);
        if pinned {
            style.background = Some(Background::Color(palette.accent));
        }
        style
    });

    let mut lines = column![
        row![
            canvas(Logo::with_radius(palette, 8.0))
                .width(Length::Fixed(26.0))
                .height(Length::Fixed(26.0)),
            text("moonlight")
                .font(moonlight_design::display())
                .size(15.0)
                .color(palette.text),
            hspace(Length::Fill),
            pin,
        ]
        .spacing(9)
        .align_y(Alignment::Center),
        status(app),
    ]
    .spacing(10);
    if let Some(problem) = app
        .last_error()
        .filter(|_| !app.state().is_connected())
        .map(str::to_string)
    {
        lines = lines.push(components::issue_line(problem, palette));
    }
    lines.into()
}

/// The state, the uptime and the speeds — the one line that changes by the
/// second.
fn status(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let connected = app.state().is_connected();
    let label = match app.state() {
        ConnectionState::Connected => format!(
            "{} · {}",
            t(S::TrayStateOn, locale),
            format::duration(app.uptime())
        ),
        ConnectionState::Connecting => t(S::Connecting, locale).to_string(),
        ConnectionState::Disconnecting => t(S::Disconnecting, locale).to_string(),
        ConnectionState::Failed(_) => t(S::TrayStateFailed, locale).to_string(),
        ConnectionState::Disconnected => t(S::TrayStateOff, locale).to_string(),
    };
    let dot = match app.state() {
        ConnectionState::Connected => palette.st_up,
        ConnectionState::Failed(_) => palette.danger,
        _ => palette.text_muted,
    };
    let (up, down) = app.rates();
    let rate = move |glyph: Icon, value: i64, tone: Color| {
        let tone = if connected { tone } else { palette.text_muted };
        row![
            icon_thin(glyph, 12.0, tone, 2.4),
            text(format::rate(
                Some(if connected { value } else { 0 }),
                locale
            ))
            .font(moonlight_design::mono())
            .size(12.0)
            .color(tone),
        ]
        .spacing(3)
        .align_y(Alignment::Center)
    };
    row![
        container(vspace(Length::Fixed(7.0)))
            .width(Length::Fixed(7.0))
            .style(move |_| container::Style {
                background: Some(Background::Color(dot)),
                border: Border {
                    radius: iced::border::Radius::from(radii::PILL),
                    ..Default::default()
                },
                ..Default::default()
            }),
        text(label)
            .size(12.5)
            .font(moonlight_design::ui(MEDIUM))
            .color(if connected {
                palette.accent_ink
            } else {
                palette.text2
            }),
        hspace(Length::Fill),
        rate(Icon::ArrowDown, down, palette.accent_ink),
        rate(Icon::ArrowUp, up, palette.text2),
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

    // The field is the capsule; the input inside it is bare, so the glyph and
    // the cross sit in the same piece of glass as the words.
    let mut inside = row![
        icon(Icon::Search, 14.0, palette.text_muted),
        text_input(t(S::SearchServers, locale), app.tray_search())
            .on_input(Message::TraySearch)
            .padding(0)
            .size(12.5)
            .style(move |_, _| text_input::Style {
                background: Background::Color(Color::TRANSPARENT),
                border: Border::default(),
                icon: palette.text_muted,
                placeholder: palette.text_muted,
                value: palette.text,
                selection: theme::alpha(palette.accent, 0.28),
            }),
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    if !app.tray_search().is_empty() {
        inside = inside.push(
            button(icon(Icon::X, 12.0, palette.text_muted))
                .on_press(Message::TraySearch(String::new()))
                .padding(2)
                .style(|_, _| button::Style::default()),
        );
    }
    let field = container(inside)
        .padding([0, 12])
        .center_y(Length::Fixed(34.0))
        .width(Length::Fill)
        .style(move |_| theme::control(palette, radii::PILL));

    let pinging = app.is_pinging();
    let ping = button(
        container(
            row![
                icon_thin(
                    Icon::Zap,
                    13.0,
                    theme::alpha(palette.accent_ink, if pinging { 0.5 } else { 1.0 }),
                    2.2
                ),
                text(t(if pinging { S::Measuring } else { S::PingAll }, locale))
                    .size(12.5)
                    .font(moonlight_design::ui(EMPHATIC))
                    .color(palette.text)
                    .wrapping(iced::widget::text::Wrapping::None),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        )
        .center_y(Length::Fill),
    )
    .on_press_maybe(
        (!pinging && app.preferences().subscription_url.is_some()).then_some(Message::Ping),
    )
    .height(Length::Fixed(34.0))
    .padding([0, 12])
    .style(move |_, status| glass_pill(palette, status));

    row![field, ping]
        .spacing(8)
        .align_y(Alignment::Center)
        .into()
}

/// A capsule of glass that brightens under the pointer — the tray's quiet
/// buttons.
fn glass_pill(palette: Palette, status: button::Status) -> button::Style {
    let mut style = theme::icon_button(palette, status);
    style.text_color = palette.text;
    style.shadow = iced::Shadow::default();
    style
}

fn servers(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();

    if app.preferences().subscription_url.is_none() {
        return container(
            column![
                icon(Icon::Globe, 26.0, palette.text_muted),
                text(t(S::NoSubscription, locale))
                    .size(14.0)
                    .font(moonlight_design::ui(EMPHATIC))
                    .color(palette.text),
                text(t(S::NoSubscriptionHint, locale))
                    .size(12.0)
                    .color(palette.text_muted)
                    .align_x(Alignment::Center),
            ]
            .spacing(10)
            .align_x(Alignment::Center),
        )
        .padding([0, 28])
        .center(Length::Fill)
        .into();
    }

    let query = app.tray_search().trim().to_lowercase();
    let selected = app.preferences().selected_node.as_deref();
    let auto_on = app.preferences().auto_select;

    let mut rows = column![].spacing(2);
    let mut any = false;
    if query.is_empty() || t(S::Auto, locale).to_lowercase().contains(&query) {
        rows = rows.push(auto_row(app));
        any = true;
    }
    for node in app.nodes().iter().filter(|node| !node.is_auto_picker()) {
        let matches = query.is_empty()
            || node.name.to_lowercase().contains(&query)
            || node.subtitle(locale).to_lowercase().contains(&query)
            || node
                .description
                .as_deref()
                .is_some_and(|note| note.to_lowercase().contains(&query));
        if matches {
            let chosen = !auto_on && selected == Some(node.name.as_str());
            rows = rows.push(node_row(app, node, chosen));
            any = true;
        }
    }
    if !any {
        rows = rows.push(
            container(
                text(t(S::NothingFound, locale))
                    .size(12.5)
                    .color(palette.text_muted),
            )
            .center_x(Length::Fill)
            .padding(Padding {
                top: 24.0,
                ..Padding::ZERO
            }),
        );
    }

    // Room under the last row for the floating button, so it can be scrolled
    // clear of it; and no bar — the list is the panel, and a bar down its
    // edge is a second edge.
    scrollable(rows.padding(Padding {
        top: 10.0,
        right: 8.0,
        bottom: 72.0,
        left: 8.0,
    }))
    .id(LIST_SCROLL)
    .direction(scrollable::Direction::Vertical(
        scrollable::Scrollbar::new()
            .width(0)
            .scroller_width(0)
            .margin(0),
    ))
    .height(Length::Fill)
    .style(move |theme, _| theme::scroller(palette, theme))
    .into()
}

/// "VLESS", "HYSTERIA2" — the transport without its security layer, which is
/// what fits in a chip beside the server's description.
fn family(node: &Node) -> Option<String> {
    node.protocol_label
        .as_deref()
        .and_then(|label| label.split_whitespace().next())
        .or((!node.is_group && !node.kind.is_empty()).then_some(node.kind.as_str()))
        .map(str::to_uppercase)
}

/// A small label under a server's name. Quiet for the transport, which every
/// row has; the description is what tells rows apart, so it reads louder.
fn chip<'a>(label: String, quiet: bool, palette: Palette) -> Element<'a, Message> {
    container(
        text(label)
            .size(10.5)
            .font(moonlight_design::ui(if quiet { ROW_TITLE } else { MEDIUM }))
            .color(if quiet {
                palette.text_muted
            } else {
                palette.text
            })
            .wrapping(iced::widget::text::Wrapping::None),
    )
    .padding([0, 7])
    .center_y(Length::Fixed(19.0))
    .style(move |_| container::Style {
        background: Some(Background::Color(theme::alpha(
            palette.text,
            if quiet { 0.06 } else { 0.10 },
        ))),
        border: Border {
            radius: iced::border::Radius::from(radii::PILL),
            ..Default::default()
        },
        ..Default::default()
    })
    .into()
}

/// A description, cut to what a chip in a row has room for.
fn note(description: &str) -> String {
    if description.chars().count() > NOTE_ROOM {
        let cut: String = description.chars().take(NOTE_ROOM - 1).collect();
        format!("{}…", cut.trim_end())
    } else {
        description.to_string()
    }
}

/// The ping button and the number it produced.
fn latency<'a>(app: &'a Moonlight, node: &'a Node) -> Element<'a, Message> {
    let palette = app.palette_of();
    let measuring = app.is_probing(&node.name);
    let ping = button(
        container(icon_thin(
            Icon::Zap,
            13.0,
            if measuring {
                theme::alpha(palette.accent_ink, 0.5)
            } else {
                palette.text_muted
            },
            2.2,
        ))
        .center(Length::Fill),
    )
    .on_press_maybe((!measuring).then(|| Message::PingNode(node.name.clone())))
    .width(Length::Fixed(26.0))
    .height(Length::Fixed(26.0))
    .padding(0)
    .style(move |_, status| button::Style {
        background: matches!(status, button::Status::Hovered | button::Status::Pressed)
            .then_some(Background::Color(theme::alpha(palette.text, 0.08))),
        border: Border {
            radius: iced::border::Radius::from(radii::PILL),
            ..Default::default()
        },
        ..Default::default()
    });

    let (figure, tone) = if measuring {
        ("…".to_string(), palette.text_muted)
    } else {
        let tone = match node.latency {
            Some(ms) => palette.ping_color(ms),
            None if node.probed => palette.danger,
            None => palette.text_muted,
        };
        (format::latency(node.latency, node.probed), tone)
    };
    row![
        ping,
        container(
            text(figure)
                .font(moonlight_design::mono())
                .size(12.0)
                .color(tone)
                .wrapping(iced::widget::text::Wrapping::None),
        )
        .align_right(Length::Fixed(54.0)),
    ]
    .spacing(2)
    .align_y(Alignment::Center)
    .into()
}

/// The row shell both kinds share: the selection on glass, the hover wash.
fn row_frame<'a>(
    palette: Palette,
    selected: bool,
    content: iced::widget::Row<'a, Message>,
    message: Message,
) -> Element<'a, Message> {
    button(content.spacing(4).align_y(Alignment::Center))
        .on_press(message)
        .padding(Padding {
            top: 9.0,
            right: 8.0,
            bottom: 9.0,
            left: 10.0,
        })
        .width(Length::Fill)
        .style(move |_, status| {
            let mut style = theme::row_button(palette, selected, status);
            style.border.radius = iced::border::Radius::from(ROW_RADIUS);
            if selected {
                style.background = Some(Background::Color(palette.surface2));
                style.border.width = 1.0;
                style.border.color = palette.hairline;
            }
            style
        })
        .into()
}

/// The one automatic row, as in the window's drawer. The subscription's own
/// auto-picker, when it has one, is what stands behind it — so this row
/// carries that picker's transport and its latency.
fn auto_row(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let selected = app.preferences().auto_select;
    let picker = app.nodes().iter().find(|node| node.is_auto_picker());

    let (fill, ink) = if selected {
        (palette.accent, palette.text_on_accent)
    } else {
        (palette.surface2, palette.accent_ink)
    };
    let label = picker
        .and_then(family)
        .filter(|family| family != "URLTEST" && family != "FALLBACK")
        .unwrap_or_else(|| t(S::AutoSubtitle, locale).to_string());

    let mut content = row![row![
        container(icon_thin(Icon::Zap, 15.0, ink, 2.2))
            .center(Length::Fixed(26.0))
            .style(move |_| container::Style {
                background: Some(Background::Color(fill)),
                ..theme::control(palette, radii::PILL)
            }),
        column![
            text(t(S::Auto, locale))
                .size(13.5)
                .font(moonlight_design::ui(ROW_TITLE))
                .color(palette.text),
            chip(label, true, palette),
        ]
        .spacing(5),
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .width(Length::Fill),];
    if let Some(picker) = picker {
        content = content.push(latency(app, picker));
    }
    row_frame(
        palette,
        selected,
        content,
        Message::SelectNode(String::new()),
    )
}

/// A server: flag, name, what it runs and what it is for, and its latency
/// with a button to measure it again.
fn node_row<'a>(app: &'a Moonlight, node: &'a Node, selected: bool) -> Element<'a, Message> {
    let palette = app.palette_of();

    let mut chips = row![].spacing(5).align_y(Alignment::Center);
    if let Some(family) = family(node) {
        chips = chips.push(chip(family, true, palette));
    }
    if let Some(description) = node.description.as_deref() {
        chips = chips.push(chip(note(description), false, palette));
    }

    let content = row![
        row![
            container(connect::flag(app, node)).center_x(Length::Fixed(26.0)),
            column![
                text(node.title())
                    .size(13.5)
                    .font(moonlight_design::ui(ROW_TITLE))
                    .color(palette.text)
                    .wrapping(iced::widget::text::Wrapping::None),
                chips,
            ]
            .spacing(5)
            .width(Length::Fill)
            .clip(true),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .width(Length::Fill),
        latency(app, node),
    ];
    row_frame(
        palette,
        selected,
        content,
        Message::SelectNode(node.name.clone()),
    )
}

fn connect_button(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let state = app.state();
    let (connected, busy) = (state.is_connected(), state.is_busy());
    let has_subscription = app.preferences().subscription_url.is_some();
    let label = match state {
        ConnectionState::Connected => S::DisconnectVerb,
        ConnectionState::Connecting => S::Connecting,
        ConnectionState::Disconnecting => S::Disconnecting,
        _ => S::ConnectNow,
    };
    let glyph = if busy {
        Icon::LoaderCircle
    } else if connected {
        Icon::Square
    } else {
        Icon::Power
    };
    // The accent while there is something to start; once it is up, or on its
    // way, the button steps back to a surface and the state line carries it.
    let quiet = connected || busy;
    let ink = if quiet {
        palette.text
    } else {
        palette.text_on_accent
    };
    let ink = theme::alpha(ink, if has_subscription { 1.0 } else { 0.5 });

    button(
        container(
            row![
                icon_thin(glyph, 15.0, ink, 2.4),
                text(t(label, locale))
                    .size(14.0)
                    .font(moonlight_design::ui(EMPHATIC))
                    .color(ink)
                    .wrapping(iced::widget::text::Wrapping::None),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .center_y(Length::Fill),
    )
    .on_press_maybe((has_subscription && !busy).then_some(Message::ToggleConnection))
    // 44 tall inside a 3px ring of the canvas, which separates it from the
    // rows scrolling beneath where a shadow would be a glow.
    .height(Length::Fixed(50.0))
    .padding([0, 27])
    .style(move |_, status| {
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        // Solid either way: the list scrolls under it.
        let fill = match (quiet, hovered) {
            (false, false) => palette.accent,
            (false, true) => palette.accent_hover,
            (true, false) => palette.raised,
            (true, true) => theme::over(theme::alpha(palette.text, 0.08), palette.raised),
        };
        button::Style {
            background: Some(Background::Color(fill)),
            text_color: ink,
            border: Border {
                radius: iced::border::Radius::from(radii::PILL),
                width: 3.0,
                color: palette.bg,
            },
            ..Default::default()
        }
    })
    .into()
}

fn footer(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();

    let open = button(
        container(
            row![
                icon(Icon::Monitor, 14.0, palette.text),
                text(t(S::OpenWindow, locale))
                    .size(12.5)
                    .font(moonlight_design::ui(EMPHATIC))
                    .color(palette.text),
            ]
            .spacing(7)
            .align_y(Alignment::Center),
        )
        .center_y(Length::Fill),
    )
    .on_press(Message::OpenMain)
    .height(Length::Fixed(32.0))
    .padding([0, 13])
    .style(move |_, status| glass_pill(palette, status));

    let mut line = row![open, hspace(Length::Fill)]
        .spacing(8)
        .align_y(Alignment::Center);

    // The plan's name, and the way to its page — only once there is a plan.
    if let Some(title) = app
        .info()
        .title
        .clone()
        .filter(|_| app.preferences().subscription_url.is_some())
    {
        line = line.push(
            button(
                container(
                    row![
                        icon(Icon::Sparkles, 13.0, palette.accent_ink),
                        text(title)
                            .size(12.0)
                            .font(moonlight_design::ui(MEDIUM))
                            .color(palette.text2)
                            .wrapping(iced::widget::text::Wrapping::None),
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center),
                )
                .center_y(Length::Fill),
            )
            .on_press(Message::OpenPage(Page::Subscription))
            .height(Length::Fixed(32.0))
            .padding([0, 11])
            .style(move |_, status| button::Style {
                background: None,
                text_color: palette.text2,
                border: Border {
                    radius: iced::border::Radius::from(radii::PILL),
                    width: 1.0,
                    color: match status {
                        button::Status::Hovered | button::Status::Pressed => {
                            theme::alpha(palette.text, 0.28)
                        }
                        _ => palette.hairline,
                    },
                },
                ..Default::default()
            }),
        );
    }

    let quit = button(container(icon(Icon::LogOut, 14.0, palette.text_muted)).center(Length::Fill))
        .on_press(Message::Quit)
        .width(Length::Fixed(32.0))
        .height(Length::Fixed(32.0))
        .padding(0)
        .style(move |_, status| glass_pill(palette, status));

    column![
        components::divider(palette),
        container(line.push(quit))
            .padding([11, 12])
            .width(Length::Fill)
            .style(move |_| container::Style {
                background: Some(Background::Color(theme::alpha(palette.text, 0.03))),
                ..Default::default()
            }),
    ]
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(kind: &str, label: Option<&str>, group: bool) -> Node {
        Node {
            name: "x".into(),
            kind: kind.into(),
            server: None,
            latency: None,
            probed: false,
            is_group: group,
            protocol_label: label.map(str::to_string),
            description: None,
        }
    }

    #[test]
    fn a_chip_names_the_transport_without_its_security_layer() {
        assert_eq!(
            family(&node("vless", Some("VLESS Reality"), false)).as_deref(),
            Some("VLESS")
        );
        assert_eq!(
            family(&node("hysteria2", None, false)).as_deref(),
            Some("HYSTERIA2")
        );
        // A group the subscription built has no transport of its own.
        assert_eq!(family(&node("LoadBalance", None, true)), None);
    }

    #[test]
    fn a_long_description_is_cut_by_characters_and_marked() {
        assert_eq!(note("быстрый сервер"), "быстрый сервер");
        let cut = note(&"д".repeat(60));
        assert_eq!(cut.chars().count(), NOTE_ROOM);
        assert!(cut.ends_with('…'));
    }
}
