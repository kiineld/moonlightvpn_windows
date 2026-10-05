//! The subscription screen: the plan card, the traffic card, and the actions.

use iced::widget::{column, container, row, text};
use iced::{Alignment, Background, Border, Element, Length};

use moonlight_core::format;
use moonlight_design::motion::radii;
use moonlight_design::typography::{scale, EMPHATIC};
use moonlight_design::Icon;

use crate::components;
use crate::localization::{t, S};
use crate::theme;
use crate::{
    hspace, localization, vspace, Message, Moonlight, Page, CABINET_URL, TELEGRAM_BOT_URL,
};

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
        .padding(components::GROUP_PADDING)
        .style(move |_| crate::theme::panel(palette))
        .into();
    }

    let columns = row![
        column![plan_card(app), traffic_card(app)]
            .spacing(16)
            .width(Length::FillPortion(1)),
        actions(app).width(Length::FillPortion(1)),
    ]
    .spacing(16);

    match app.announce() {
        Some(message) => column![
            // As wide as the page, which is as wide as the window lets it be:
            // measured for a narrow one, so a wider one only arrives early.
            components::announce_banner(
                message,
                app.announce_openness(),
                760.0,
                Message::ToggleAnnounce,
                palette
            ),
            columns,
        ]
        .spacing(14)
        .into(),
        None => columns.into(),
    }
}

/// The plan card, on the accent fill. Everything on it is ink on accent, which
/// is why the palette keeps `text_on_accent` as its own role rather than
/// reusing `text`.
fn plan_card(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let info = app.info();

    let stat = |label: S, value: String| {
        column![
            text(t(label, locale))
                .size(scale::MICRO)
                .font(moonlight_design::ui(EMPHATIC))
                // Dimmed ink, not `accent_ink_strong`: that role is the fill's
                // own colour, and would vanish on it. Ink at 60% matches the
                // macOS hero's own 0.6-opacity labels.
                .color(theme::alpha(palette.text_on_accent, 0.6)),
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
                .color(theme::alpha(palette.text_on_accent, 0.6)),
            hspace(Length::Fill),
            components::pill(status_label.to_string(), status_fill, palette.accent),
        ]
        .align_y(Alignment::Center),
        text(
            info.title
                .clone()
                .unwrap_or_else(|| t(S::NavSubscription, locale).to_string())
        )
        .font(moonlight_design::display())
        // The plan's name on one line, as the macOS card sets it: at the hero
        // step "moonlight vpn" filled the card's width and its emoji fell to
        // a line of its own.
        .size(scale::TITLE)
        .wrapping(iced::widget::text::Wrapping::None)
        .color(palette.text_on_accent),
        vspace(Length::Fixed(10.0)),
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
    if info.refill_date.is_some() {
        content = content.push(
            text(format!(
                "{} {}",
                t(S::TrafficResets, locale),
                format::date(info.refill_date, locale)
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

    // What the row says under its title: syncing, when it last worked, or
    // what it offers. It used to say "just now" whether or not it ever had.
    // And when the last one failed, why — here, quietly, for a refresh the
    // app made on its own; one the user asked for says so in a note as well.
    let refreshed = if app.is_refreshing() {
        t(S::RefreshMetaSyncing, locale).to_string()
    } else if let Some(issue) = app.refresh_issue() {
        localization::issue(issue, locale)
    } else {
        app.last_updated()
            .unwrap_or_else(|| t(S::RefreshMetaIdle, locale).to_string())
    };

    let refresh = column![components::group(
        components::action_row(
            Icon::RefreshCw,
            palette.accent,
            palette.text_on_accent,
            t(S::RefreshSubscription, locale).to_string(),
            refreshed,
            None,
            Some(Message::Refresh),
            palette,
        ),
        palette
    )]
    .spacing(10);

    column![
        refresh,
        components::group(
            column![
                components::action_row(
                    Icon::Sparkles,
                    palette.cat2,
                    palette.text,
                    t(S::ExtendSubscription, locale).to_string(),
                    t(S::ExtendSubtitle, locale).to_string(),
                    // An outward-pointing mark, because this opens a browser —
                    // a chevron would promise another screen inside the app.
                    // Always the bot: it is where a plan is paid for.
                    Some(Icon::ExternalLink),
                    Some(Message::OpenUrl(TELEGRAM_BOT_URL)),
                    palette,
                ),
                components::row_divider(palette),
                components::action_row(
                    Icon::Globe,
                    palette.cat3,
                    palette.text,
                    t(S::PersonalAccount, locale).to_string(),
                    t(S::PersonalAccountSub, locale).to_string(),
                    Some(Icon::ExternalLink),
                    Some(Message::OpenUrl(CABINET_URL)),
                    palette,
                ),
                // No "add a subscription" beside an active one: importing
                // replaces it, so the row promised something the app does not
                // do. Removing it brings the empty state and its add row back.
                components::row_divider(palette),
                components::action_row(
                    Icon::Trash2,
                    palette.danger,
                    palette.text_on_accent,
                    t(S::RemoveSubscription, locale).to_string(),
                    // Never the link itself: it is a credential, and anyone
                    // who reads it off the screen has the subscription.
                    t(S::RemoveSubscriptionSub, locale).to_string(),
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
