//! The page header: title, subtitle and the two global actions.

use iced::widget::{button, column, row, text};
use iced::{Alignment, Element, Length};

use moonlight_core::AppLocale;
use moonlight_design::typography::scale;
use moonlight_design::{icon, Icon, Palette};

use crate::localization::{t, S};
use crate::{hspace, theme, Message, Page};

pub fn view<'a>(palette: Palette, locale: AppLocale, page: Page) -> Element<'a, Message> {
    let titles = column![
        text(t(page.title(), locale))
            .font(moonlight_design::display())
            .size(scale::TITLE)
            .color(palette.text),
        text(t(page.subtitle(), locale))
            .size(scale::BODY_SM)
            .color(palette.text2),
    ]
    .spacing(2);

    let actions = row![
        action(palette, locale, Icon::Activity, S::Ping),
        action(palette, locale, Icon::RefreshCw, S::Refresh),
    ]
    .spacing(10);

    row![titles, hspace(Length::Fill), actions]
        .align_y(Alignment::Center)
        .into()
}

fn action<'a>(palette: Palette, locale: AppLocale, glyph: Icon, label: S) -> Element<'a, Message> {
    button(
        row![
            icon(glyph, 17.0, palette.accent_ink),
            text(t(label, locale))
                .size(scale::BODY_SM)
                .font(moonlight_design::ui(moonlight_design::typography::EMPHATIC))
                .color(palette.text),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding([10, 16])
    .style(move |_, status| theme::ghost_button(palette, status))
    .into()
}
