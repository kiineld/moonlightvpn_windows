//! Rules: the user's own routing rules, and the subscription's to read.
//!
//! Modelled on Flowvy's "Мои правила". The user's rules are a draft until
//! Apply, which has the core check the config they make before anything
//! changes — one rule it refuses would otherwise take the whole config, and the
//! tunnel, down with it.

use iced::widget::{button, column, container, mouse_area, row, scrollable, text, text_input};
use iced::{Alignment, Background, Border, Element, Length};

use moonlight_core::rules::{self, Priority, ProfileRule, RoutingRule};
use moonlight_core::RoutingMode;
use moonlight_design::motion::radii;
use moonlight_design::typography::{scale, EMPHATIC};
use moonlight_design::{icon, Icon};

use crate::localization::{t, S};
use crate::{components, hspace, theme, Message, Moonlight, RulesTab};

/// Every row is this tall, which is what lets a drag turn its distance into a
/// number of places moved.
pub const ROW_HEIGHT: f32 = 52.0;

/// A row of the subscription's rules, and that list's scroller.
const PROFILE_ROW: f32 = 44.0;
pub const PROFILE_SCROLL: &str = "profile-rules";
/// Rows built beyond the ones in view, either side, so a fast scroll does
/// not outrun them.
const OVERSCAN: usize = 12;

const HANDLE: f32 = 22.0;
const TYPE_WIDTH: f32 = 196.0;
const TARGET_WIDTH: f32 = 170.0;
const PRIORITY_WIDTH: f32 = 112.0;

pub fn view(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let tab = app.rules_tab();

    let count = match tab {
        RulesTab::Mine => format!(
            "{}: {}",
            t(S::RulesOwnCount, locale),
            app.rules_draft().len()
        ),
        RulesTab::Subscription => format!(
            "{}: {}",
            t(S::RulesProfileCount, locale),
            app.profile_rules().len()
        ),
    };
    let mut toolbar = row![
        components::segmented(
            &[
                (RulesTab::Mine, t(S::RulesMine, locale)),
                (RulesTab::Subscription, t(S::RulesProfile, locale)),
            ],
            tab,
            Message::RulesTab,
            palette,
        ),
        components::count_pill(count, palette),
        text_input(t(S::RulesFilter, locale), app.rules_filter())
            .on_input(Message::RulesFilter)
            .padding([9, 14])
            .size(13.5)
            .width(Length::Fill)
            .style(move |_, status| theme::field(palette, status)),
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    if tab == RulesTab::Mine {
        toolbar = toolbar.push(
            button(
                row![
                    icon(Icon::Plus, 15.0, palette.text_on_accent),
                    text(t(S::RulesAdd, locale))
                        .size(13.5)
                        .font(moonlight_design::ui(EMPHATIC)),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
            .on_press(Message::RuleNew)
            .padding([9, 16])
            .style(move |_, status| theme::accent_button(palette, status)),
        );
    }

    let mut page = column![toolbar].spacing(14);
    if app.preferences().routing_mode != RoutingMode::Rule {
        page = page.push(
            text(t(S::RulesModeNote, locale))
                .size(scale::META)
                .color(palette.text_muted),
        );
    }
    if tab == RulesTab::Mine && (app.rules_dirty() || app.rules_applying()) {
        page = page.push(apply_bar(app));
    }

    page.push(match tab {
        RulesTab::Mine => mine(app),
        RulesTab::Subscription => profile(app),
    })
    .into()
}

fn apply_bar(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let applying = app.rules_applying();
    container(
        row![
            icon(Icon::CircleAlert, 16.0, palette.text_muted),
            text(t(S::RulesUnsaved, locale))
                .size(13.5)
                .color(palette.text2),
            hspace(Length::Fill),
            button(
                text(t(S::RulesReset, locale))
                    .size(13.5)
                    .font(moonlight_design::ui(EMPHATIC))
            )
            .on_press_maybe((!applying).then_some(Message::RulesReset))
            .padding([8, 16])
            .style(move |_, status| theme::header_button(palette, status)),
            button(
                text(t(
                    if applying {
                        S::RulesApplying
                    } else {
                        S::RulesApply
                    },
                    locale
                ))
                .size(13.5)
                .font(moonlight_design::ui(EMPHATIC))
            )
            .on_press_maybe((!applying).then_some(Message::RulesApply))
            .padding([8, 18])
            .style(move |_, status| theme::accent_button(palette, status)),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .padding([10, 16])
    .style(move |_| theme::panel(palette))
    .into()
}

fn matches(filter: &str, fields: [&str; 3]) -> bool {
    let filter = filter.trim().to_lowercase();
    filter.is_empty() || fields.iter().any(|f| f.to_lowercase().contains(&filter))
}

fn header<'a>(app: &Moonlight, lead: f32) -> Element<'a, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let label = |key| {
        text(t(key, locale))
            .size(scale::MICRO)
            .font(moonlight_design::ui(EMPHATIC))
            .color(palette.text_muted)
    };
    // The user's rows lead with a grip and a switch and end with two actions;
    // the subscription's have none of those, and no priority either.
    let mine = lead > 0.0;
    let mut heading = row![].spacing(12).padding([10, 16]);
    if mine {
        heading = heading.push(hspace(Length::Fixed(lead)));
    }
    heading = heading
        .push(label(S::ColType).width(Length::Fixed(TYPE_WIDTH)))
        .push(label(S::ColValue).width(Length::Fill));
    if mine {
        heading
            .push(label(S::ColTarget).width(Length::Fixed(TARGET_WIDTH)))
            .push(label(S::ColPriority).width(Length::Fixed(PRIORITY_WIDTH)))
            .push(hspace(Length::Fixed(72.0)))
            .into()
    } else {
        heading
            .push(label(S::ColTarget).width(Length::Fixed(TARGET_WIDTH + PRIORITY_WIDTH + 12.0)))
            .into()
    }
}

/// A kind, set as the grammar writes it, on a quiet chip.
fn kind_chip<'a>(label: String, app: &Moonlight, dim: bool) -> Element<'a, Message> {
    let palette = app.palette_of();
    container(
        text(label)
            .font(moonlight_design::mono())
            .size(12.5)
            .color(if dim {
                palette.text_muted
            } else {
                palette.text
            }),
    )
    .padding([4, 8])
    // A wash of the text colour, not the glass: a chip on a white card is
    // white glass on white, and would not be there.
    .style(move |_| container::Style {
        background: Some(Background::Color(theme::alpha(palette.text, 0.07))),
        border: Border {
            radius: iced::border::Radius::from(radii::CHIP),
            ..Default::default()
        },
        ..Default::default()
    })
    .into()
}

fn mine(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let rules = app.rules_in_order();
    if rules.is_empty() {
        return components::surface(
            components::empty_state_full(
                Icon::Route,
                t(S::RulesEmpty, locale).to_string(),
                t(S::RulesEmptyHint, locale).to_string(),
                Some((t(S::RulesAdd, locale).to_string(), Message::RuleNew)),
                palette,
            ),
            palette,
        );
    }

    // Dragging reorders the whole list, so it is offered only when all of it
    // is showing.
    let filter = app.rules_filter();
    let can_drag = filter.trim().is_empty();
    let dragging = app.dragging_rule();

    let mut rows = column![];
    for (index, rule) in rules.iter().enumerate() {
        if !matches(filter, [rule.kind.token(), &rule.value, &rule.target]) {
            continue;
        }
        rows = rows.push(rule_row(
            app,
            rule,
            index,
            can_drag,
            dragging == Some(rule.id),
        ));
    }
    // The rows scroll inside their card, under a heading that stays, as the
    // subscription's do. Both tabs are then the same page with a different
    // list in it — and the switch between them is the same switch throughout,
    // which is what lets its capsule glide across rather than be drawn anew
    // on the other side. As tall as its rows, and no taller than the page.
    let list = scrollable(rows.padding(iced::Padding {
        right: crate::SCROLLBAR_GUTTER,
        ..iced::Padding::ZERO
    }))
    .direction(scrollable::Direction::Vertical(
        scrollable::Scrollbar::new()
            .width(crate::SCROLLBAR_WIDTH)
            .scroller_width(crate::SCROLLBAR_WIDTH)
            .margin(crate::SCROLLBAR_MARGIN),
    ))
    .width(Length::Fill)
    .height(Length::Shrink)
    .style(move |theme, _| theme::scroller(palette, theme));

    container(column![
        header(app, HANDLE + 12.0 + 44.0),
        components::divider(palette),
        list
    ])
    .style(move |_| theme::panel(palette))
    .into()
}

fn rule_row<'a>(
    app: &'a Moonlight,
    rule: &'a RoutingRule,
    index: usize,
    can_drag: bool,
    lifted: bool,
) -> Element<'a, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let dim = !rule.enabled;
    let ink = |color: iced::Color| {
        if dim {
            theme::alpha(color, 0.45)
        } else {
            color
        }
    };

    let grip = container(icon(Icon::GripVertical, 16.0, palette.text_muted))
        .width(Length::Fixed(HANDLE))
        .center_x(Length::Fixed(HANDLE));
    let handle: Element<'a, Message> = if can_drag {
        mouse_area(grip)
            .on_press(Message::DragStart(index))
            .interaction(iced::mouse::Interaction::Grab)
            .into()
    } else {
        grip.into()
    };

    let missing = rule.target != rules::DIRECT
        && rule.target != rules::REJECT
        && !app.routing_groups().contains(&rule.target);
    let target_color = if rule.target == rules::REJECT || missing {
        palette.danger
    } else {
        palette.text2
    };

    let priority = match rule.priority {
        Priority::Override => S::PriorityOverride,
        Priority::Extend => S::PriorityExtend,
    };
    let action = |glyph, message| {
        button(icon(glyph, 16.0, palette.text_muted))
            .on_press(message)
            .padding(8)
            .style(move |_, status| theme::row_button(palette, false, status))
    };

    let content = row![
        handle,
        components::toggle(rule.enabled, Message::RuleToggle(rule.id), palette),
        container(kind_chip(rule.kind.token().to_string(), app, dim))
            .width(Length::Fixed(TYPE_WIDTH)),
        text(rule.value.clone())
            .font(moonlight_design::mono())
            .size(13.5)
            .color(ink(palette.text))
            .width(Length::Fill),
        text(if missing {
            format!("{} ⚠", rule.target)
        } else {
            rule.target.clone()
        })
        .size(13.5)
        .font(moonlight_design::ui(EMPHATIC))
        .color(ink(target_color))
        .width(Length::Fixed(TARGET_WIDTH)),
        container(kind_chip(t(priority, locale).to_uppercase(), app, dim))
            .width(Length::Fixed(PRIORITY_WIDTH)),
        action(Icon::SquarePen, Message::RuleEdit(rule.id)),
        action(Icon::Trash2, Message::RuleDelete(rule.id)),
    ]
    .spacing(12)
    .align_y(Alignment::Center);

    container(content)
        .padding([0, 16])
        .height(Length::Fixed(ROW_HEIGHT))
        .center_y(Length::Fixed(ROW_HEIGHT))
        .style(move |_| container::Style {
            // Solid while it is carried, so the rows it passes over do not
            // show through it.
            background: lifted.then_some(Background::Color(palette.raised)),
            ..Default::default()
        })
        .into()
}

fn profile(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    if app.profile_rules().is_empty() {
        return components::surface(
            components::empty_state(t(S::RulesProfileEmpty, locale), palette),
            palette,
        );
    }
    // A subscription carries thousands of these. Built all at once they took
    // seconds to lay out, so only the rows in view are built — every row is
    // the same height, which is what lets the rest be two empty spaces.
    //
    // ponytail: the filter runs over every line each frame while it is set.
    // Keep the matches between frames if a subscription ever ships tens of
    // thousands of rules.
    let filter = app.rules_filter().trim().to_lowercase();
    let shown: Vec<&String> = app
        .profile_rules()
        .iter()
        .filter(|line| filter.is_empty() || line.to_lowercase().contains(&filter))
        .collect();
    let (first, last) = window(app.rules_scroll(), shown.len());

    let mut rows = column![crate::vspace(Length::Fixed(first as f32 * PROFILE_ROW))];
    for line in &shown[first..last] {
        let rule = ProfileRule::parse(line);
        let target_color = if rule.target.eq_ignore_ascii_case(rules::REJECT) {
            palette.danger
        } else {
            palette.text2
        };
        rows = rows.push(
            container(
                row![
                    container(kind_chip(rule.kind, app, false)).width(Length::Fixed(TYPE_WIDTH)),
                    text(rule.value)
                        .font(moonlight_design::mono())
                        .size(13.5)
                        .color(palette.text)
                        .wrapping(iced::widget::text::Wrapping::None)
                        .width(Length::Fill),
                    text(rule.target)
                        .size(13.5)
                        .font(moonlight_design::ui(EMPHATIC))
                        .color(target_color)
                        .wrapping(iced::widget::text::Wrapping::None)
                        .width(Length::Fixed(TARGET_WIDTH + PRIORITY_WIDTH + 12.0)),
                ]
                .spacing(12)
                .align_y(Alignment::Center),
            )
            .padding([0, 16])
            .height(Length::Fixed(PROFILE_ROW))
            .center_y(Length::Fixed(PROFILE_ROW))
            .clip(true),
        );
    }
    rows = rows.push(crate::vspace(Length::Fixed(
        (shown.len() - last) as f32 * PROFILE_ROW,
    )));

    let list = scrollable(rows.padding(iced::Padding {
        right: crate::SCROLLBAR_GUTTER,
        ..iced::Padding::ZERO
    }))
    .id(PROFILE_SCROLL)
    .on_scroll(Message::RulesScrolled)
    .direction(scrollable::Direction::Vertical(
        scrollable::Scrollbar::new()
            .width(crate::SCROLLBAR_WIDTH)
            .scroller_width(crate::SCROLLBAR_WIDTH)
            .margin(crate::SCROLLBAR_MARGIN),
    ))
    .width(Length::Fill)
    .height(Length::Fill)
    .style(move |theme, _| theme::scroller(palette, theme));

    container(column![
        header(app, 0.0),
        components::divider(palette),
        list
    ])
    .height(Length::Fill)
    .style(move |_| theme::panel(palette))
    .into()
}

/// Which rows to build, of `total`: the ones in view and a margin either side.
/// The room's height is unknown until the list first reports it; until then a
/// tall window's worth is built.
fn window((offset, height): (f32, f32), total: usize) -> (usize, usize) {
    let height = if height > 0.0 { height } else { 1400.0 };
    let first = ((offset / PROFILE_ROW).floor().max(0.0) as usize)
        .saturating_sub(OVERSCAN)
        .min(total);
    let last = (first + (height / PROFILE_ROW).ceil() as usize + 2 * OVERSCAN).min(total);
    (first, last)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_rows_in_view_are_built_however_many_there_are() {
        // At the head of five thousand: a screenful and its margin.
        let (first, last) = window((0.0, 600.0), 5000);
        assert_eq!(first, 0);
        assert!(last < 60, "{last} rows built for a 600px room");
        // Scrolled well down: the same number, around where it is.
        let (first, last) = window((44_000.0, 600.0), 5000);
        assert!(first <= 1000 && last > 1000);
        assert!(last - first < 60);
    }

    #[test]
    fn the_window_never_runs_past_the_list() {
        assert_eq!(window((0.0, 600.0), 0), (0, 0));
        assert_eq!(window((0.0, 600.0), 3), (0, 3));
        // Scrolled further than a list the filter has just shortened.
        let (first, last) = window((90_000.0, 600.0), 10);
        assert_eq!((first, last), (10, 10));
    }

    #[test]
    fn a_room_not_yet_measured_gets_a_tall_windows_worth() {
        let (first, last) = window((0.0, 0.0), 5000);
        assert_eq!(first, 0);
        assert!(last >= 32);
    }
}
