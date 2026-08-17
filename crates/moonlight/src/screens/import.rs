//! Adding a subscription.

use iced::widget::{button, column, container, row, text, text_input};
use iced::{Alignment, Element, Length};

use moonlight_design::typography::{scale, EMPHATIC};
use moonlight_design::{icon, Icon};

use crate::localization::{t, S};
use crate::{hspace, theme, Message, Moonlight, Page};

pub fn view(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();

    let field = row![
        text_input(t(S::ImportPlaceholder, locale), app.import_field())
            .on_input(Message::ImportChanged)
            .on_submit(Message::ImportSubmit)
            .padding([14, 16])
            .size(scale::BODY)
            .style(move |_, status| theme::field(palette, status)),
        button(
            text(t(S::Import, locale))
                .size(scale::BODY_SM)
                .font(moonlight_design::ui(EMPHATIC))
        )
        .on_press_maybe((!app.import_field().trim().is_empty()).then_some(Message::ImportSubmit))
        .padding([13, 22])
        .style(move |_, status| theme::accent_button(palette, status)),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    let paste = button(
        row![
            icon(Icon::Link2, 17.0, palette.text2),
            text(t(S::PasteFromClipboard, locale))
                .size(scale::BODY_SM)
                .font(moonlight_design::ui(EMPHATIC))
                .color(palette.text),
        ]
        .spacing(9)
        .align_y(Alignment::Center),
    )
    .on_press(Message::PasteFromClipboard)
    .padding([14, 18])
    .width(Length::Fill)
    .style(move |_, status| theme::header_button(palette, status));

    // The link field, and pasting one. No "open the bot" row: the macOS client
    // does not carry one either, and a button that leaves for a browser is a
    // detour on the one screen whose whole job is to accept a link the user
    // already has.
    let mut content = column![
        text(t(S::ImportHelp, locale))
            .size(scale::BODY)
            .color(palette.text2),
        field,
        paste,
    ]
    .spacing(16);

    if let Some(error) = app.last_error() {
        content = content.push(
            text(error.to_string())
                .size(scale::BODY_SM)
                .color(palette.danger),
        );
    }

    // Only offered once there is something to go back to.
    if app.preferences().subscription_url.is_some() {
        content = content.push(
            button(
                text(t(S::BackToSubscription, locale))
                    .size(scale::BODY_SM)
                    .font(moonlight_design::ui(EMPHATIC))
                    .color(palette.text_link),
            )
            .on_press(Message::Navigate(Page::Subscription))
            .padding([8, 4])
            .style(move |_, status| theme::nav_button(palette, status)),
        );
    }

    container(row![
        hspace(Length::FillPortion(1)),
        container(content).width(Length::FillPortion(8)),
        hspace(Length::FillPortion(1)),
    ])
    .width(Length::Fill)
    .into()
}
