//! What is going through the tunnel, grouped by the program that opened it.
//!
//! Grouping by process is the question people actually bring to this screen: is
//! *this program* going through the tunnel. `find-process-mode` is therefore
//! always on in the generated config — without it every row reads "—".

use iced::widget::{button, column, row, text};
use iced::{Alignment, Element, Length};

use moonlight_core::format;
use moonlight_design::typography::{scale, EMPHATIC};
use moonlight_design::{icon, Icon};

use crate::components;
use crate::localization::{t, S};
use crate::{hspace, theme, vspace, Message, Moonlight};

pub fn view(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let grouped = app.connections_by_process();

    let head = row![
        components::overline(t(S::NavConnections, locale), palette),
        hspace(Length::Fill),
        text(format!("{}", app.connections().len()))
            .size(scale::META)
            .color(palette.text_muted),
        hspace(Length::Fixed(12.0)),
        button(
            text(t(S::CloseAll, locale))
                .size(scale::BODY_SM)
                .font(moonlight_design::ui(EMPHATIC))
        )
        .on_press_maybe((!grouped.is_empty()).then_some(Message::CloseAllConnections))
        .padding([9, 16])
        .style(move |_, status| theme::header_button(palette, status)),
    ]
    .align_y(Alignment::Center);

    let mut list = column![].spacing(2);
    if grouped.is_empty() {
        list = list.push(components::empty_state(
            t(S::NoConnections, locale),
            palette,
        ));
    } else {
        for (process, connections) in grouped {
            let total_up: i64 = connections.iter().map(|c| c.upload).sum();
            let total_down: i64 = connections.iter().map(|c| c.download).sum();

            list = list.push(
                row![
                    icon(Icon::Monitor, 18.0, palette.text2),
                    column![
                        text(process.clone())
                            .size(scale::BODY)
                            .font(moonlight_design::ui(EMPHATIC))
                            .color(palette.text),
                        text(format!(
                            "{} · ↑ {} ↓ {}",
                            connections.len(),
                            format::bytes(Some(total_up), locale),
                            format::bytes(Some(total_down), locale)
                        ))
                        .size(scale::META)
                        .color(palette.text_muted),
                    ]
                    .spacing(1),
                ]
                .spacing(12)
                .padding([10, 12])
                .align_y(Alignment::Center),
            );

            // The hosts behind the process, each with the node that carried it
            // and the rule that chose. Closing one reads as "move this onto the
            // node I just picked", because the core reopens what is still
            // wanted.
            for connection in connections {
                list = list.push(
                    row![
                        hspace(Length::Fixed(30.0)),
                        column![
                            text(connection.host.clone())
                                .font(moonlight_design::mono())
                                .size(scale::META)
                                .color(palette.text2),
                            text(format!(
                                "{} · {} · {}",
                                connection.network,
                                connection.node(),
                                if connection.rule.is_empty() {
                                    t(S::Unknown, locale)
                                } else {
                                    connection.rule.as_str()
                                }
                            ))
                            .size(scale::MICRO)
                            .color(palette.text_muted),
                        ]
                        .spacing(1),
                        hspace(Length::Fill),
                        button(icon(Icon::X, 14.0, palette.text_muted))
                            .on_press(Message::CloseConnection(connection.id.clone()))
                            .padding(7)
                            .style(move |_, status| theme::nav_button(palette, status)),
                    ]
                    .spacing(8)
                    .padding([4, 12])
                    .align_y(Alignment::Center),
                );
            }
            list = list.push(components::divider(palette));
        }
    }

    column![
        head,
        vspace(Length::Fixed(14.0)),
        components::surface(list, palette),
    ]
    .into()
}
