//! The connect screen: the time connected, the moon, and the servers.
//!
//! As the macOS client lays it out. Closed, the moon is the page: large, with
//! the column it heads — time, moon, state, servers — in the middle of the
//! page. Opening the server list shrinks the moon and lifts the column to the
//! top, and the list takes the room that frees, all on one curve.

use iced::widget::{button, canvas, column, container, row, scrollable, text, tooltip, Space};
use iced::{Alignment, Border, Element, Length};

use moonlight_core::{format, ConnectionState, Node, TunnelMode};
use moonlight_design::motion::{border, radii};
use moonlight_design::typography::{scale, EMPHATIC, MEDIUM, ROW_TITLE};
use moonlight_design::{icon, Icon};

use crate::components;
use crate::localization::{t, S};
use crate::moon::Moon;
use crate::{hspace, localization, theme, vspace, Message, Moonlight, Page};

/// The moon with the list open; closed it is twice this.
const MOON: f32 = 92.0;
/// The picker pill's height.
const PICKER: f32 = 60.0;
/// The widest the column gets, list included.
const COLUMN: f32 = 560.0;
/// The time's size, and how wide each digit is given so the figures do not
/// shift as they tick — the display face has no tabular digits.
const TIME: f32 = 24.0;
const DIGIT: f32 = TIME * 0.8;
const COLON: f32 = TIME * 0.42;

/// The connect shortcut, named in the moon's tooltip.
const SHORTCUT: &str = "Ctrl+Shift+C";

pub fn view(app: &Moonlight) -> Element<'_, Message> {
    let open = app.drawer();
    let has_nodes = !app.nodes().is_empty();

    // Spacers above and below centre the column while the list is closed,
    // and give their room to the list as it opens: at `open` of the way the
    // list has `open` of the free height. Weights, not measurements — iced
    // shares out the height itself, so nothing has to be measured first.
    let spacer = |weight: f32| Space::new().height(Length::FillPortion(portion(weight)));

    let mut page = column![].align_x(Alignment::Center).width(Length::Fill);
    if open < 1.0 {
        page = page.push(spacer(1.0 - open));
    }
    page = page
        .push(timer(app))
        .push(vspace(Length::Fixed(20.0)))
        .push(moon(app, MOON * (2.0 - open)))
        .push(below_moon(app));
    if has_nodes && open > 0.0 {
        page = page
            .push(vspace(Length::Fixed(10.0)))
            .push(drawer(app, open));
    }
    if open < 1.0 {
        page = page.push(spacer(1.0 - open));
    }

    let page = container(page.max_width(COLUMN).height(Length::Fill))
        .center_x(Length::Fill)
        .height(Length::Fill);

    // Over the page rather than in it, so its arrival moves nothing; at the
    // foot, where it covers neither the time nor the moon.
    match refresh_note(app) {
        Some(note) => iced::widget::stack![
            page,
            container(note)
                .center_x(Length::Fill)
                .align_bottom(Length::Fill),
        ]
        .into(),
        None => page.into(),
    }
}

/// A share of the free height as a fill portion. Never zero: a zero portion
/// is not "none" to iced but its own fill.
fn portion(weight: f32) -> u16 {
    (weight * 1000.0).round().clamp(1.0, 2000.0) as u16
}

/// How long the tunnel has been up.
fn timer(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let tone = if app.state().is_connected() {
        palette.text
    } else {
        palette.text_muted
    };

    let mut figures = row![];
    for character in format::duration(app.uptime()).chars() {
        let width = if character.is_ascii_digit() {
            DIGIT
        } else {
            COLON
        };
        figures = figures.push(
            container(
                text(character.to_string())
                    .font(moonlight_design::display())
                    .size(TIME)
                    .color(tone),
            )
            .center_x(Length::Fixed(width)),
        );
    }

    column![
        text(t(S::ConnectionTime, app.locale_of()))
            .size(12.0)
            .font(moonlight_design::ui(MEDIUM))
            .color(palette.text_muted),
        figures,
    ]
    .spacing(2)
    .align_x(Alignment::Center)
    .into()
}

/// The moon, as the button that connects and disconnects.
fn moon(app: &Moonlight, side: f32) -> Element<'_, Message> {
    let palette = app.palette_of();
    let state = app.state();
    let has_subscription = app.preferences().subscription_url.is_some();

    let press = button(
        canvas(Moon {
            palette,
            phase: app.moon_phase(),
            orbit: app.orbit(),
            enabled: has_subscription,
        })
        .width(Length::Fixed(side))
        .height(Length::Fixed(side)),
    )
    .on_press_maybe((has_subscription && !state.is_busy()).then_some(Message::ToggleConnection))
    .padding(0)
    .style(|_, _| button::Style::default());

    let hint = if state.is_connected() {
        S::PressToDisconnect
    } else {
        S::PressToConnect
    };
    tooltip(
        press,
        container(
            text(format!("{} · {SHORTCUT}", t(hint, app.locale_of())))
                .size(scale::META)
                .color(palette.text),
        )
        .padding([6, 10])
        .style(move |_| theme::panel(palette)),
        tooltip::Position::Bottom,
    )
    .into()
}

/// The state and the mode, why it is not connected, the service's message,
/// and the servers.
fn below_moon(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let state = app.state();

    let mut below = column![
        vspace(Length::Fixed(18.0)),
        row![status_pill(app), mode_switch(app)]
            .spacing(8)
            .align_y(Alignment::Center)
    ]
    .align_x(Alignment::Center)
    .width(Length::Fill);

    // Why it is not connected — a failed connect first; failing that, why the
    // subscription did not load. Not while the refresh note is saying the
    // same thing.
    let problem = app.last_error().map(str::to_string).or_else(|| {
        app.refresh_note()
            .is_none()
            .then(|| {
                app.refresh_issue()
                    .map(|issue| localization::issue(issue, locale))
            })
            .flatten()
    });
    if let Some(problem) = problem.filter(|_| !state.is_connected()) {
        below = below.push(vspace(Length::Fixed(14.0))).push(
            container(
                text(problem)
                    .size(scale::META)
                    .color(palette.danger)
                    .align_x(Alignment::Center),
            )
            .max_width(440),
        );
    }
    if let Some(message) = app.announce() {
        below = below
            .push(vspace(Length::Fixed(24.0)))
            .push(components::announce_banner(
                message,
                Message::DismissAnnounce,
                palette,
            ));
    }

    below
        .push(vspace(Length::Fixed(28.0)))
        .push(servers(app))
        .into()
}

/// The state in words, as a way into the connections screen. Filled with the
/// accent's wash while the tunnel is up.
fn status_pill(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let state = app.state();
    let connected = state.is_connected();
    let label = match state {
        ConnectionState::Connected => S::Connection,
        ConnectionState::Connecting => S::Connecting,
        ConnectionState::Disconnecting => S::Disconnecting,
        ConnectionState::Disconnected | ConnectionState::Failed(_) => S::Disconnected,
    };
    let ink = if connected {
        palette.accent_ink_strong
    } else {
        palette.text2
    };

    button(
        container(
            row![
                text(t(label, app.locale_of()))
                    .size(13.5)
                    .font(moonlight_design::ui(ROW_TITLE))
                    .color(ink),
                moonlight_design::icon_thin(Icon::ChevronRight, 14.0, ink, 2.4),
            ]
            .spacing(5)
            .align_y(Alignment::Center),
        )
        .center_y(Length::Fill),
    )
    .on_press(Message::Navigate(Page::Connections))
    .height(Length::Fixed(34.0))
    .padding(iced::Padding {
        top: 0.0,
        right: 12.0,
        bottom: 0.0,
        left: 16.0,
    })
    .style(move |_, status| {
        let fill = if connected {
            palette.accent_quiet
        } else {
            palette.surface
        };
        let rim = match status {
            button::Status::Hovered | button::Status::Pressed => palette.accent_line,
            _ => palette.hairline,
        };
        button::Style {
            background: Some(iced::Background::Color(fill)),
            text_color: ink,
            border: Border {
                radius: iced::border::Radius::from(radii::PILL),
                width: border::HAIRLINE,
                color: rim,
            },
            ..Default::default()
        }
    })
    .into()
}

/// Proxy or TUN, beside the state it is in.
fn mode_switch(app: &Moonlight) -> Element<'_, Message> {
    components::segmented_compact(
        &[
            (
                TunnelMode::SystemProxy,
                t(S::ModeProxyShort, app.locale_of()),
            ),
            (TunnelMode::Tun, t(S::ModeTun, app.locale_of())),
        ],
        app.preferences().mode,
        Message::ChooseMode,
        app.palette_of(),
    )
}

/// The heading, and the picker — or, before there is anything to pick, the
/// way to add a subscription.
fn servers(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let nodes = app.nodes();

    let mut heading = row![components::overline(t(S::Servers, locale), palette)]
        .spacing(10)
        .align_y(Alignment::Center);
    if !nodes.is_empty() {
        let count = nodes.iter().filter(|n| !n.is_auto_picker()).count();
        heading = heading.push(
            text(format!("{count} {}", t(S::Nodes, locale)))
                .size(12.0)
                .color(palette.text_muted),
        );
    }
    heading = heading.push(hspace(Length::Fill));
    // Ping and refresh sit over the list they act on, and only once there is
    // a subscription to measure or fetch again.
    if app.preferences().subscription_url.is_some() {
        heading = heading.push(list_action(
            app,
            Icon::Activity,
            if app.is_pinging() {
                S::Measuring
            } else {
                S::Ping
            },
            app.is_pinging(),
            Message::Ping,
        ));
        heading = heading.push(list_action(
            app,
            Icon::RefreshCw,
            if app.is_refreshing() {
                S::Refreshing
            } else {
                S::Refresh
            },
            app.is_refreshing(),
            Message::Refresh,
        ));
    }

    let body: Element<'_, Message> = if nodes.is_empty() {
        container(components::empty_state_full(
            Icon::Globe,
            t(S::NoSubscription, locale).to_string(),
            t(S::NoSubscriptionHint, locale).to_string(),
            Some((
                t(S::AddSubscription, locale).to_string(),
                Message::Navigate(Page::Import),
            )),
            palette,
        ))
        .padding(8)
        .width(Length::Fill)
        .style(move |_| theme::panel(palette))
        .into()
    } else {
        picker(app)
    };

    column![container(heading).padding([0, 6]), body]
        .spacing(10)
        .width(Length::Fill)
        .into()
}

/// The server in use, and the way into the rest — one row until it is opened.
fn picker(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();

    let picked = (!app.preferences().auto_select)
        .then(|| app.preferences().selected_node.as_deref())
        .flatten()
        .and_then(|name| app.nodes().iter().find(|n| n.name == name));

    let (glyph, title, subtitle): (Element<'_, Message>, String, String) = match picked {
        Some(node) => (flag(app, node), node.title(), about(node, app)),
        None => (
            moonlight_design::icon_thin(Icon::Zap, 18.0, palette.accent_ink, 2.2),
            t(S::Auto, locale).to_string(),
            app.auto_choice()
                .unwrap_or_else(|| t(S::AutoSubtitle, locale).to_string()),
        ),
    };

    let chevron = if app.servers_open() {
        Icon::ChevronUp
    } else {
        Icon::ChevronDown
    };

    button(
        container(
            row![
                container(glyph)
                    .center(Length::Fixed(40.0))
                    .style(move |_| container::Style {
                        background: Some(iced::Background::Color(palette.surface2)),
                        border: Border {
                            radius: iced::border::Radius::from(radii::PILL),
                            ..Default::default()
                        },
                        ..Default::default()
                    }),
                column![
                    text(title)
                        .size(14.5)
                        .font(moonlight_design::ui(EMPHATIC))
                        .color(palette.text)
                        .wrapping(iced::widget::text::Wrapping::None),
                    text(subtitle)
                        .size(12.0)
                        .color(palette.text_muted)
                        .wrapping(iced::widget::text::Wrapping::None),
                ]
                .spacing(1)
                .width(Length::Fill),
                moonlight_design::icon_thin(chevron, 15.0, palette.text2, 2.4),
            ]
            .spacing(12)
            .align_y(Alignment::Center),
        )
        .center_y(Length::Fill),
    )
    .on_press(Message::ToggleServers)
    .width(Length::Fill)
    .height(Length::Fixed(PICKER))
    .padding(iced::Padding {
        top: 0.0,
        right: 20.0,
        bottom: 0.0,
        left: 10.0,
    })
    .style(move |_, status| {
        let rim = match status {
            button::Status::Hovered | button::Status::Pressed => palette.accent_line,
            _ => palette.hairline,
        };
        button::Style {
            background: Some(iced::Background::Color(palette.surface)),
            text_color: palette.text,
            border: Border {
                radius: iced::border::Radius::from(radii::PILL),
                width: border::HAIRLINE,
                color: rim,
            },
            ..Default::default()
        }
    })
    .into()
}

/// Every server, under the picker. Always the same card; only the height it
/// is given moves, from nothing to its content, and past the room there is it
/// scrolls.
fn drawer(app: &Moonlight, open: f32) -> Element<'_, Message> {
    let palette = app.palette_of();

    let mut rows = column![auto_row(app)].spacing(2);
    let mut count = 1;
    for node in app.nodes() {
        // A panel that already offers a url-test picker makes the app's own
        // Авто row redundant, so only one of the two is shown.
        if node.is_auto_picker() {
            continue;
        }
        let selected = !app.preferences().auto_select
            && app.preferences().selected_node.as_deref() == Some(node.name.as_str());
        rows = rows.push(node_row(app, node, selected));
        count += 1;
    }

    // What the rows come to, so a short list is a short card rather than one
    // stretched to the foot of the page. Rows are a fixed height; a little
    // over is harmless, it only scrolls.
    let content = 16.0 + count as f32 * ROW_ESTIMATE;

    container(
        scrollable(rows.padding(iced::Padding {
            right: crate::SCROLLBAR_GUTTER,
            ..iced::Padding::ZERO
        }))
        .direction(scrollable::Direction::Vertical(
            scrollable::Scrollbar::new()
                .width(crate::SCROLLBAR_WIDTH)
                .scroller_width(crate::SCROLLBAR_WIDTH)
                .margin(crate::SCROLLBAR_MARGIN),
        ))
        .height(Length::Fill)
        .style(move |theme, _| theme::scroller(palette, theme)),
    )
    .padding(8)
    .width(Length::Fill)
    .height(Length::FillPortion(portion(2.0 * open)))
    .max_height(content)
    .style(move |_| theme::panel(palette))
    .into()
}

/// A row's height in the list: 10 above and below two lines of type.
const ROW_ESTIMATE: f32 = 58.0;

/// What a server is for, when the service says; otherwise its country and
/// transport.
fn about(node: &Node, app: &Moonlight) -> String {
    node.description
        .clone()
        .unwrap_or_else(|| node.subtitle(app.locale_of()))
}

/// What the refresh the user asked for came to: updated, or not and why.
/// Refreshing used to turn the glyph and stop, and whether anything had
/// happened was left to a timestamp on another page. Clicking it puts it away;
/// otherwise it goes on its own.
fn refresh_note(app: &Moonlight) -> Option<Element<'_, Message>> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let (id, outcome) = app.refresh_note()?;
    let (glyph, tone, title, detail) = match outcome {
        Ok(()) => (
            Icon::Check,
            palette.st_up_ink,
            t(S::RefreshDone, locale),
            String::new(),
        ),
        Err(issue) => (
            Icon::CircleAlert,
            palette.danger,
            t(S::RefreshFailed, locale),
            localization::issue(issue, locale),
        ),
    };
    let mut words = column![text(title)
        .size(13.5)
        .font(moonlight_design::ui(EMPHATIC))
        .color(palette.text)]
    .spacing(1);
    if !detail.is_empty() {
        words = words.push(text(detail).size(12.0).color(palette.text_muted));
    }
    Some(
        button(
            row![moonlight_design::icon(glyph, 15.0, tone), words]
                .spacing(12)
                .align_y(Alignment::Center),
        )
        .on_press(Message::HideRefreshNote(id))
        .padding([9, 16])
        .style(move |_, status| theme::row_button(palette, false, status))
        .into(),
    )
}

/// A round glyph button over the server list, named by its tooltip. Its glyph
/// becomes the loader while the work runs.
fn list_action<'a>(
    app: &'a Moonlight,
    glyph: Icon,
    label: S,
    busy: bool,
    message: Message,
) -> Element<'a, Message> {
    let palette = app.palette_of();
    let glyph = if busy { Icon::LoaderCircle } else { glyph };
    let button = button(
        container(moonlight_design::icon_thin(glyph, 16.0, palette.text2, 2.2))
            .center(Length::Fill),
    )
    .on_press(message)
    .width(Length::Fixed(34.0))
    .height(Length::Fixed(34.0))
    .padding(0)
    .style(move |_, status| theme::icon_button(palette, status));
    tooltip(
        button,
        container(
            text(t(label, app.locale_of()))
                .size(scale::META)
                .color(palette.text),
        )
        .padding([6, 10])
        .style(move |_| theme::panel(palette)),
        tooltip::Position::Bottom,
    )
    .into()
}

/// "Авто" is the app's own latency picker.
fn auto_row(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let selected = app.preferences().auto_select;

    // Selection is carried by the **tile**, not by the row: the row goes
    // surface-2 like any selected row, and the tile fills with the accent. The
    // other way round — an accent wash behind an accent tile — composites to
    // olive and loses the tile entirely.
    let (tile_fill, tile_ink) = if selected {
        (palette.accent, palette.text_on_accent)
    } else {
        (palette.surface2, palette.accent_ink)
    };

    // "Выбран Helsinki · 37 ms" once Auto has actually picked something, so the
    // row says what it did rather than only what it is for.
    let subtitle = match app.auto_choice() {
        Some(choice) if selected => choice,
        _ => t(S::AutoSubtitle, locale).to_string(),
    };

    let content = row![
        container(moonlight_design::icon_thin(Icon::Zap, 18.0, tile_ink, 2.2))
            .width(Length::Fixed(36.0))
            .height(Length::Fixed(36.0))
            .center(Length::Fixed(36.0))
            .style(move |_| container::Style {
                background: Some(iced::Background::Color(tile_fill)),
                border: Border {
                    radius: iced::border::Radius::from(radii::ICON),
                    ..Default::default()
                },
                ..Default::default()
            }),
        column![
            text(t(S::Auto, locale))
                .size(scale::BODY_SM)
                .font(moonlight_design::ui(EMPHATIC))
                .color(palette.text),
            text(subtitle).size(12.0).color(palette.text_muted),
        ]
        .spacing(1),
    ]
    .spacing(12)
    .align_y(Alignment::Center);

    button(content)
        .on_press(Message::SelectNode(String::new()))
        .padding([11, 12])
        .width(Length::Fill)
        .style(move |_, status| theme::row_button(palette, selected, status))
        .into()
}

/// A node's flag.
///
/// A picture, not the emoji. Windows renders a regional-indicator pair as the
/// two letters it is built from — 🇩🇪 comes out as "DE" — and no system font on
/// the platform can draw these, so the list read as a column of country codes.
pub fn flag<'a>(app: &'a Moonlight, node: &'a Node) -> Element<'a, Message> {
    match node.region_code().and_then(|c| app.flag_image(&c)) {
        Some(handle) => container(
            iced::widget::image(handle)
                .width(Length::Fixed(24.0))
                .height(Length::Fixed(18.0)),
        )
        .width(Length::Fixed(24.0))
        .into(),
        // A cross-country balancer has no flag, and inventing one would be a lie
        // about where the traffic goes.
        None => container(icon(Icon::Globe, 18.0, app.palette_of().text_muted))
            .width(Length::Fixed(24.0))
            .center_x(Length::Fixed(24.0))
            .into(),
    }
}

fn node_row<'a>(app: &'a Moonlight, node: &'a Node, selected: bool) -> Element<'a, Message> {
    let palette = app.palette_of();

    let flag = flag(app, node);

    // A node still being measured shows a spinner rather than its old number,
    // so a stale figure is never mistaken for a fresh one.
    // A dot in the latency colour, and the figure itself in text-2. Colouring
    // the number instead puts a green or orange digit in a column of white type,
    // which reads as a warning rather than as a measurement.
    let latency: Element<'a, Message> = if app.is_probing(&node.name) {
        text("…").size(scale::META).color(palette.text_muted).into()
    } else {
        let (dot, label) = match node.latency {
            Some(ms) => (palette.ping_color(ms), format::latency(Some(ms), true)),
            None => (palette.text_muted, format::latency(None, node.probed)),
        };
        row![
            container(vspace(Length::Fixed(6.0)))
                .width(Length::Fixed(6.0))
                .style(move |_| container::Style {
                    background: Some(iced::Background::Color(dot)),
                    border: Border {
                        radius: iced::border::Radius::from(radii::PILL),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
            text(label)
                .font(moonlight_design::mono())
                .size(scale::META)
                .color(palette.text2),
        ]
        .spacing(5)
        .align_y(Alignment::Center)
        .into()
    };

    let content = row![
        flag,
        column![
            text(node.title())
                .size(scale::BODY_SM)
                .font(moonlight_design::ui(ROW_TITLE))
                .color(palette.text),
            // What the service says the row is for, when it says — "Poland LTE
            // 1" means little until "Доступность во время БС" is under it.
            text(about(node, app))
                .size(12.0)
                .color(palette.text_muted)
                .wrapping(iced::widget::text::Wrapping::None),
        ]
        .spacing(1)
        .width(Length::Fill),
        latency,
    ]
    .spacing(12)
    .align_y(Alignment::Center);

    button(content)
        .on_press(Message::SelectNode(node.name.clone()))
        .padding([10, 12])
        .width(Length::Fill)
        .style(move |_, status| theme::row_button(palette, selected, status))
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_takes_the_room_the_spacers_give_up() {
        // Half open: the list's share equals both spacers' together, so it has
        // half the free height. Closed and open, nothing rounds to zero.
        assert_eq!(portion(2.0 * 0.5), 2 * portion(1.0 - 0.5));
        assert_eq!(portion(0.0), 1);
        assert_eq!(portion(2.0), 2000);
    }
}
