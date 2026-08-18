//! Adding a subscription.

use iced::widget::{button, column, container, row, text, text_input};
use iced::{Alignment, Element, Length};

use moonlight_core::format;
use moonlight_design::typography::{scale, EMPHATIC};
use moonlight_design::{icon, icon_thin, Icon};

use crate::components;
use crate::localization::{t, S};
use crate::{hspace, theme, vspace, Message, Moonlight, Page};

pub fn view(app: &Moonlight) -> Element<'_, Message> {
    if app.import_done() {
        return activated(app);
    }
    form(app)
}

/// The confirmation, which is the screen's whole reward: a mark, what was
/// actually bought, and the one thing left to do with it.
fn activated(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let info = app.info();

    // «Луна» · 12 дней · 7 узлов · 100 ГБ — the plan, then the three numbers
    // that decide whether it was the right one. Each is dropped when the panel
    // does not report it rather than shown as a dash.
    let mut facts: Vec<String> = Vec::new();
    if let Some(title) = info.title.as_deref() {
        facts.push(format!("«{title}»"));
    }
    facts.push(format::time_left(info.expire, locale));
    let nodes = app.nodes().iter().filter(|n| !n.is_auto_picker()).count();
    if nodes > 0 {
        facts.push(format!("{nodes} {}", t(S::Nodes, locale)));
    }
    if let Some(total) = info.total {
        facts.push(format::bytes(Some(total), locale));
    }

    let mark = container(icon_thin(Icon::Check, 40.0, palette.text_on_accent, 2.6))
        .center(Length::Fixed(88.0))
        .style(move |_| container::Style {
            background: Some(iced::Background::Color(palette.accent)),
            border: iced::Border {
                radius: iced::border::Radius::from(44.0),
                ..Default::default()
            },
            ..Default::default()
        });

    let content = column![
        mark,
        vspace(Length::Fixed(16.0)),
        text(t(S::SubscriptionActivated, locale))
            .font(moonlight_design::display())
            .size(28.0)
            .align_x(Alignment::Center)
            .color(palette.text),
        text(facts.join(" · "))
            .size(scale::BODY_SM)
            .align_x(Alignment::Center)
            .color(palette.text2),
        vspace(Length::Fixed(14.0)),
        button(components::centre(
            text(t(S::ConnectNow, locale))
                .size(scale::BODY)
                .font(moonlight_design::ui(EMPHATIC))
                .color(palette.text_on_accent)
        ))
        .on_press(Message::Navigate(Page::Connect))
        .padding([0, 32])
        .height(Length::Fixed(50.0))
        .style(move |_, status| theme::accent_button(palette, status)),
    ]
    .spacing(10)
    .align_x(Alignment::Center);

    container(content)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}

fn form(app: &Moonlight) -> Element<'_, Message> {
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
