//! The page header: title, subtitle, and the three global actions.

use iced::widget::{button, column, container, row, text};
use iced::{Alignment, Element, Length};

use moonlight_design::typography::{scale, EMPHATIC};
use moonlight_design::{icon, Icon, Palette};

use crate::localization::{t, S};
use crate::{hspace, theme, Message, Moonlight};

pub fn view(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let page = app.page();

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

    // Ping and Refresh only mean anything once there is a subscription to
    // measure or re-fetch; offering them before is a button that answers with
    // nothing.
    let has_subscription = app.preferences().subscription_url.is_some();

    let actions = row![
        action(
            palette,
            locale,
            Icon::Activity,
            S::Ping,
            app.is_pinging(),
            has_subscription.then_some(Message::Ping)
        ),
        action(
            palette,
            locale,
            Icon::RefreshCw,
            S::Refresh,
            app.is_refreshing(),
            has_subscription.then_some(Message::Refresh)
        ),
        appearance_button(palette, app.preferences().appearance.as_deref()),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    row![titles, hspace(Length::Fill), actions]
        .align_y(Alignment::Center)
        .into()
}

fn action<'a>(
    palette: Palette,
    locale: moonlight_core::AppLocale,
    glyph: Icon,
    label: S,
    busy: bool,
    message: Option<Message>,
) -> Element<'a, Message> {
    // The glyph swaps for a loader while the action is running, rather than the
    // button being disabled with no explanation.
    let glyph = if busy { Icon::LoaderCircle } else { glyph };
    let ink = if message.is_some() {
        palette.accent_ink
    } else {
        palette.text_muted
    };

    let mut element = button(
        row![
            icon(glyph, 17.0, ink),
            text(t(label, locale))
                .size(scale::BODY_SM)
                .font(moonlight_design::ui(EMPHATIC))
                .color(if message.is_some() {
                    palette.text
                } else {
                    palette.text_muted
                }),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding([10, 16])
    .style(move |_, status| theme::ghost_button(palette, status));

    if let Some(message) = message {
        element = element.on_press(message);
    }
    element.into()
}

/// The round sun/moon button. Its glyph is what the theme *is*, not what
/// pressing it would do — a moon on a dark UI reading as "you are in dark mode"
/// is the convention every OS uses.
fn appearance_button<'a>(palette: Palette, appearance: Option<&str>) -> Element<'a, Message> {
    let glyph = match appearance {
        Some("dark") => Icon::Moon,
        Some("light") => Icon::Sun,
        // Following the system is its own state, and neither a sun nor a moon
        // says so.
        _ => Icon::Monitor,
    };

    button(container(icon(glyph, 18.0, palette.accent_ink)).center_x(Length::Fixed(22.0)))
        .on_press(Message::CycleAppearance)
        .padding(12)
        .style(move |_, status| theme::ghost_button(palette, status))
        .into()
}
