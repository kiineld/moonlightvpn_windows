//! What is going through the tunnel, as a table.
//!
//! Two levels, because the question arrives in two shapes. "Is anything of mine
//! going out unproxied" is asked of the whole machine, and is answered by one
//! row per program with its totals. "What is Chrome actually talking to" is
//! asked of one program, and is answered by its hosts. The old screen tried to
//! answer both at once by nesting every host under its process, which on a real
//! machine is eighty rows of indented text and answers neither.
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

/// The column widths. Fixed so every row lines up under its heading — the whole
/// point of a table — with the first column taking whatever is left.
const CHAIN: f32 = 190.0;
const RULE: f32 = 130.0;
const NET: f32 = 78.0;
const BYTES: f32 = 82.0;
const TIME: f32 = 56.0;
/// The leading count badge on a process row.
const BADGE: f32 = 34.0;

pub fn view(app: &Moonlight) -> Element<'_, Message> {
    match app.connection_process() {
        Some(process) => hosts(app, process),
        None => processes(app),
    }
}

// MARK: - Level one: one row per process

fn processes(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let needle = app.connection_filter().to_lowercase();

    let grouped: Vec<(String, Vec<&Connection>)> = app
        .connections_by_process()
        .into_iter()
        .filter(|(process, _)| needle.is_empty() || process.to_lowercase().contains(&needle))
        .collect();

    let empty_list = grouped.is_empty();
    let mut rows = column![].spacing(0);
    // Consumed rather than borrowed: the rows outlive this Vec, and only the
    // `&Connection`s inside it — which point into the app — may travel with them.
    for (index, (process, connections)) in grouped.into_iter().enumerate() {
        if index > 0 {
            rows = rows.push(components::soft_divider(palette));
        }
        rows = rows.push(process_row(app, process, connections));
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
        controls(app, None),
        vspace(Length::Fixed(14.0)),
        heading(palette, t(S::ColProcess, locale)),
        components::surface(body, palette),
    ]
    .height(Length::Fill)
    .into()
}

fn process_row<'a>(
    app: &'a Moonlight,
    process: String,
    connections: Vec<&'a Connection>,
) -> Element<'a, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();

    let download: i64 = connections.iter().map(|c| c.download).sum();
    let upload: i64 = connections.iter().map(|c| c.upload).sum();
    // The oldest connection is how long the program has been talking.
    let age = connections.iter().map(|c| elapsed(c)).max().unwrap_or(0);

    // The chain and rule of the busiest connection stand for the group: a
    // program's connections almost always share both, and where they do not the
    // drill-down is one press away.
    let representative = connections.first().copied();

    let count = container(
        text(format!("{}", connections.len()))
            .size(scale::MICRO)
            .font(moonlight_design::ui(EMPHATIC))
            .color(palette.accent_ink),
    )
    .center_x(Length::Fixed(BADGE))
    .center_y(Length::Fixed(22.0))
    .style(move |_| container::Style {
        background: Some(iced::Background::Color(palette.accent_quiet)),
        border: iced::Border {
            radius: iced::border::Radius::from(radii::CHIP),
            ..Default::default()
        },
        ..Default::default()
    });

    let name = row![
        count,
        app_glyph(app, &process),
        text(process.clone())
            .size(scale::BODY_SM)
            .font(moonlight_design::ui(ROW_TITLE))
            .color(palette.text),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    let content = row![
        container(name).width(Length::Fill),
        chain_cell(app, representative),
        rule_cell(palette, representative, locale),
        network_cell(palette, &connections),
        bytes_cell(palette, download, locale),
        bytes_cell(palette, upload, locale),
        time_cell(palette, age),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    button(content)
        .on_press(Message::SelectConnectionProcess(Some(process)))
        .padding([11, 14])
        .width(Length::Fill)
        .style(move |_, status| theme::row_button(palette, false, status))
        .into()
}

// MARK: - Level two: one row per host, for a single process

fn hosts<'a>(app: &'a Moonlight, process: &'a str) -> Element<'a, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let needle = app.connection_filter().to_lowercase();

    let matching: Vec<&Connection> = app
        .connections()
        .iter()
        .filter(|c| c.process == process)
        .filter(|c| needle.is_empty() || c.host.to_lowercase().contains(&needle))
        .collect();

    let mut rows = column![].spacing(0);
    for (index, connection) in matching.iter().enumerate() {
        if index > 0 {
            rows = rows.push(components::soft_divider(palette));
        }
        rows = rows.push(host_row(app, connection));
    }

    let body: Element<'_, Message> = if matching.is_empty() {
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
        controls(app, Some(process)),
        vspace(Length::Fixed(14.0)),
        heading(palette, t(S::ColHost, locale)),
        components::surface(body, palette),
    ]
    .height(Length::Fill)
    .into()
}

fn host_row<'a>(app: &'a Moonlight, connection: &'a Connection) -> Element<'a, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let one = [connection];

    let content = row![
        container(
            text(connection.host.clone())
                .font(moonlight_design::mono())
                .size(scale::META)
                .color(palette.text)
        )
        .width(Length::Fill),
        chain_cell(app, Some(connection)),
        rule_cell(palette, Some(connection), locale),
        network_cell(palette, &one),
        bytes_cell(palette, connection.download, locale),
        bytes_cell(palette, connection.upload, locale),
        time_cell(palette, elapsed(connection)),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    // Closing one reads as "move this onto the node I just picked", because the
    // core reopens whatever is still wanted.
    button(content)
        .on_press(Message::CloseConnection(connection.id.clone()))
        .padding([11, 14])
        .width(Length::Fill)
        .style(move |_, status| theme::row_button(palette, false, status))
        .into()
}

// MARK: - The shared furniture

fn scrollbar() -> scrollable::Direction {
    scrollable::Direction::Vertical(
        scrollable::Scrollbar::new()
            .width(crate::SCROLLBAR_WIDTH)
            .scroller_width(crate::SCROLLBAR_WIDTH)
            .margin(crate::SCROLLBAR_MARGIN),
    )
}

/// The controls strip: a count, a filter, and a way out.
fn controls<'a>(app: &'a Moonlight, process: Option<&'a str>) -> Element<'a, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();

    let shown = match process {
        Some(process) => app
            .connections()
            .iter()
            .filter(|c| c.process == process)
            .count(),
        None => app.connections().len(),
    };

    let mut strip = row![].spacing(10).align_y(Alignment::Center);

    // Drilled in, the strip leads with the way back and says whose list this is.
    if let Some(process) = process {
        strip = strip.push(
            button(components::centre(icon(
                Icon::ChevronLeft,
                16.0,
                palette.text2,
            )))
            .on_press(Message::SelectConnectionProcess(None))
            .width(Length::Fixed(30.0))
            .height(Length::Fixed(30.0))
            .padding(0)
            .style(move |_, status| theme::icon_button(palette, status)),
        );
        strip = strip.push(app_glyph(app, process));
        strip = strip.push(
            text(process.to_string())
                .size(scale::BODY_SM)
                .font(moonlight_design::ui(ROW_TITLE))
                .color(palette.text),
        );
    }

    strip = strip.push(components::count_pill(
        format!("{}: {shown}", t(S::ActiveConnections, locale)),
        palette,
    ));

    strip = strip.push(
        text_input(t(S::FilterText, locale), app.connection_filter())
            .on_input(Message::ConnectionFilterChanged)
            .padding([7, 12])
            .size(scale::META)
            .width(Length::Fixed(220.0))
            .style(move |_, status| theme::field(palette, status)),
    );

    strip = strip.push(hspace(Length::Fill));

    let live = shown > 0;
    let close_ink = if live {
        palette.danger
    } else {
        theme::alpha(palette.danger, 0.5)
    };
    strip = strip.push(
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
    );

    strip.into()
}

/// The column headings, in the overline the rest of the app uses for a label.
fn heading(palette: Palette, first: &str) -> Element<'_, Message> {
    let cell = move |label: &'static str, width: f32| -> Element<'_, Message> {
        container(components::overline(label, palette))
            .width(Length::Fixed(width))
            .into()
    };

    container(
        row![
            container(components::overline(first, palette)).width(Length::Fill),
            cell("ЦЕПОЧКА", CHAIN),
            cell("ПРАВИЛО", RULE),
            cell("СЕТЬ", NET),
            cell("↓ DL", BYTES),
            cell("↑ UL", BYTES),
            cell("ВРЕМЯ", TIME),
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

/// The node that carried it, with the flag of where it left through.
fn chain_cell<'a>(app: &'a Moonlight, connection: Option<&'a Connection>) -> Element<'a, Message> {
    let palette = app.palette_of();
    let Some(connection) = connection else {
        return blank(CHAIN);
    };

    let node = connection.node();
    let mut cell = row![].spacing(6).align_y(Alignment::Center);

    if let Some(handle) = app.node_region(node).and_then(|code| app.flag_image(&code)) {
        cell = cell.push(
            iced::widget::image(handle)
                .width(Length::Fixed(16.0))
                .height(Length::Fixed(12.0)),
        );
    }
    cell = cell.push(
        text(if node.is_empty() {
            "—".to_string()
        } else {
            node.to_string()
        })
        .size(scale::META)
        .color(palette.text2),
    );

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
    container(text(label).size(scale::META).color(palette.text_muted))
        .width(Length::Fixed(RULE))
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
                .size(10.5)
                .font(moonlight_design::ui(EMPHATIC))
                .color(ink),
        )
        .padding([2, 6])
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
        chips = chips.push(chip("UDP", palette.st_partial_ink));
    }

    container(chips).width(Length::Fixed(NET)).into()
}

fn bytes_cell<'a>(palette: Palette, value: i64, locale: AppLocale) -> Element<'a, Message> {
    container(
        text(format::bytes(Some(value), locale))
            .font(moonlight_design::mono())
            .size(scale::META)
            .color(palette.text2),
    )
    .width(Length::Fixed(BYTES))
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
        // Whatever the value, it has to sit in a 56pt column beside a heading.
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
