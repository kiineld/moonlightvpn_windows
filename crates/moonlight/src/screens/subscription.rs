//! The subscription screen: the plan card, the traffic card, and the actions.

use iced::widget::{column, container, row, text};
use iced::{Alignment, Background, Border, Element, Length};

use moonlight_core::format;
use moonlight_design::motion::radii;
use moonlight_design::typography::{scale, EMPHATIC};
use moonlight_design::Icon;

use crate::components;
use crate::localization::{t, S};
use crate::{hspace, vspace, Message, Moonlight, Page, TELEGRAM_BOT_URL};

pub fn view(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();

    if app.preferences().subscription_url.is_none() {
        return container(components::action_row(
            Icon::Plus,
            palette.accent,
            palette.text_on_accent,
            t(S::AddSubscription, locale).to_string(),
            t(S::PasteFromBot, locale).to_string(),
            Some(Icon::ChevronRight),
            Some(Message::Navigate(Page::Import)),
            palette,
        ))
        .padding(18)
        .style(move |_| crate::theme::panel(palette))
        .into();
    }

    row![
        column![plan_card(app), traffic_card(app)]
            .spacing(16)
            .width(Length::FillPortion(1)),
        actions(app).width(Length::FillPortion(1)),
    ]
    .spacing(20)
    .into()
}

/// The lime plan card. Everything on it is ink on accent, which is why the
/// palette keeps `text_on_accent` as its own role rather than reusing `text`.
fn plan_card(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let info = app.info();

    let stat = |label: S, value: String| {
        column![
            text(t(label, locale))
                .size(scale::MICRO)
                .font(moonlight_design::ui(EMPHATIC))
                // On the accent wash, accent-coloured type would vanish; this is
                // the role that exists for exactly this position.
                .color(palette.accent_ink_strong),
            text(value)
                .font(moonlight_design::display())
                .size(scale::LEAD)
                .color(palette.text_on_accent),
        ]
        .spacing(2)
    };

    let (status_label, status_fill) = if info.is_active() {
        (t(S::Active, locale), palette.text_on_accent)
    } else {
        (t(S::Expired, locale), palette.danger)
    };

    let content = column![
        row![
            text(t(S::Plan, locale))
                .size(scale::BODY_SM)
                .font(moonlight_design::ui(EMPHATIC))
                .color(palette.accent_ink_strong),
            hspace(Length::Fill),
            components::pill(status_label.to_string(), status_fill, palette.accent),
        ]
        .align_y(Alignment::Center),
        text(
            info.title
                .as_deref()
                .map(format::without_emoji)
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| t(S::NavSubscription, locale).to_string())
        )
        .font(moonlight_design::display())
        .size(scale::HERO)
        .color(palette.text_on_accent),
        vspace(Length::Fixed(6.0)),
        // No device count: Remnawave does not report one on every plan, so the
        // figure was usually a dash sitting between two real numbers.
        row![
            stat(S::Remaining, format::time_left(info.expire, locale)),
            stat(S::Traffic, format::bytes(info.used(), locale)),
        ]
        .spacing(26),
    ]
    .spacing(8);

    container(content)
        .padding(24)
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(palette.accent)),
            border: Border {
                radius: iced::border::Radius::from(radii::PANEL),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

fn traffic_card(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let info = app.info();

    let mut content = column![row![
        components::overline(t(S::Traffic, locale), palette),
        hspace(Length::Fill),
        text(format::quota(info.used(), info.total, locale))
            .size(scale::BODY_SM)
            .font(moonlight_design::ui(EMPHATIC))
            .color(palette.text),
    ]
    .align_y(Alignment::Center)]
    .spacing(12);

    // Only a plan with a quota gets a bar. An unlimited plan with an empty bar
    // under it reads as "nothing used of nothing".
    if let Some(fraction) = info.used_fraction() {
        content = content.push(components::bar(fraction as f32, palette, 8.0));
    }

    if info.expire.is_some() {
        content = content.push(
            text(format!(
                "{} {}",
                t(S::ValidUntil, locale),
                format::date(info.expire, locale)
            ))
            .size(scale::META)
            .color(palette.text_muted),
        );
    }

    // A panel, not a card: `card` is surface-2, which in light mode is #F1F3EB
    // against a #F2F3ED page — a one-value difference nobody can see, so the
    // bar and its labels floated on the background with no card around them.
    // Every other card on this screen is the white surface with a hairline.
    components::surface(content, palette)
}

fn actions(app: &Moonlight) -> iced::widget::Column<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();

    let refreshed = if app.is_refreshing() {
        t(S::Checking, locale)
    } else {
        t(S::RefreshedJustNow, locale)
    };

    let url = app
        .preferences()
        .subscription_url
        .clone()
        .unwrap_or_default();

    column![
        components::surface(
            components::action_row(
                Icon::RefreshCw,
                palette.accent,
                palette.text_on_accent,
                t(S::RefreshSubscription, locale).to_string(),
                refreshed.to_string(),
                None,
                Some(Message::Refresh),
                palette,
            ),
            palette
        ),
        components::surface(
            column![
                components::action_row(
                    Icon::Sparkles,
                    palette.cat2,
                    palette.text_on_accent,
                    t(S::ExtendSubscription, locale).to_string(),
                    t(S::OpensAccount, locale).to_string(),
                    // An outward-pointing mark, because this opens a browser —
                    // a chevron would promise another screen inside the app.
                    Some(Icon::ExternalLink),
                    Some(Message::OpenUrl(TELEGRAM_BOT_URL)),
                    palette,
                ),
                components::divider(palette),
                components::action_row(
                    Icon::Plus,
                    palette.cat4,
                    palette.text_on_accent,
                    t(S::AddSubscription, locale).to_string(),
                    t(S::PasteFromBot, locale).to_string(),
                    Some(Icon::ChevronRight),
                    Some(Message::Navigate(Page::Import)),
                    palette,
                ),
                components::divider(palette),
                components::action_row(
                    Icon::Trash2,
                    palette.danger,
                    palette.text_on_accent,
                    t(S::RemoveSubscription, locale).to_string(),
                    url,
                    None,
                    Some(Message::RemoveSubscription),
                    palette,
                ),
            ]
            .spacing(2),
            palette
        ),
    ]
    .spacing(16)
}
