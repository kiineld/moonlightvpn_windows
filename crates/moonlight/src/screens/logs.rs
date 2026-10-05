//! The core's log and the app's own narration, on one timeline.
//!
//! That pairing is the point: read apart, a failed connect is a core error with
//! no cause; together it is "the app switched to TUN, then the core could not
//! take the route".

use iced::widget::{button, column, container, row, scrollable, text, text_input};
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

    // Which timeline. "Обе" is the default and its own option rather than a
    // cleared filter: the two read together is the whole reason they share a
    // list, so the combined view has to be reachable in one press.
    let sources: [(LogFilter, &str); 3] = [
        (LogFilter::Both, t(S::LogBoth, locale)),
        (LogFilter::App, t(S::LogClient, locale)),
        (LogFilter::Core, t(S::LogCore, locale)),
    ];

    let controls = row![
        components::segmented(
            &sources,
            app.log_source(),
            Message::LogFilterSource,
            palette
        ),
        hspace(Length::Fixed(12.0)),
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
        .style(move |_, status| theme::header_button(palette, status)),
    ]
    .align_y(Alignment::Center);

    let matching: Vec<_> = app
        .logs()
        .iter()
        .filter(|entry| {
            app.log_source().accepts(entry.source)
                && passes(&entry.level, &entry.message, app.log_level(), &needle)
        })
        .collect();

    // ponytail: the newest SHOWN lines only. Rows wrap, so they are not one
    // height and the list cannot build just the ones in view; two thousand of
    // them laid out on every arriving line is what made this page drag. The
    // filters reach the older ones. Give rows one height if all of them ever
    // have to scroll.
    const SHOWN: usize = 400;
    let older = matching.len().saturating_sub(SHOWN);

    let mut list = column![].spacing(0);
    if matching.is_empty() {
        list = list.push(components::empty_state(t(S::NoLogs, locale), palette));
    } else {
        // Newest last, and the list is kept at its end: what just happened is
        // what a log is opened for. Scrolling back stops it following, and
        // returning to the end starts it again — see `Message::LogScrolled`.
        for entry in matching.into_iter().skip(older) {
            let rank = level_rank(&entry.level);
            let (level, ink) = match rank {
                3 => ("ERROR", palette.danger),
                2 => ("WARN", palette.warning),
                0 => ("DEBUG", palette.text_muted),
                _ => ("INFO", palette.text_muted),
            };
            // Only what went wrong is set in the text colour's full strength
            // or a signal's; a page of ordinary lines is quiet, so the eye
            // lands on the one that is not.
            let said = if rank >= 2 {
                palette.text
            } else {
                palette.text2
            };
            let label = |content: &'static str, tone, width| {
                text(content)
                    .font(moonlight_design::mono())
                    .size(LABEL)
                    .line_height(LINE)
                    .color(tone)
                    .width(Length::Fixed(width))
            };
            list = list.push(
                row![
                    // When, in local time: without it a line said what
                    // happened and nothing about how long the step before took.
                    text(clock(entry.at))
                        .font(moonlight_design::mono())
                        .size(LABEL)
                        .line_height(LINE)
                        .color(palette.text_muted)
                        .width(Length::Fixed(62.0)),
                    // The source, so the two timelines can be told apart at a
                    // glance without reading the message.
                    label(
                        match entry.source {
                            LogSource::Core => "core",
                            LogSource::App => "app",
                        },
                        palette.text_muted,
                        34.0
                    ),
                    label(level, ink, 44.0),
                    text(entry.message.clone())
                        .font(moonlight_design::mono())
                        .size(SAID)
                        .line_height(LINE)
                        .color(said)
                        .width(Length::Fill),
                ]
                .spacing(12)
                .padding([4, 4]),
            );
        }
    }

    // The list scrolls on its own inside its panel, under controls that stay
    // put, so there is one place — its end — to keep it at.
    let list = scrollable(list.padding(iced::Padding {
        right: crate::SCROLLBAR_GUTTER,
        ..iced::Padding::ZERO
    }))
    .id(crate::LOG_SCROLL)
    .on_scroll(Message::LogScrolled)
    .direction(scrollable::Direction::Vertical(
        scrollable::Scrollbar::new()
            .width(crate::SCROLLBAR_WIDTH)
            .scroller_width(crate::SCROLLBAR_WIDTH)
            .margin(crate::SCROLLBAR_MARGIN),
    ))
    .width(Length::Fill)
    .height(Length::Fill)
    .style(move |theme, _| theme::scroller(palette, theme));

    column![
        controls,
        vspace(Length::Fixed(14.0)),
        container(list)
            .padding(components::SURFACE_PADDING)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| theme::panel(palette)),
    ]
    .height(Length::Fill)
    .into()
}

/// The log's type: the message a step above its three labels, and all four on
/// one line height so a row's columns share a baseline.
const SAID: f32 = 12.5;
const LABEL: f32 = 11.5;
const LINE: iced::widget::text::LineHeight =
    iced::widget::text::LineHeight::Absolute(iced::Pixels(19.0));

/// `HH:MM:SS` in the machine's own time zone.
fn clock(unix: i64) -> String {
    let Ok(utc) = time::OffsetDateTime::from_unix_timestamp(unix) else {
        return String::new();
    };
    let at = time::UtcOffset::current_local_offset().map_or(utc, |offset| utc.to_offset(offset));
    format!("{:02}:{:02}:{:02}", at.hour(), at.minute(), at.second())
}

/// Which of the two timelines the list is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogFilter {
    /// Both, interleaved — the default, and the reason they share a list.
    #[default]
    Both,
    /// The app's own narration.
    App,
    /// mihomo's log.
    Core,
}

impl LogFilter {
    pub fn accepts(self, source: LogSource) -> bool {
        match self {
            LogFilter::Both => true,
            LogFilter::App => source == LogSource::App,
            LogFilter::Core => source == LogSource::Core,
        }
    }
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

    #[test]
    fn both_is_the_default_and_hides_neither_timeline() {
        // Read apart, a failed connect is a core error with no cause.
        let filter = LogFilter::default();
        assert_eq!(filter, LogFilter::Both);
        assert!(filter.accepts(LogSource::App));
        assert!(filter.accepts(LogSource::Core));
    }

    #[test]
    fn each_source_admits_only_its_own_lines() {
        assert!(LogFilter::App.accepts(LogSource::App));
        assert!(!LogFilter::App.accepts(LogSource::Core));
        assert!(LogFilter::Core.accepts(LogSource::Core));
        assert!(!LogFilter::Core.accepts(LogSource::App));
    }
}
