//! The core's log and the app's own narration, on one timeline.
//!
//! That pairing is the point: read apart, a failed connect is a core error with
//! no cause; together it is "the app switched to TUN, then the core could not
//! take the route".

use iced::widget::{button, column, row, text, text_input};
use iced::{Alignment, Element, Length};

use moonlight_core::controller::{level_rank, LogSource};
use moonlight_design::typography::{scale, EMPHATIC};

use crate::components;
use crate::localization::{t, S};
use crate::{hspace, theme, vspace, Message, Moonlight};

pub fn view(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let needle = app.log_filter().to_lowercase();

    // The level is a floor, not an exact match: WARN means warnings and errors.
    let levels: [(u8, &str); 4] = [(0, "DEBUG"), (1, "INFO"), (2, "WARN"), (3, "ERROR")];

    let controls = row![
        components::segmented(&levels, app.log_level(), Message::LogFilterLevel, palette),
        hspace(Length::Fixed(12.0)),
        text_input(t(S::FilterText, locale), app.log_filter())
            .on_input(Message::LogFilterText)
            .padding([9, 14])
            .size(scale::BODY_SM)
            .width(Length::Fixed(260.0))
            .style(move |_, status| theme::field(palette, status)),
        hspace(Length::Fill),
        button(
            text(t(S::ClearLogs, locale))
                .size(scale::BODY_SM)
                .font(moonlight_design::ui(EMPHATIC))
        )
        .on_press(Message::ClearLogs)
        .padding([9, 16])
        .style(move |_, status| theme::ghost_button(palette, status)),
    ]
    .align_y(Alignment::Center);

    let matching: Vec<_> = app
        .logs()
        .iter()
        .filter(|entry| passes(&entry.level, &entry.message, app.log_level(), &needle))
        .collect();

    let mut list = column![].spacing(1);
    if matching.is_empty() {
        list = list.push(components::empty_state(t(S::NoLogs, locale), palette));
    } else {
        // Newest last, and the scrollable starts at the top — a log read from
        // the beginning is how you find the line before the failure.
        for entry in matching {
            let ink = match level_rank(&entry.level) {
                3 => palette.danger,
                2 => palette.warning,
                0 => palette.text_muted,
                _ => palette.text2,
            };
            list = list.push(
                row![
                    // The source, so the two timelines can be told apart at a
                    // glance without reading the message.
                    text(match entry.source {
                        LogSource::Core => "core",
                        LogSource::App => "app",
                    })
                    .font(moonlight_design::mono())
                    .size(scale::MICRO)
                    .color(palette.text_muted)
                    .width(Length::Fixed(38.0)),
                    text(entry.level.clone())
                        .font(moonlight_design::mono())
                        .size(scale::MICRO)
                        .color(ink)
                        .width(Length::Fixed(58.0)),
                    text(entry.message.clone())
                        .font(moonlight_design::mono())
                        .size(scale::META)
                        .color(palette.text),
                ]
                .spacing(8)
                .padding([3, 8]),
            );
        }
    }

    column![
        controls,
        vspace(Length::Fixed(14.0)),
        components::surface(list, palette),
    ]
    .into()
}

/// Exposed so the tests can assert the filter without building a widget tree.
pub fn passes(level: &str, message: &str, floor: u8, needle: &str) -> bool {
    level_rank(level) >= floor
        && (needle.is_empty() || message.to_lowercase().contains(&needle.to_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_level_filter_is_a_floor_not_an_exact_match() {
        // Choosing WARN must not hide the errors, which are the lines you were
        // looking for.
        assert!(passes("ERROR", "x", 2, ""));
        assert!(passes("WARNING", "x", 2, ""));
        assert!(!passes("INFO", "x", 2, ""));
    }

    #[test]
    fn the_text_filter_is_case_insensitive() {
        assert!(passes("INFO", "Start TUN listening error", 0, "tun"));
        assert!(passes("INFO", "start tun listening", 0, "TUN"));
        assert!(!passes("INFO", "nothing here", 0, "tun"));
    }

    #[test]
    fn an_empty_filter_keeps_everything_at_that_level() {
        assert!(passes("INFO", "anything", 1, ""));
    }

    #[test]
    fn the_cores_own_level_spellings_pass_the_floor() {
        // mihomo writes ERRO and WARN, not ERROR and WARNING.
        assert!(passes("ERRO", "boom", 3, ""));
        assert!(passes("WARN", "slow", 2, ""));
    }
}
