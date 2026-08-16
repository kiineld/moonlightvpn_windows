//! The sidebar, and the quota block at its foot.
//!
//! It collapses to a 72pt icon rail; the wordmark is the toggle.

use iced::widget::{button, column, container, progress_bar, row, text};
use iced::{Alignment, Element, Length};

use moonlight_core::preferences::Preferences;
use moonlight_core::{format, AppLocale};
use moonlight_design::motion::radii;
use moonlight_design::typography::scale;
use moonlight_design::{icon, Icon, Palette};

use crate::localization::{t, S};
use crate::{hspace, theme, vspace, Message, Page};

/// The collapsed rail's width, from the design.
const RAIL: f32 = 72.0;
const EXPANDED: f32 = 248.0;

pub fn view<'a>(
    palette: Palette,
    locale: AppLocale,
    current: Page,
    collapsed: bool,
    preferences: &'a Preferences,
) -> Element<'a, Message> {
    let width = if collapsed { RAIL } else { EXPANDED };

    let mut items = column![].spacing(6);
    for page in Page::SIDEBAR {
        items = items.push(nav_item(palette, locale, page, current, collapsed));
    }

    let content = column![
        wordmark(palette, collapsed),
        vspace(Length::Fixed(18.0)),
        items,
        vspace(Length::Fill),
        quota(palette, locale, collapsed, preferences),
    ]
    .padding(14)
    .width(Length::Fixed(width));

    container(content)
        .width(Length::Fixed(width))
        .height(Length::Fill)
        .style(move |_| container::Style {
            background: Some(iced::Background::Color(palette.bg_deep)),
            ..Default::default()
        })
        .into()
}

/// The wordmark doubles as the collapse toggle, which is why it is a button
/// rather than a label with a chevron beside it.
fn wordmark<'a>(palette: Palette, collapsed: bool) -> Element<'a, Message> {
    let glyph = if collapsed {
        Icon::PanelLeftOpen
    } else {
        Icon::PanelLeftClose
    };

    let inner: Element<'a, Message> = if collapsed {
        icon(glyph, 20.0, palette.text2)
    } else {
        row![
            text("moonlight")
                .font(moonlight_design::display())
                .size(scale::LEAD)
                .color(palette.text),
            hspace(Length::Fill),
            icon(glyph, 18.0, palette.text_muted),
        ]
        .align_y(Alignment::Center)
        .into()
    };

    button(inner)
        .on_press(Message::ToggleSidebar)
        .padding(10)
        .width(Length::Fill)
        .style(move |_, status| theme::nav_button(palette, status))
        .into()
}

fn nav_item<'a>(
    palette: Palette,
    locale: AppLocale,
    page: Page,
    current: Page,
    collapsed: bool,
) -> Element<'a, Message> {
    let selected = page == current;
    // On the accent fill the glyph takes ink, not the accent — the same rule
    // the type follows.
    let ink = if selected {
        palette.text_on_accent
    } else {
        palette.text2
    };

    let inner: Element<'a, Message> = if collapsed {
        icon(page.icon(), 20.0, ink)
    } else {
        row![
            icon(page.icon(), 20.0, ink),
            text(t(page.title(), locale))
                .size(scale::BODY)
                .font(moonlight_design::ui(moonlight_design::typography::EMPHATIC))
                .color(ink),
        ]
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
    };

    button(inner)
        .on_press(Message::Navigate(page))
        .padding([12, 14])
        .width(Length::Fill)
        .style(move |_, status| {
            if selected {
                theme::accent_button(palette, status)
            } else {
                theme::nav_button(palette, status)
            }
        })
        .into()
}

/// The quota block. A partial fill is the point here, which is exactly why the
/// connect dial does not carry it.
fn quota<'a>(
    palette: Palette,
    locale: AppLocale,
    collapsed: bool,
    preferences: &'a Preferences,
) -> Element<'a, Message> {
    if collapsed {
        return vspace(Length::Fixed(0.0)).into();
    }
    if preferences.subscription_url.is_none() {
        return container(
            text(t(S::NoSubscription, locale))
                .size(scale::META)
                .color(palette.text_muted),
        )
        .padding(14)
        .into();
    }

    let content = column![
        text(t(S::Remaining, locale))
            .size(scale::MICRO)
            .color(palette.text_muted),
        text(format::days(None, locale))
            .font(moonlight_design::display())
            .size(scale::PLAN)
            .color(palette.text),
        progress_bar(0.0..=1.0, 0.0).girth(6.0),
    ]
    .spacing(6);

    container(content)
        .padding(14)
        .width(Length::Fill)
        .style(move |_| iced::widget::container::Style {
            background: Some(iced::Background::Color(palette.surface)),
            border: iced::Border {
                radius: iced::border::Radius::from(radii::CARD),
                width: 1.0,
                color: palette.hairline,
            },
            ..Default::default()
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_collapsed_rail_is_the_width_the_design_specifies() {
        assert_eq!(RAIL, 72.0);
    }

    #[test]
    fn the_quota_block_is_hidden_on_the_rail() {
        // 72pt has no room for a plan name and a bar, and half of one reads as
        // a clipped layout rather than as a deliberate collapse.
        let preferences = Preferences::default();
        let element = quota(Palette::DARK, AppLocale::Ru, true, &preferences);
        // A zero-height spacer is what "nothing here" looks like in iced.
        assert_eq!(element.as_widget().size().height, iced::Length::Fixed(0.0));
    }
}
