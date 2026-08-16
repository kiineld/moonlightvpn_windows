//! The screens that are not written yet.
//!
//! Deliberately explicit rather than an empty page: a blank screen reads as a
//! bug, and a user who has just installed this needs to know the difference
//! between "nothing here yet" and "this broke".

use iced::widget::{column, container, text};
use iced::{Alignment, Element, Length};

use moonlight_core::AppLocale;
use moonlight_design::typography::scale;
use moonlight_design::{icon, Palette};

use crate::localization::t;
use crate::{theme, Message, Page};

pub fn view<'a>(palette: Palette, locale: AppLocale, page: Page) -> Element<'a, Message> {
    let content = column![
        icon(page.icon(), 32.0, palette.text_muted),
        text(t(page.title(), locale))
            .font(moonlight_design::display())
            .size(scale::LEAD)
            .color(palette.text),
        text(match locale {
            AppLocale::Ru => "Этот экран ещё не перенесён",
            AppLocale::En => "This screen has not been ported yet",
        })
        .size(scale::BODY_SM)
        .color(palette.text_muted),
    ]
    .spacing(10)
    .align_x(Alignment::Center);

    container(content)
        .center_x(Length::Fill)
        .center_y(Length::Fixed(360.0))
        .style(move |_| theme::panel(palette))
        .into()
}
