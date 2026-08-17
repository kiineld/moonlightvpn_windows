//! The connect screen: the dial on the left, the server list on the right.

use iced::widget::{button, canvas, column, container, row, text};
use iced::{Alignment, Border, Element, Length};

use moonlight_core::{format, AppLocale, ConnectionState, Node};
use moonlight_design::motion::{border, metrics, radii};
use moonlight_design::typography::{line, scale, EMPHATIC, ROW_TITLE};
use moonlight_design::{icon, Icon};

use crate::components;
use crate::dial::Dial;
use crate::localization::{t, S};
use crate::{hspace, theme, vspace, Message, Moonlight};

/// The dial's drawn size, from the composition.
const DIAL: f32 = metrics::DIAL;

/// The dial's big label. Smaller than `--ml-t-hero`: 40px does not fit inside a
/// 238px ring alongside a status line and a timer, and the composition sets 26.
const BIG_LABEL: f32 = 26.0;

/// The stats strip's figures.
const STAT_VALUE: f32 = 20.0;

pub fn view(app: &Moonlight) -> Element<'_, Message> {
    row![
        container(dial_column(app))
            .padding(24)
            .height(Length::Fill)
            .width(Length::FillPortion(3))
            .style({
                let palette = app.palette_of();
                move |_| theme::panel(palette)
            }),
        container(server_column(app))
            .padding(16)
            .height(Length::Fill)
            .width(Length::FillPortion(2))
            .style({
                let palette = app.palette_of();
                move |_| theme::panel(palette)
            }),
    ]
    .spacing(metrics::GAP_COLUMNS)
    .height(Length::Fill)
    .into()
}

fn dial_column(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let state = app.state();
    let connected = state.is_connected();

    // One tone drives the status label, the dot and the timer — the composition
    // keys all three off whether the tunnel is up, not off three separate roles.
    let (label, tone) = match state {
        ConnectionState::Connected => (S::StateSecure, palette.accent_ink),
        ConnectionState::Connecting => (S::Connecting, palette.text2),
        ConnectionState::Disconnecting => (S::Disconnecting, palette.text2),
        ConnectionState::Failed(_) => (S::StateFailed, palette.danger),
        ConnectionState::Disconnected => (S::StateDisconnected, palette.text_muted),
    };

    // "Соединение" while up, not "Отключить": the dial names what you *have*,
    // and the hint under it says what pressing does.
    let action = match state {
        ConnectionState::Connected => S::Connection,
        ConnectionState::Connecting => S::Connecting,
        ConnectionState::Disconnecting => S::Disconnecting,
        _ => S::Connect,
    };

    let face = column![
        row![
            // 8px, and it glows while connected — the one glow the system
            // allows, reserved for tiny accent marks like this.
            container(vspace(Length::Fixed(8.0)))
                .width(Length::Fixed(8.0))
                .style(move |_| container::Style {
                    background: Some(iced::Background::Color(tone)),
                    border: Border {
                        radius: iced::border::Radius::from(radii::PILL),
                        ..Default::default()
                    },
                    shadow: if connected {
                        theme::glow_sm(tone)
                    } else {
                        iced::Shadow::default()
                    },
                    ..Default::default()
                }),
            text(t(label, locale))
                .size(scale::MICRO)
                .font(moonlight_design::ui(EMPHATIC))
                .color(tone),
        ]
        .spacing(7)
        .align_y(Alignment::Center),
        text(t(action, locale))
            .font(moonlight_design::display())
            .size(BIG_LABEL)
            .line_height(line::TIGHT)
            .color(palette.text),
        text(format::duration(app.uptime()))
            .font(moonlight_design::mono())
            .size(15.0)
            .color(tone),
    ]
    .spacing(7)
    .align_x(Alignment::Center);

    let ring = canvas(Dial::new(
        state.clone(),
        palette,
        app.progress(),
        app.breath(),
    ))
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
        None => text(t(
            if connected {
                S::PressToDisconnect
            } else {
                S::PressToConnect
            },
            locale,
        ))
        .size(scale::META)
        .color(palette.text_muted)
        .into(),
    };

    let hint_row = row![
        hint,
        // The shortcut lives in a chip beside the hint rather than in the hint's
        // own sentence, so the sentence stays translatable and the keys stay
        // monospaced.
        container(
            text(SHORTCUT)
                .font(moonlight_design::mono())
                .size(11.0)
                .color(palette.text2)
        )
        .padding([0, 8])
        .height(Length::Fixed(22.0))
        .align_y(Alignment::Center)
        .style(move |_| container::Style {
            background: Some(iced::Background::Color(palette.surface2)),
            border: Border {
                radius: iced::border::Radius::from(radii::CHIP),
                width: border::HAIRLINE,
                color: palette.hairline,
            },
            ..Default::default()
        }),
    ]
    .spacing(9)
    .align_y(Alignment::Center);

    container(
        column![press, hint_row, stats(app)]
            .spacing(20)
            .align_x(Alignment::Center),
    )
    // Centred in whatever height the panel has, which is what
    // `justify-content:center` does in the composition — and unlike a pair of
    // Fill spacers it still works when the height is unbounded.
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .into()
}

/// The connect shortcut. The composition spells it per platform; this build is
/// the Windows one.
const SHORTCUT: &str = "Ctrl+Shift+C";

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
                .size(STAT_VALUE)
                .color(palette.text),
        ]
        .spacing(6)
        .align_x(Alignment::Center)
        .width(Length::Fill)
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

    // With no subscription all three read "—". Showing "без срока" for the third
    // while the other two are dashes claims a plan with no expiry rather than no
    // plan at all — and it is wide enough to overrun its third of the strip.
    let remaining = if app.preferences().subscription_url.is_some() {
        format::time_left(info.expire, locale)
    } else {
        format::bytes(None, locale)
    };

    container(
        row![
            cell(S::Downloaded, downloaded),
            divider(app),
            cell(S::Uploaded, uploaded),
            divider(app),
            cell(S::Remaining, remaining),
        ]
        .align_y(Alignment::Center),
    )
    .width(Length::Fixed(metrics::STATS_MAX))
    .padding([16, 6])
    .style(move |_| theme::card(palette))
    .into()
}

fn divider(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    container(hspace(Length::Fixed(1.0)))
        .height(Length::Fixed(38.0))
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

    let heading = container(
        row![
            components::overline(t(S::Servers, locale), palette),
            hspace(Length::Fill),
            text(format!("{} {}", nodes.len(), t(S::Nodes, locale)))
                .size(12.0)
                .color(palette.text_muted),
        ]
        .align_y(Alignment::Center),
    )
    .padding([2, 4]);

    let mut list = column![
        heading,
        vspace(Length::Fixed(12.0)),
        auto_row(app),
        vspace(Length::Fixed(8.0)),
        components::soft_divider(palette),
        vspace(Length::Fixed(8.0)),
    ]
    .spacing(2);

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

fn node_row<'a>(app: &'a Moonlight, node: &'a Node, selected: bool) -> Element<'a, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();

    let flag: Element<'a, Message> = match node.flag() {
        Some(flag) => text(flag).size(20.0).into(),
        // A cross-country balancer has no flag, and inventing one would be a lie
        // about where the traffic goes.
        None => icon(Icon::Globe, 18.0, palette.text_muted),
    };

    // A node still being measured shows a spinner rather than its old number,
    // so a stale figure is never mistaken for a fresh one.
    // A dot in the latency colour, and the figure itself in text-2. Colouring
    // the number instead puts a green or orange digit in a column of white type,
    // which reads as a warning rather than as a measurement.
    let latency: Element<'a, Message> = if app.is_probing(&node.name) {
        text("…").size(scale::META).color(palette.text_muted).into()
    } else {
        let (dot, label) = match node.latency {
            Some(ms) => (palette.ping_color(ms), format::latency(Some(ms))),
            None => (palette.text_muted, format::latency(None)),
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
            text(node.subtitle(locale))
                .size(12.0)
                .color(palette.text_muted),
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
