//! The page header: title, subtitle, and the three global actions.
//!
//! 64px tall with a soft hairline under it, from the desktop composition — the
//! `--ml-header-height` token says 56, but the composition that ships sets 64,
//! and the composition wins.

use iced::widget::{button, column, container, row, text};
use iced::{Alignment, Background, Element, Length};

use moonlight_core::AppLocale;
use moonlight_design::motion::{metrics, radii};
use moonlight_design::typography::{line, scale, EMPHATIC};
use moonlight_design::{icon, Icon, Palette};

use crate::localization::{t, S};
use crate::{hspace, theme, Message, Moonlight};

/// The header's own type step. Smaller than `--ml-t-title`: the page title sits
/// under a window title bar here, and 24px competes with it.
const TITLE: f32 = 20.0;

pub fn view(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let page = app.page();

    let titles = column![
        text(t(page.title(), locale))
            .font(moonlight_design::display())
            .size(TITLE)
            .line_height(line::TITLE)
            .color(palette.text),
        text(t(page.subtitle(), locale))
            .size(scale::META)
            .color(palette.text_muted),
    ]
    .spacing(3);

    // Ping and Refresh only mean anything once there is a subscription to
    // measure or re-fetch; offering them before is a button that answers with
    // nothing.
    let has_subscription = app.preferences().subscription_url.is_some();

    let actions = row![
        action(
            palette,
            locale,
            Icon::Activity,
            if app.is_pinging() {
                S::Measuring
            } else {
                S::Ping
            },
            app.is_pinging(),
            has_subscription.then_some(Message::Ping),
        ),
        action(
            palette,
            locale,
            Icon::RefreshCw,
            if app.is_refreshing() {
                S::Refreshing
            } else {
                S::Refresh
            },
            app.is_refreshing(),
            has_subscription.then_some(Message::Refresh),
        ),
        appearance_button(palette, app.preferences().appearance.as_deref()),
    ]
    .spacing(12)
    .align_y(Alignment::Center);

    container(
        row![titles, hspace(Length::Fill), actions]
            .spacing(12)
            .align_y(Alignment::Center),
    )
    .height(Length::Fixed(metrics::HEADER))
    .padding([0, 24])
    .style(move |_| container::Style {
        // A soft hairline, not the full one: it separates the header from the
        // page rather than drawing a box around it.
        border: iced::Border {
            width: 0.0,
            ..Default::default()
        },
        background: None,
        ..container::Style {
            background: Some(Background::Color(iced::Color::TRANSPARENT)),
            ..Default::default()
        }
    })
    .into()
}

fn action<'a>(
    palette: Palette,
    locale: AppLocale,
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
            // 2.2 rather than lucide's 2.0: at 16px the composition thickens
            // these two glyphs so they hold their weight beside 800 type.
            moonlight_design::icon_thin(glyph, 16.0, ink, 2.2),
            text(t(label, locale))
                .size(13.0)
                .font(moonlight_design::ui(EMPHATIC))
                .color(ink),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .height(Length::Fixed(metrics::CONTROL_SM))
    .padding([0, 15])
    .style(move |_, status| theme::header_button(palette, status));

    if let Some(message) = message {
        element = element.on_press(message);
    }
    element.into()
}

/// The round theme button. Its glyph is what pressing it moves *towards*, which
/// is the convention the composition follows: a sun on the dark theme.
fn appearance_button<'a>(palette: Palette, appearance: Option<&str>) -> Element<'a, Message> {
    let glyph = match appearance {
        Some("dark") => Icon::Sun,
        Some("light") => Icon::Moon,
        // Following the system is its own state, and neither a sun nor a moon
        // says so.
        _ => Icon::Monitor,
    };

    button(container(icon(glyph, 17.0, palette.text2)).center(Length::Fill))
        .width(Length::Fixed(metrics::CONTROL_SM))
        .height(Length::Fixed(metrics::CONTROL_SM))
        .padding(0)
        .on_press(Message::CycleAppearance)
        .style(move |_, status| theme::icon_button(palette, status))
        .into()
}

/// The header's hairline, drawn by the shell so it spans the full content width
/// rather than stopping at the header's padding.
pub fn rule<'a>(palette: Palette) -> Element<'a, Message> {
    container(crate::vspace(Length::Fixed(1.0)))
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(palette.hairline_soft)),
            border: iced::Border {
                radius: iced::border::Radius::from(radii::PILL),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}
