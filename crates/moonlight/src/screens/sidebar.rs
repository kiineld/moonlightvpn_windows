//! The sidebar, and the quota block at its foot.
//!
//! Ported metric-for-metric from the macOS client's `RootView.Sidebar`, which is
//! the reference this product is meant to look like: 236pt expanded, 72pt as an
//! icon rail, and a quota card that is always present — with zeroes rather than
//! dashes before a subscription exists.
//!
//! It floats: a rounded panel inset from the window's edges, with the tab that
//! opens and closes it swelling out of its own edge.

use iced::widget::canvas::{Geometry, LineCap, LineJoin, Path, Stroke};
use iced::widget::{button, canvas, column, container, row, text};
use iced::{
    mouse, Alignment, Background, Border, Element, Length, Point, Rectangle, Renderer, Theme,
};

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

/// How far the tab swells out of the panel's edge, and how far along the edge
/// the swell reaches either side of its middle.
const TAB_DEPTH: f32 = 16.0;
const TAB_REACH: f32 = 30.0;
/// The strip of the tab laid back over the panel, covering the panel's own
/// border where the swell leaves it.
pub const TAB_OVERLAP: f32 = 2.5;

/// The gap between the panel and the window's edges.
pub const INSET: f32 = 10.0;

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
            background: Some(Background::Color(palette.surface)),
            border: Border {
                radius: iced::border::Radius::from(radii::CARD_LG),
                width: border::HAIRLINE,
                color: palette.hairline,
            },
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

/// The tab that opens and closes the rail.
///
/// Not a chip laid on the seam but the panel's own edge swelling out, leaving
/// and rejoining it along its tangent, in the panel's fill and rimmed by its
/// hairline — one surface, as the macOS client draws it. The chevron turns
/// with the rail rather than being swapped: `turn` is 0 open, pointing left,
/// and 1 closed, pointing right, and in between while the rail glides.
///
/// Returned on its own so the shell can float it over the edge, vertically
/// centred, rather than reserving a column for it.
pub fn edge_toggle<'a>(palette: Palette, turn: f32) -> Element<'a, Message> {
    button(
        canvas(Tab { palette, turn })
            .width(Length::Fixed(TAB_OVERLAP + TAB_DEPTH + 1.0))
            .height(Length::Fixed(TAB_REACH * 2.0)),
    )
    .on_press(Message::ToggleSidebar)
    .padding(0)
    .style(|_, _| button::Style::default())
    .into()
}

struct Tab {
    palette: Palette,
    turn: f32,
}

impl<Message> canvas::Program<Message> for Tab {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        // The panel's border is drawn inside its bounds, so its line sits half
        // a pixel in from the edge; the swell leaves from that line.
        let (edge, middle) = (TAB_OVERLAP - 0.5, bounds.height / 2.0);
        let (reach, depth) = (TAB_REACH, TAB_DEPTH);

        // Two cubics, each leaving the edge along it and meeting the other
        // square to it at the deepest point: a bell, not a bitten-off circle.
        let swell = |b: &mut canvas::path::Builder| {
            b.move_to(Point::new(edge, middle - reach));
            b.bezier_curve_to(
                Point::new(edge, middle - reach * 0.45),
                Point::new(edge + depth, middle - reach * 0.5),
                Point::new(edge + depth, middle),
            );
            b.bezier_curve_to(
                Point::new(edge + depth, middle + reach * 0.5),
                Point::new(edge, middle + reach * 0.45),
                Point::new(edge, middle + reach),
            );
        };
        let body = Path::new(|b| {
            swell(b);
            b.line_to(Point::new(0.0, middle + reach));
            b.line_to(Point::new(0.0, middle - reach));
            b.close();
        });
        frame.fill(&body, self.palette.surface);
        frame.stroke(
            &Path::new(swell),
            Stroke::default()
                .with_width(border::HAIRLINE)
                .with_color(self.palette.hairline),
        );

        let ink = if cursor.is_over(bounds) {
            self.palette.text
        } else {
            self.palette.text2
        };
        let centre = Point::new(edge + depth * 0.42, middle);
        let (sin, cos) = (std::f32::consts::PI * self.turn).sin_cos();
        let at =
            |x: f32, y: f32| Point::new(centre.x + x * cos - y * sin, centre.y + x * sin + y * cos);
        let chevron = Path::new(|b| {
            b.move_to(at(2.0, -4.5));
            b.line_to(at(-2.0, 0.0));
            b.line_to(at(2.0, 4.5));
        });
        frame.stroke(
            &chevron,
            Stroke::default()
                .with_width(2.0)
                .with_color(ink)
                .with_line_cap(LineCap::Round)
                .with_line_join(LineJoin::Round),
        );
        vec![frame.into_geometry()]
    }
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
