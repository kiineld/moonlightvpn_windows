//! The question a `moonlight://` link asks before it adds anything.
//!
//! A link never adds a subscription by itself: any page can open one, and a
//! subscription added unseen would route the machine through whoever wrote the
//! page. So the app comes forward and asks, saying whether the link would
//! replace the current subscription or update it — and never shows the link.

use iced::widget::{button, canvas, column, container, opaque, row, text};
use iced::{Alignment, Background, Border, Color, Element, Length};

use moonlight_core::subscription;
use moonlight_design::motion::radii;
use moonlight_design::typography::EMPHATIC;

use crate::localization::{self, t, S};
use crate::logo::Logo;
use crate::{components, hspace, theme, LinkPrompt, Message, Moonlight};

/// A button along the foot: its label, what it does (none while busy), and
/// whether it is the one to press.
type Action = (S, Option<Message>, bool);

pub fn view<'a>(app: &'a Moonlight, prompt: &'a LinkPrompt) -> Element<'a, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();

    let (title, body, note, actions): (S, String, Option<S>, Vec<Action>) = match prompt {
        LinkPrompt::Ask(link) => (
            S::LinkTitle,
            t(S::LinkBody, locale).to_string(),
            replacement_note(app, link),
            vec![
                (S::Cancel, Some(Message::LinkDismiss), false),
                (S::LinkAdd, Some(Message::LinkAdd), true),
            ],
        ),
        // What is happening and for how long, and a way to put the dialog
        // away. It had neither, and a server slow to answer left a spinner
        // on top of the whole window with nothing to do but wait. Hidden, the
        // loading goes on, and ends where a refresh's result always shows.
        LinkPrompt::Adding(_) => (
            S::LinkTitle,
            match app.link_waited() {
                0 | 1 => t(S::LinkAdding, locale).to_string(),
                seconds => format!(
                    "{} {seconds} {}",
                    t(S::LinkAdding, locale),
                    t(S::SecondsShort, locale)
                ),
            },
            None,
            vec![(S::LinkHide, Some(Message::LinkDismiss), false)],
        ),
        LinkPrompt::Failed(_, issue) => (
            S::LinkFailedTitle,
            localization::issue(issue, locale),
            None,
            vec![
                (S::LinkClose, Some(Message::LinkDismiss), false),
                (S::LinkRetry, Some(Message::LinkAdd), true),
            ],
        ),
        LinkPrompt::Invalid => (
            S::LinkInvalidTitle,
            t(S::LinkInvalidBody, locale).to_string(),
            None,
            vec![(S::LinkClose, Some(Message::LinkDismiss), true)],
        ),
    };

    // Every line fills the card and centres itself in it. Left to its own
    // width, the block hugged the card's left edge whenever its widest line was
    // shorter than the card — plain to see on "Загружаем подписку…".
    let line =
        |content: iced::widget::Text<'a>| content.width(Length::Fill).align_x(Alignment::Center);
    let mut words = column![
        line(
            text(t(title, locale))
                .font(moonlight_design::display())
                .size(24.0)
                .color(palette.text)
        ),
        line(text(body).size(14.0).color(palette.text_muted)),
    ]
    .spacing(10)
    .width(Length::Fill);
    if let Some(note) = note {
        words = words.push(line(
            text(t(note, locale))
                .size(14.0)
                .font(moonlight_design::ui(EMPHATIC))
                .color(palette.text),
        ));
    }

    let has_actions = !actions.is_empty();
    let mut buttons = row![hspace(Length::Fill)]
        .spacing(10)
        .align_y(Alignment::Center);
    for (label, message, primary) in actions {
        let label = text(t(label, locale))
            .size(14.5)
            .font(moonlight_design::ui(EMPHATIC));
        let press = button(label).on_press_maybe(message).padding([11, 22]);
        buttons = buttons.push(if primary {
            press.style(move |_, status| theme::accent_button(palette, status))
        } else {
            press.style(move |_, status| theme::header_button(palette, status))
        });
    }

    let mut content = column![column![
        canvas(Logo::with_radius(palette, 20.0))
            .width(Length::Fixed(72.0))
            .height(Length::Fixed(72.0)),
        words,
    ]
    .spacing(18)
    .width(Length::Fill)
    .align_x(Alignment::Center)
    .padding([32, 32])]
    .width(Length::Fill);
    // While it loads there is nothing to press, and an empty strip under a
    // rule reads as something missing.
    if has_actions {
        content = content
            .push(components::divider(palette))
            .push(container(buttons).padding([14, 20]));
    }

    let card = container(content)
        .width(Length::Fixed(460.0))
        .style(move |_| container::Style {
            background: Some(Background::Color(palette.surface)),
            border: Border {
                radius: iced::border::Radius::from(radii::PANEL),
                width: 1.0,
                color: palette.hairline,
            },
            ..Default::default()
        });

    // Opaque, so nothing under the question can be clicked while it is asked.
    opaque(
        container(card)
            .center(Length::Fill)
            .style(|_| container::Style {
                background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.5))),
                ..Default::default()
            }),
    )
}

/// Whether adding would replace the subscription in use, or refresh it.
fn replacement_note(app: &Moonlight, link: &str) -> Option<S> {
    let current = app.preferences().subscription_url.as_deref()?;
    Some(
        if subscription::normalize(current) == subscription::normalize(link) {
            S::LinkSame
        } else {
            S::LinkReplaces
        },
    )
}
