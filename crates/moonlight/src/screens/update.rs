//! The corner banner a new version gets when the app opens.
//!
//! The launch check is quiet: this is the only thing it ever shows. Clicking
//! it opens Settings and starts the install there, with its progress in view;
//! its cross puts it away until the next launch.

use iced::widget::{button, canvas, column, container, row, text};
use iced::{Alignment, Element, Length};

use moonlight_design::typography::EMPHATIC;
use moonlight_design::{icon, Icon};

use crate::localization::{t, S};
use crate::logo::Logo;
use crate::{theme, Message, Moonlight};

pub fn banner<'a>(app: &'a Moonlight, version: &'a str) -> Element<'a, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();

    let open = button(
        row![
            canvas(Logo::with_radius(palette, 11.0))
                .width(Length::Fixed(40.0))
                .height(Length::Fixed(40.0)),
            column![
                text(t(S::UpdateAvailable, locale))
                    .size(14.5)
                    .font(moonlight_design::ui(EMPHATIC))
                    .color(palette.text),
                text(t(S::UpdateAvailableSub, locale).replace("{version}", version))
                    .size(12.5)
                    .color(palette.text_muted),
            ]
            .spacing(2),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .on_press(Message::OpenUpdate)
    .padding([8, 10])
    .style(move |_, status| theme::row_button(palette, false, status));

    let close = button(icon(Icon::X, 14.0, palette.text_muted))
        .on_press(Message::HideUpdateBanner)
        .padding(8)
        .style(move |_, status| theme::row_button(palette, false, status));

    container(row![open, close].spacing(4).align_y(Alignment::Center))
        .padding(6)
        .style(move |_| theme::panel(palette))
        .into()
}
