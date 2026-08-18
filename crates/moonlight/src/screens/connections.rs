//! What is going through the tunnel, as a table.
//!
//! One row per program, unfolding in place to the hosts behind it. Expanding
//! rather than pushing a second screen is what lets two programs be compared —
//! and the question here is usually comparative: *this* one is going through the
//! tunnel, is that one?
//!
//! Grouping by process is why `find-process-mode` is always on in the generated
//! config — without it every row reads "—".

use iced::widget::{button, column, container, row, scrollable, text, text_input};
use iced::{Alignment, Element, Length};

use moonlight_core::api::Connection;
use moonlight_core::{format, AppLocale};
use moonlight_design::motion::radii;
use moonlight_design::typography::{scale, EMPHATIC, ROW_TITLE};
use moonlight_design::{icon, Icon, Palette};

use crate::components;
use crate::localization::{t, S};
use crate::{hspace, theme, vspace, Message, Moonlight};

/// The column widths. Fixed so every row lines up under its heading, which is
/// the whole point of a table; the process column takes whatever is left.
const CHAIN: f32 = 150.0;
const RULE: f32 = 92.0;
const NET: f32 = 74.0;
const BYTES: f32 = 78.0;
const TIME: f32 = 52.0;
/// The trailing close control, and the leading count.
const CLOSE: f32 = 24.0;
const COUNT: f32 = 20.0;

pub fn view(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let needle = app.connection_filter().to_lowercase();

    // A filter matches on either level: typing "telegram" keeps that program,
    // and typing "443" keeps the hosts inside every program that has one.
    let grouped: Vec<(String, Vec<&Connection>)> = app
        .connections_by_process()
        .into_iter()
        .filter_map(|(process, connections)| {
            if needle.is_empty() || process.to_lowercase().contains(&needle) {
                return Some((process, connections));
            }
            let matching: Vec<&Connection> = connections
                .into_iter()
                .filter(|c| c.host.to_lowercase().contains(&needle))
                .collect();
            (!matching.is_empty()).then_some((process, matching))
        })
        .collect();

    let empty_list = grouped.is_empty();
    let mut rows = column![].spacing(0);
    for (index, (process, connections)) in grouped.into_iter().enumerate() {
        if index > 0 {
            rows = rows.push(components::soft_divider(palette));
        }
        let expanded = app.is_process_expanded(&process);
        rows = rows.push(process_row(app, process, &connections, expanded));
        if expanded {
            for connection in connections {
                rows = rows.push(host_row(app, connection));
            }
        }
    }

    let body: Element<'_, Message> = if empty_list {
        empty(app)
    } else {
        scrollable(rows.padding(iced::Padding {
            right: crate::SCROLLBAR_GUTTER,
            ..iced::Padding::ZERO
        }))
        .direction(scrollbar())
        .height(Length::Fill)
        .style(move |theme, _| theme::scroller(palette, theme))
        .into()
    };

    column![
        controls(app),
        vspace(Length::Fixed(14.0)),
        heading(palette, locale),
        components::surface(body, palette),
    ]
    .height(Length::Fill)
    .into()
}

/// One program: how many connections it holds, its own icon, and its totals.
fn process_row<'a>(
    app: &'a Moonlight,
    process: String,
    connections: &[&'a Connection],
    expanded: bool,
) -> Element<'a, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();

    let download: i64 = connections.iter().map(|c| c.download).sum();
    let upload: i64 = connections.iter().map(|c| c.upload).sum();
    // The oldest connection is how long the program has been talking.
    let age = connections.iter().map(|c| elapsed(c)).max().unwrap_or(0);

    // The chain and rule of the busiest connection stand for the group: a
    // program's connections almost always share both, and where they do not,
    // unfolding shows the difference.
    let representative = connections.first().copied();

    let count = container(
        text(format!("{}", connections.len()))
            .size(10.5)
            .font(moonlight_design::ui(EMPHATIC))
            .color(palette.text_muted),
    )
    .width(Length::Fixed(COUNT));

    let chevron = if expanded {
        Icon::ChevronDown
    } else {
        Icon::ChevronRight
    };

    let name = row![
        count,
        app_glyph(app, &process),
        text(process.clone())
            .size(scale::BODY_SM)
            .font(moonlight_design::ui(ROW_TITLE))
            .color(palette.text),
        moonlight_design::icon_thin(chevron, 14.0, palette.text_muted, 2.2),
    ]
    .spacing(9)
    .align_y(Alignment::Center);

    let content = row![
        container(name).width(Length::Fill),
        chain_cell(app, representative),
        rule_cell(palette, representative, locale),
        network_cell(palette, connections),
        bytes_cell(palette, download, locale, true),
        bytes_cell(palette, upload, locale, false),
        time_cell(palette, age),
        // Closing a program's connections at once, without unfolding it.
        close_button(palette, CloseTarget::Process(process.clone())),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    button(content)
        .on_press(Message::ToggleConnectionProcess(process))
        .padding([9, 14])
        .width(Length::Fill)
        .style(move |_, status| theme::row_button(palette, false, status))
        .into()
}

/// One host under an unfolded program. Indented, and dimmer, because it is a
/// detail of the row above rather than a peer of it.
fn host_row<'a>(app: &'a Moonlight, connection: &'a Connection) -> Element<'a, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let one = [connection];

    let name = row![
        hspace(Length::Fixed(COUNT + 9.0)),
        text(connection.host.clone())
            .font(moonlight_design::mono())
            .size(scale::META)
            .color(palette.text2),
    ]
    .align_y(Alignment::Center);

    row![
        container(name).width(Length::Fill),
        chain_cell(app, Some(connection)),
        rule_cell(palette, Some(connection), locale),
        network_cell(palette, &one),
        bytes_cell(palette, connection.download, locale, true),
        bytes_cell(palette, connection.upload, locale, false),
        time_cell(palette, elapsed(connection)),
        close_button(palette, CloseTarget::One(connection.id.clone())),
    ]
    .spacing(10)
    .padding([7, 14])
    .align_y(Alignment::Center)
    .into()
}

// MARK: - Cells

/// What a close control shuts: one connection, or everything a program holds.
enum CloseTarget {
    One(String),
    Process(String),
}

/// Closing one reads as "move this onto the node I just picked", because the
/// core reopens whatever is still wanted.
fn close_button<'a>(palette: Palette, target: CloseTarget) -> Element<'a, Message> {
    let message = match target {
        CloseTarget::One(id) => Message::CloseConnection(id),
        CloseTarget::Process(process) => Message::CloseProcessConnections(process),
    };
    button(moonlight_design::icon_thin(
        Icon::X,
        13.0,
        palette.text_muted,
        2.2,
    ))
    .on_press(message)
    .padding(4)
    .width(Length::Fixed(CLOSE))
    .style(move |_, status| theme::nav_button(palette, status))
    .into()
}

fn scrollbar() -> scrollable::Direction {
    scrollable::Direction::Vertical(
        scrollable::Scrollbar::new()
            .width(crate::SCROLLBAR_WIDTH)
            .scroller_width(crate::SCROLLBAR_WIDTH)
            .margin(crate::SCROLLBAR_MARGIN),
    )
}

/// The controls strip: how many are open, a search, and a way to end them all.
fn controls(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let open = app.connections().len();

    let search = row![
        icon(Icon::Search, 14.0, palette.text_muted),
        text_input(t(S::SearchApps, locale), app.connection_filter())
            .on_input(Message::ConnectionFilterChanged)
            .padding(0)
            .size(scale::META)
            .width(Length::Fixed(200.0))
            .style(move |_, status| {
                // The magnifier is the affordance; the field itself is bare, as
                // the composition draws it.
                let mut style = theme::field(palette, status);
                style.background = iced::Background::Color(iced::Color::TRANSPARENT);
                style.border.width = 0.0;
                style
            }),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let live = open > 0;
    let close_ink = if live {
        palette.danger
    } else {
        theme::alpha(palette.danger, 0.5)
    };

    row![
        components::count_pill(
            format!("{}: {open}", t(S::ActiveConnections, locale)),
            palette,
        ),
        search,
        hspace(Length::Fill),
        button(components::centre(
            row![
                moonlight_design::icon_thin(Icon::X, 13.0, close_ink, 2.4),
                text(t(S::CloseAll, locale))
                    .size(scale::META)
                    .font(moonlight_design::ui(EMPHATIC))
                    .color(close_ink),
            ]
            .spacing(7)
            .align_y(Alignment::Center),
        ))
        .on_press_maybe(live.then_some(Message::CloseAllConnections))
        .padding([0, 13])
        .height(Length::Fixed(30.0))
        .style(move |_, _| button::Style {
            background: Some(iced::Background::Color(if live {
                palette.danger_quiet
            } else {
                theme::alpha(palette.danger_quiet, 0.5)
            })),
            text_color: close_ink,
            border: iced::Border {
                radius: iced::border::Radius::from(radii::PILL),
                ..Default::default()
            },
            ..Default::default()
        }),
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .into()
}

/// The column headings, in the overline the rest of the app uses for a label.
fn heading(palette: Palette, locale: AppLocale) -> Element<'static, Message> {
    let cell = move |label: &'static str, width: f32, right: bool| -> Element<'static, Message> {
        let mut cell = container(components::overline(label, palette)).width(Length::Fixed(width));
        if right {
            cell = cell.align_x(Alignment::End);
        }
        cell.into()
    };

    container(
        row![
            container(components::overline(t(S::ColProcess, locale), palette)).width(Length::Fill),
            cell("ЦЕПОЧКА", CHAIN, false),
            cell("ПРАВИЛО", RULE, false),
            cell("СЕТЬ", NET, false),
            // Numbers are read by their last digit, so they align right — and so
            // do their headings.
            cell(t(S::Downloaded, locale), BYTES, true),
            cell(t(S::Uploaded, locale), BYTES, true),
            cell("ВРЕМЯ", TIME, true),
            hspace(Length::Fixed(CLOSE)),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .padding([0, 14])
    .center_y(Length::Fixed(30.0))
    .into()
}

/// The programme's own icon, falling back to a generic one.
fn app_glyph<'a>(app: &'a Moonlight, process: &str) -> Element<'a, Message> {
    match app.app_icon(process) {
        Some(handle) => iced::widget::image(handle.clone())
            .width(Length::Fixed(18.0))
            .height(Length::Fixed(18.0))
            .into(),
        None => icon(Icon::Monitor, 16.0, app.palette_of().text_muted),
    }
}

/// Where it actually left the machine: the flag, the country, and a mark saying
/// it went through the tunnel at all.
///
/// In the accent, because this is the column the whole screen is read for — a
/// row that says DIRECT is the one worth spotting, and it says so by *not* being
/// coloured.
fn chain_cell<'a>(app: &'a Moonlight, connection: Option<&'a Connection>) -> Element<'a, Message> {
    let palette = app.palette_of();
    let Some(connection) = connection else {
        return blank(CHAIN);
    };

    let node = connection.node();
    if node.is_empty() || node.eq_ignore_ascii_case("direct") {
        // Shown verbatim rather than translated: DIRECT is the chain the core
        // reported, not a word this app chose, and it appears in mihomo's own
        // logs and rules under that name.
        return container(
            text("DIRECT")
                .size(scale::META)
                .font(moonlight_design::ui(EMPHATIC))
                .color(palette.text_muted),
        )
        .width(Length::Fixed(CHAIN))
        .into();
    }

    let mut cell = row![].spacing(5).align_y(Alignment::Center);
    if let Some(handle) = app.node_region(node).and_then(|code| app.flag_image(&code)) {
        cell = cell.push(
            iced::widget::image(handle)
                .width(Length::Fixed(16.0))
                .height(Length::Fixed(12.0)),
        );
    }
    cell = cell.push(
        text(app.node_country(node).unwrap_or_else(|| node.to_string()))
            .size(scale::META)
            .font(moonlight_design::ui(EMPHATIC))
            .color(palette.accent_ink),
    );
    cell = cell.push(moonlight_design::icon_thin(
        Icon::Zap,
        11.0,
        palette.accent_ink,
        2.4,
    ));

    container(cell).width(Length::Fixed(CHAIN)).into()
}

fn rule_cell<'a>(
    palette: Palette,
    connection: Option<&'a Connection>,
    locale: AppLocale,
) -> Element<'a, Message> {
    let label = match connection {
        Some(connection) if !connection.rule.is_empty() => connection.rule.clone(),
        Some(_) => t(S::Unknown, locale).to_string(),
        None => "—".to_string(),
    };
    container(
        text(label)
            .size(scale::META)
            .color(palette.text_muted)
            .wrapping(text::Wrapping::None),
    )
    .width(Length::Fixed(RULE))
    .clip(true)
    .into()
}

/// TCP and UDP as separate chips, because a program doing both is worth seeing
/// at a glance.
fn network_cell<'a>(palette: Palette, connections: &[&Connection]) -> Element<'a, Message> {
    let mut tcp = false;
    let mut udp = false;
    for connection in connections {
        match connection.network.to_lowercase().as_str() {
            "udp" => udp = true,
            _ => tcp = true,
        }
    }

    let chip = move |label: &'static str, ink: iced::Color| -> Element<'a, Message> {
        container(
            text(label)
                .size(9.5)
                .font(moonlight_design::ui(EMPHATIC))
                .color(ink),
        )
        .padding([2, 5])
        .style(move |_| container::Style {
            background: Some(iced::Background::Color(theme::alpha(ink, 0.14))),
            border: iced::Border {
                radius: iced::border::Radius::from(radii::CHIP),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
    };

    let mut chips = row![].spacing(4).align_y(Alignment::Center);
    if tcp {
        chips = chips.push(chip("TCP", palette.st_up_ink));
    }
    if udp {
        chips = chips.push(chip("UDP", palette.st_degraded_ink));
    }

    container(chips).width(Length::Fixed(NET)).into()
}

fn bytes_cell<'a>(
    palette: Palette,
    value: i64,
    locale: AppLocale,
    emphatic: bool,
) -> Element<'a, Message> {
    // Downloaded is the figure people scan for, so it carries the weight.
    let ink = if emphatic {
        palette.text
    } else {
        palette.text2
    };
    container(
        text(format::bytes(Some(value), locale))
            .font(moonlight_design::mono())
            .size(scale::META)
            .color(ink),
    )
    .width(Length::Fixed(BYTES))
    .align_x(Alignment::End)
    .into()
}

fn time_cell<'a>(palette: Palette, seconds: i64) -> Element<'a, Message> {
    container(
        text(short_age(seconds))
            .font(moonlight_design::mono())
            .size(scale::META)
            .color(palette.text_muted),
    )
    .width(Length::Fixed(TIME))
    .align_x(Alignment::End)
    .into()
}

fn blank<'a>(width: f32) -> Element<'a, Message> {
    container(vspace(Length::Fixed(0.0)))
        .width(Length::Fixed(width))
        .into()
}

fn empty(app: &Moonlight) -> Element<'_, Message> {
    let message = if app.state().is_connected() {
        S::NoConnections
    } else {
        S::ConnectionsNeedTunnel
    };
    components::empty_state_icon(Icon::Globe, t(message, app.locale_of()), app.palette_of())
}

/// How long a connection has been open, in seconds.
fn elapsed(connection: &Connection) -> i64 {
    let now = time::OffsetDateTime::now_utc();
    (now - connection.start).whole_seconds().max(0)
}

/// A column this narrow has room for a number and one letter, not a sentence.
pub fn short_age(seconds: i64) -> String {
    match seconds {
        s if s < 60 => format!("{s} с"),
        s if s < 3600 => format!("{} м", s / 60),
        s if s < 86_400 => format!("{} ч", s / 3600),
        s => format!("{} д", s / 86_400),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_age_column_stays_short_enough_to_fit() {
        // Whatever the value, it has to sit in a 52pt column beside a heading.
        for seconds in [0, 59, 60, 3599, 3600, 86_399, 86_400, 9_000_000] {
            assert!(
                short_age(seconds).chars().count() <= 6,
                "{seconds} was too wide"
            );
        }
    }

    #[test]
    fn each_unit_takes_over_where_the_last_runs_out() {
        assert_eq!(short_age(59), "59 с");
        assert_eq!(short_age(60), "1 м");
        assert_eq!(short_age(3_600), "1 ч");
        assert_eq!(short_age(86_400), "1 д");
    }
}
