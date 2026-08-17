//! The sidebar, and the quota block at its foot.
//!
//! It collapses to a 72pt icon rail; the wordmark is the toggle.

use iced::widget::{button, canvas, column, container, row, text};
use iced::{Alignment, Background, Element, Length};

use moonlight_core::preferences::Preferences;
use moonlight_core::{format, AppLocale, SubscriptionInfo};
use moonlight_design::motion::{border, metrics, radii};
use moonlight_design::typography::{scale, EMPHATIC};
use moonlight_design::{icon, Icon, Palette};

use crate::components;
use crate::localization::{t, S};
use crate::logo::Logo;
use crate::{hspace, theme, vspace, Message, Page};

/// The collapsed rail and the full sidebar, from `tokens/spacing.css`.
const RAIL: f32 = metrics::RAIL_COLLAPSED;
const EXPANDED: f32 = metrics::RAIL;

/// The wordmark. Smaller and lighter than the display steps: 17px at 700, from
/// the composition — it labels the app rather than titling a page.
const WORDMARK: f32 = 17.0;

/// The logo tile.
const MARK: f32 = 32.0;

pub fn view<'a>(
    palette: Palette,
    locale: AppLocale,
    current: Page,
    collapsed: bool,
    preferences: &'a Preferences,
    info: &'a SubscriptionInfo,
) -> Element<'a, Message> {
    let width = if collapsed { RAIL } else { EXPANDED };

    let mut items = column![].spacing(6);
    for page in Page::SIDEBAR {
        items = items.push(nav_item(palette, locale, page, current, collapsed));
    }

    let content = column![
        wordmark(palette, collapsed),
        items,
        vspace(Length::Fill),
        quota(palette, locale, collapsed, preferences, info),
    ]
    .spacing(6)
    .padding(iced::Padding {
        top: 18.0,
        right: 14.0,
        bottom: 16.0,
        left: 14.0,
    })
    .width(Length::Fixed(width));

    container(content)
        .width(Length::Fixed(width))
        .height(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(palette.bg_deep)),
            ..Default::default()
        })
        .into()
}

/// The rail's right-hand hairline, drawn as its own strip so it spans the full
/// height rather than being clipped by the rail's padding.
///
/// Without it the deep-slate rail and the slate page meet at two values close
/// enough to read as one uneven surface.
pub fn rule<'a>(palette: Palette) -> Element<'a, Message> {
    container(hspace(Length::Fixed(1.0)))
        .height(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(palette.hairline)),
            ..Default::default()
        })
        .into()
}

/// The logo and the wordmark, which together double as the collapse toggle —
/// which is why this is a button rather than a label with a chevron beside it.
fn wordmark<'a>(palette: Palette, collapsed: bool) -> Element<'a, Message> {
    let mark = canvas(Logo::new(palette))
        .width(Length::Fixed(MARK))
        .height(Length::Fixed(MARK));

    let inner: Element<'a, Message> = if collapsed {
        mark.into()
    } else {
        row![
            mark,
            text("moonlight")
                .font(moonlight_design::display())
                .size(WORDMARK)
                .color(palette.text),
            hspace(Length::Fill),
            icon(Icon::PanelLeftClose, 16.0, palette.text_muted),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .into()
    };

    button(inner)
        .on_press(Message::ToggleSidebar)
        .padding(iced::Padding {
            top: 0.0,
            right: 6.0,
            bottom: 14.0,
            left: 6.0,
        })
        .width(Length::Fill)
        .style(move |_, status| theme::nav_button(palette, status))
        .into()
}

fn nav_item<'a>(
    palette: Palette,
    locale: AppLocale,
    page: Page,
    current: Page,
    collapsed: bool,
) -> Element<'a, Message> {
    let selected = page == current;
    // On the accent fill the glyph takes ink, not the accent — the same rule
    // the type follows.
    let ink = if selected {
        palette.text_on_accent
    } else {
        palette.text2
    };

    let inner: Element<'a, Message> = if collapsed {
        icon(page.icon(), 19.0, ink)
    } else {
        row![
            icon(page.icon(), 19.0, ink),
            text(t(page.title(), locale))
                .size(scale::BODY_SM)
                .font(moonlight_design::ui(EMPHATIC))
                .color(ink),
        ]
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
    };

    button(inner)
        .on_press(Message::Navigate(page))
        .height(Length::Fixed(metrics::NAV_ROW))
        .padding([0, 12])
        .width(Length::Fill)
        .style(move |_, status| {
            if selected {
                theme::accent_button(palette, status)
            } else {
                theme::nav_button(palette, status)
            }
        })
        .into()
}

/// The quota block. A partial fill is the point here, which is exactly why the
/// connect dial does not carry one.
fn quota<'a>(
    palette: Palette,
    locale: AppLocale,
    collapsed: bool,
    preferences: &'a Preferences,
    info: &'a SubscriptionInfo,
) -> Element<'a, Message> {
    if collapsed {
        // 72pt has no room for a plan figure and a bar, and half of one reads
        // as a clipped layout rather than as a deliberate collapse.
        return vspace(Length::Fixed(0.0)).into();
    }
    if preferences.subscription_url.is_none() {
        return button(
            text(t(S::AddSubscription, locale))
                .size(scale::META)
                .color(palette.text_muted),
        )
        .on_press(Message::Navigate(Page::Import))
        .padding(14)
        .width(Length::Fill)
        .style(move |_, status| theme::nav_button(palette, status))
        .into();
    }

    let (status_label, status_fill) = if info.is_active() {
        (t(S::Active, locale), palette.accent)
    } else {
        (t(S::Expired, locale), palette.danger)
    };

    let mut content = column![
        row![
            components::overline(t(S::Remaining, locale), palette),
            hspace(Length::Fill),
            components::pill(
                status_label.to_string(),
                status_fill,
                palette.text_on_accent
            ),
        ]
        .align_y(Alignment::Center),
        text(format::time_left(info.expire, locale))
            .font(moonlight_design::display())
            .size(22.0)
            .color(palette.text),
    ]
    .spacing(8);

    // The bar is only drawn for a plan that has a quota. An unlimited plan with
    // an empty bar under it reads as "nothing used of nothing".
    if let Some(fraction) = info.used_fraction() {
        content = content.push(components::bar(fraction as f32, palette, 6.0));
    }
    content = content.push(
        text(format!(
            "{} {}",
            format::quota(info.used(), info.total, locale),
            t(S::OfTraffic, locale)
        ))
        .size(12.0)
        .color(palette.text_muted),
    );

    // A button, not a card: it goes to the subscription screen, and the
    // composition lifts its border to the accent on hover to say so.
    button(content)
        .on_press(Message::Navigate(Page::Subscription))
        .padding(14)
        .width(Length::Fill)
        .style(move |_, status| {
            let mut style = theme::outlined(palette, status);
            style.border.radius = iced::border::Radius::from(radii::CARD_SM);
            style.border.width = border::HAIRLINE;
            style
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rail_widths_come_from_the_tokens() {
        // --ml-rail-w and --ml-rail-w-tablet. The Swift port had rounded these
        // to 248 and 72, which is a 12px and a 4px drift from the design.
        assert_eq!(EXPANDED, 236.0);
        assert_eq!(RAIL, 76.0);
    }

    #[test]
    fn the_quota_block_is_hidden_on_the_rail() {
        let preferences = Preferences::default();
        let info = SubscriptionInfo::default();
        let element = quota(Palette::DARK, AppLocale::Ru, true, &preferences, &info);
        // A zero-height spacer is what "nothing here" looks like in iced.
        assert_eq!(element.as_widget().size().height, Length::Fixed(0.0));
    }
}
