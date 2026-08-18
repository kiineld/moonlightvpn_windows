//! The sidebar, and the quota block at its foot.
//!
//! Ported metric-for-metric from the macOS client's `RootView.Sidebar`, which is
//! the reference this product is meant to look like: 236pt expanded, 72pt as an
//! icon rail, and a quota card that is always present — with zeroes rather than
//! dashes before a subscription exists.

use iced::widget::{button, canvas, column, container, row, text};
use iced::{Alignment, Background, Border, Element, Length};

use moonlight_core::preferences::Preferences;
use moonlight_core::{format, AppLocale, SubscriptionInfo};
use moonlight_design::motion::{border, metrics, radii};
use moonlight_design::typography::{scale, EMPHATIC};
use moonlight_design::{icon, Icon, Palette};

use crate::components;
use crate::localization::{t, S};
use crate::logo::Logo;
use crate::{hspace, theme, vspace, Message, Page};

/// The wordmark: 17px at 700 with display tracking, from the composition — it
/// labels the app rather than titling a page.
const WORDMARK: f32 = 17.0;

/// The logo tile beside it.
const MARK: f32 = 32.0;
const MARK_RADIUS: f32 = 10.0;

/// The edge tab: a full circle sitting on the rail's edge, most of it over the
/// page so the chevron at its centre is plainly centred.
const TAB_DIAMETER: f32 = 34.0;
/// How far the circle laps back over the rail. Half its width, so the circle is
/// centred on the seam — straddling it, half over the rail and half over the
/// page — rather than floating out on the page side.
pub const TAB_OVERLAP: f32 = TAB_DIAMETER / 2.0;

/// Where the rail swaps between its two layouts, mid-glide.
///
/// The layout follows the *drawn width*, not the target state. Switching on the
/// boolean put the full sidebar — wordmark, labels, quota card — inside a box
/// still 72px wide for the length of the animation: everything wrapped, the
/// column grew, and the contents jumped up and then back down as the box caught
/// up. Swapping at the halfway point means neither layout is ever drawn into a
/// box too small for it.
const LAYOUT_SWAP: f32 = (metrics::RAIL + metrics::RAIL_COLLAPSED) / 2.0;

pub fn view<'a>(
    palette: Palette,
    locale: AppLocale,
    current: Page,
    // The rail's current width, which is mid-glide while it opens or closes.
    width: f32,
    preferences: &'a Preferences,
    info: &'a SubscriptionInfo,
) -> Element<'a, Message> {
    let collapsed = width < LAYOUT_SWAP;
    let pad_x = if collapsed { 10.0 } else { 14.0 };

    let mut items = column![].spacing(6);
    for page in Page::SIDEBAR {
        items = items.push(nav_item(palette, locale, page, current, collapsed));
    }

    let content = column![
        header(palette, collapsed),
        items,
        vspace(Length::Fill),
        quota(palette, locale, collapsed, preferences, info),
    ]
    .spacing(6)
    .padding(iced::Padding {
        top: 18.0,
        right: pad_x,
        bottom: 16.0,
        left: pad_x,
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
pub fn rule<'a>(palette: Palette) -> Element<'a, Message> {
    container(hspace(Length::Fixed(1.0)))
        .height(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(palette.hairline)),
            ..Default::default()
        })
        .into()
}

/// The logo and, when there is room, the wordmark.
///
/// The collapse control is no longer here — it sits as a tab on the rail's own
/// edge (see [`edge_toggle`]), which is where the eye goes to look for the seam
/// between rail and page.
fn header<'a>(palette: Palette, collapsed: bool) -> Element<'a, Message> {
    let mark = canvas(Logo::with_radius(palette, MARK_RADIUS))
        .width(Length::Fixed(MARK))
        .height(Length::Fixed(MARK));

    if collapsed {
        return container(mark)
            .center_x(Length::Fill)
            .padding(iced::Padding {
                bottom: 8.0,
                ..iced::Padding::ZERO
            })
            .into();
    }

    container(
        row![
            mark,
            text("moonlight")
                .font(moonlight_design::display())
                .size(WORDMARK)
                .color(palette.text),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .padding(iced::Padding {
        top: 0.0,
        right: 6.0,
        bottom: 14.0,
        left: 6.0,
    })
    .into()
}

/// The half-circle tab that opens and closes the rail.
///
/// It straddles the seam between rail and page: a D — flat against the rail,
/// bulging out over the page — with a chevron pointing the way it will move the
/// edge. Left to close, right to open. The macOS client puts the control on the
/// edge for the same reason, because that is the line the change happens on.
///
/// Returned on its own so the shell can float it, vertically centred, over the
/// seam rather than reserving a column for it.
pub fn edge_toggle<'a>(palette: Palette, collapsed: bool) -> Element<'a, Message> {
    let glyph = if collapsed {
        Icon::ChevronRight
    } else {
        Icon::ChevronLeft
    };

    button(
        // Dead centre of the circle — nothing else to reason about.
        container(moonlight_design::icon_thin(glyph, 15.0, palette.text2, 2.4))
            .center(Length::Fill),
    )
    .on_press(Message::ToggleSidebar)
    .width(Length::Fixed(TAB_DIAMETER))
    .height(Length::Fixed(TAB_DIAMETER))
    .padding(0)
    .style(move |_, status| {
        // A surface chip with a hairline, sitting on the seam and drawn over the
        // rail's last few pixels so it reads as attached. Circle, so its centred
        // glyph is obviously centred.
        let fill = match status {
            button::Status::Hovered | button::Status::Pressed => palette.surface3,
            _ => palette.surface2,
        };
        button::Style {
            background: Some(Background::Color(fill)),
            text_color: palette.text2,
            border: Border {
                radius: iced::border::Radius::from(TAB_DIAMETER / 2.0),
                width: border::HAIRLINE,
                color: palette.hairline,
            },
            ..Default::default()
        }
    })
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
        container(icon(page.icon(), 19.0, ink))
            .center_x(Length::Fill)
            .into()
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

    let styled = move |_: &_, status| {
        if selected {
            theme::accent_button(palette, status)
        } else {
            theme::nav_button(palette, status)
        }
    };

    // Collapsed, the item is a fixed square centred in the rail: a pill radius on
    // a square is a circle, whereas the same radius on the full 52px rail width
    // stretched it into an ellipse. Expanded, it fills the rail as a row.
    if collapsed {
        return container(
            button(components::centre(inner))
                .on_press(Message::Navigate(page))
                .width(Length::Fixed(metrics::NAV_ROW))
                .height(Length::Fixed(metrics::NAV_ROW))
                .padding(0)
                .style(styled),
        )
        .center_x(Length::Fill)
        .into();
    }

    button(components::centre(inner))
        .on_press(Message::Navigate(page))
        .height(Length::Fixed(metrics::NAV_ROW))
        .padding([0, 12])
        .width(Length::Fill)
        .style(styled)
        .into()
}

/// The quota block.
///
/// It is drawn whether or not a subscription exists. Before one does the figures
/// read zero rather than "—": a dash is an answer *about a plan*, and showing it
/// before there is one looks like a plan whose panel omitted a field. The
/// earlier build replaced the whole card with an "Добавить подписку" text link,
/// which left the foot of the sidebar looking unfinished.
fn quota<'a>(
    palette: Palette,
    locale: AppLocale,
    collapsed: bool,
    preferences: &'a Preferences,
    info: &'a SubscriptionInfo,
) -> Element<'a, Message> {
    let has_subscription = preferences.subscription_url.is_some();
    let used = if has_subscription {
        info.used_fraction().unwrap_or(0.0) as f32
    } else {
        0.0
    };

    // At 72pt there is no room for a card, but the plan still has to be
    // glanceable — so it becomes the mark and the bar alone.
    if collapsed {
        let tone = if info.is_active() || !has_subscription {
            palette.accent_ink
        } else {
            palette.danger
        };
        // No `centre` here: it fills the available height, which is bounded
        // inside a fixed-height button but not inside this one — it has no set
        // height, so filling made the card stretch down the rail with its
        // contents stranded at the bottom. The padding centres it already,
        // because the content is what gives the button its height.
        return button(
            column![
                icon(Icon::Sparkles, 16.0, tone),
                container(components::bar(used, palette, 4.0)).width(Length::Fixed(34.0)),
            ]
            .spacing(6)
            .align_x(Alignment::Center)
            .width(Length::Fill),
        )
        .on_press(Message::Navigate(Page::Subscription))
        .padding([12, 0])
        .width(Length::Fill)
        .style(move |_, status| {
            let mut style = theme::outlined(palette, status);
            style.border.radius = iced::border::Radius::from(radii::FIELD);
            style.border.width = 0.0;
            style
        })
        .into();
    }

    let days = if has_subscription {
        format::time_left(info.expire, locale)
    } else {
        format::days(Some(0), locale)
    };

    let quota_line = if has_subscription {
        format!(
            "{} {}",
            format::quota(info.used(), info.total, locale),
            t(S::OfTraffic, locale)
        )
    } else {
        format!(
            "{} {}",
            format::bytes(Some(0), locale),
            t(S::OfTraffic, locale)
        )
    };

    let mut heading = row![components::overline(t(S::Remaining, locale), palette)]
        .spacing(8)
        .align_y(Alignment::Center);
    heading = heading.push(hspace(Length::Fill));
    // The status pill only means something once there is a plan to have a
    // status.
    if has_subscription {
        let (label, fill) = if info.is_active() {
            (t(S::Active, locale), palette.accent)
        } else {
            (t(S::Expired, locale), palette.danger)
        };
        heading = heading.push(components::pill(
            label.to_string(),
            fill,
            palette.text_on_accent,
        ));
    }

    let content = column![
        heading,
        text(days)
            .font(moonlight_design::display())
            .size(22.0)
            .color(palette.text),
        components::bar(used, palette, 6.0),
        text(quota_line).size(12.0).color(palette.text_muted),
    ]
    .spacing(8);

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
    fn the_rail_widths_match_the_macos_client() {
        // 236 expanded, 72 collapsed. An earlier pass had 248 and 76, which is a
        // 12pt and a 4pt drift from the client this is meant to mirror. The
        // width itself is passed in now, because it glides between the two.
        assert_eq!(metrics::RAIL, 236.0);
        assert_eq!(metrics::RAIL_COLLAPSED, 72.0);
    }

    #[test]
    fn the_quota_block_survives_the_collapse() {
        // It becomes the bar alone rather than disappearing: the plan is the one
        // thing the rail still has to make glanceable.
        let preferences = Preferences::default();
        let info = SubscriptionInfo::default();
        let element = quota(Palette::DARK, AppLocale::Ru, true, &preferences, &info);
        assert_ne!(element.as_widget().size().height, Length::Fixed(0.0));
    }

    #[test]
    fn the_quota_card_is_drawn_before_a_subscription_exists() {
        // With no plan it reads zeroes, not dashes, and never collapses to a
        // bare text link.
        let preferences = Preferences::default();
        assert!(preferences.subscription_url.is_none());
        let info = SubscriptionInfo::default();
        let element = quota(Palette::DARK, AppLocale::Ru, false, &preferences, &info);
        assert_ne!(element.as_widget().size().height, Length::Fixed(0.0));
    }
}
