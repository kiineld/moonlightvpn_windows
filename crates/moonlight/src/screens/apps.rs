//! Split tunnelling: the program list on the left, the rules on the right.

use iced::widget::{button, column, container, pick_list, row, text, text_input};
use iced::{Alignment, Element, Length};

use moonlight_core::split_rule::Kind;
use moonlight_core::{SplitMode, TunnelMode};
use moonlight_design::typography::{scale, EMPHATIC};
use moonlight_design::{icon, Icon};

use crate::components;
use crate::localization::{t, S};
use crate::{hspace, theme, vspace, Message, Moonlight};

pub fn view(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let mode = app.preferences().split_mode;

    let note = match mode {
        SplitMode::All => S::SplitAllNote,
        SplitMode::Only => S::SplitOnlyNote,
        SplitMode::Except => S::SplitExceptNote,
    };

    let head = row![
        components::segmented(
            &[
                (SplitMode::All, t(S::SplitAll, locale)),
                (SplitMode::Only, t(S::SplitOnly, locale)),
                (SplitMode::Except, t(S::SplitExcept, locale)),
            ],
            mode,
            Message::SetSplitMode,
            palette,
        ),
        hspace(Length::Fixed(22.0)),
        text(t(note, locale))
            .size(scale::BODY_SM)
            .color(palette.text2),
    ]
    .align_y(Alignment::Center);

    column![
        head,
        vspace(Length::Fixed(18.0)),
        row![
            programs(app).width(Length::FillPortion(1)),
            rules(app).width(Length::FillPortion(1)),
        ]
        .spacing(20),
    ]
    .into()
}

fn programs(app: &Moonlight) -> iced::widget::Column<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let needle = app.app_search().to_lowercase();

    let head = row![
        components::overline(t(S::Programs, locale), palette),
        hspace(Length::Fill),
        text_input(t(S::SearchApps, locale), app.app_search())
            .on_input(Message::AppSearchChanged)
            .padding([9, 14])
            .size(scale::BODY_SM)
            .width(Length::Fixed(220.0))
            .style(move |_, status| theme::field(palette, status)),
    ]
    .align_y(Alignment::Center);

    let matching: Vec<_> = app
        .apps()
        .iter()
        .filter(|entry| {
            needle.is_empty()
                || entry.name.to_lowercase().contains(&needle)
                || entry.executable.to_lowercase().contains(&needle)
        })
        .collect();

    let mut list = column![].spacing(2);
    if matching.is_empty() {
        list = list.push(components::empty_state(t(S::NoApps, locale), palette));
    } else {
        for entry in matching {
            let on = app.preferences().has_app(&entry.executable);
            let running = app.is_running(&entry.executable);

            let mut title = row![text(entry.name.clone())
                .size(scale::BODY)
                .font(moonlight_design::ui(EMPHATIC))
                .color(palette.text)]
            .spacing(8)
            .align_y(Alignment::Center);
            if running {
                title = title.push(components::pill(
                    t(S::RunningNow, locale).to_string(),
                    palette.accent_quiet,
                    palette.accent_ink,
                ));
            }

            list = list.push(
                row![
                    column![
                        title,
                        // The executable, because that is what a PROCESS-NAME
                        // rule will actually match — showing only the display
                        // name hides what the rule is made of.
                        text(entry.executable.clone())
                            .font(moonlight_design::mono())
                            .size(scale::META)
                            .color(palette.text_muted),
                    ]
                    .spacing(2),
                    hspace(Length::Fill),
                    components::toggle(on, Message::ToggleApp(entry.executable.clone()), palette),
                ]
                .padding([10, 12])
                .align_y(Alignment::Center),
            );
        }
    }

    column![
        head,
        vspace(Length::Fixed(12.0)),
        components::surface(list, palette),
    ]
}

fn rules(app: &Moonlight) -> iced::widget::Column<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();

    let kinds: Vec<Kind> = Kind::ALL.to_vec();
    let composer = column![
        pick_list(kinds, Some(app.rule_kind()), Message::RuleKindChanged)
            .padding([8, 12])
            .text_size(scale::BODY_SM)
            .width(Length::Fill),
        row![
            text_input(app.rule_kind().placeholder(), app.rule_value())
                .on_input(Message::RuleValueChanged)
                .on_submit(Message::RuleSubmit)
                .padding([12, 14])
                .size(scale::BODY_SM)
                .style(move |_, status| theme::field(palette, status)),
            button(icon(Icon::Plus, 18.0, palette.text_on_accent))
                .on_press(Message::RuleSubmit)
                .padding(12)
                .style(move |_, status| theme::accent_button(palette, status)),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    ]
    .spacing(10);

    let mut panel = column![composer].spacing(10);

    // A rejected rule says why, in place, rather than failing silently or
    // waiting for the core to refuse the whole config.
    if let Some(error) = app.rule_error() {
        panel = panel.push(
            text(error.to_string())
                .size(scale::META)
                .color(palette.danger),
        );
    }

    for rule in &app.preferences().split_rules {
        // App-list rules are shown as toggles on the left; repeating them here
        // would offer two switches for one thing.
        if rule.is_from_app_list() {
            continue;
        }
        let needs_tun =
            rule.kind.needs_process_matching() && app.preferences().mode != TunnelMode::Tun;

        panel = panel.push(components::divider(palette));
        panel = panel.push(
            row![
                column![
                    row![text(rule.kind.token())
                        .font(moonlight_design::mono())
                        .size(scale::MICRO)
                        .color(if needs_tun {
                            palette.warning
                        } else {
                            palette.text_muted
                        }),],
                    text(rule.value.clone())
                        .font(moonlight_design::mono())
                        .size(scale::BODY_SM)
                        .color(palette.text),
                ]
                .spacing(2),
                hspace(Length::Fill),
                button(icon(Icon::Trash2, 16.0, palette.text_muted))
                    .on_press(Message::DeleteRule(rule.id))
                    .padding(8)
                    .style(move |_, status| theme::nav_button(palette, status)),
                components::toggle(rule.enabled, Message::ToggleRule(rule.id), palette),
            ]
            .spacing(8)
            .padding([8, 10])
            .align_y(Alignment::Center),
        );
    }

    column![
        components::overline(t(S::RulesHeading, locale), palette),
        vspace(Length::Fixed(12.0)),
        components::surface(panel, palette),
        vspace(Length::Fixed(10.0)),
        container(
            text(t(S::RulesFootnote, locale))
                .size(scale::META)
                .color(palette.text_muted)
        )
        .padding([0, 4]),
    ]
}
