//! Adding or changing one of the user's rules.
//!
//! The value is checked as it is typed, against the same grammar the core
//! applies — a bad one never reaches a config. A process rule can take its
//! value from a running or installed program, which is the job the apps screen
//! used to do.

use iced::widget::{
    button, column, container, opaque, pick_list, row, scrollable, text, text_input,
};
use iced::{Alignment, Background, Border, Color, Element, Length};

use moonlight_core::rules::{self, Family, Kind, Priority};
use moonlight_design::motion::radii;
use moonlight_design::typography::{scale, EMPHATIC};
use moonlight_design::{icon, Icon};

use crate::localization::{self, t, S};
use crate::{components, hspace, theme, vspace, Message, Moonlight, RuleEditor};

/// A kind as the picker lists it: its token, marked when it needs TUN.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KindChoice(pub Kind);

impl std::fmt::Display for KindChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0.needs_process_matching() {
            write!(f, "{}   · TUN", self.0.token())
        } else {
            f.write_str(self.0.token())
        }
    }
}

pub fn view<'a>(app: &'a Moonlight, editor: &'a RuleEditor) -> Element<'a, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();

    let label = |key| {
        text(t(key, locale))
            .size(13.5)
            .font(moonlight_design::ui(EMPHATIC))
            .color(palette.text)
    };

    let kinds: Vec<KindChoice> = Kind::ALL.iter().copied().map(KindChoice).collect();
    let kind = pick_list(kinds, Some(KindChoice(editor.kind)), |choice| {
        Message::EditorKind(choice.0)
    })
    .font(moonlight_design::mono())
    .padding([10, 14])
    .width(Length::Fill)
    .style(move |_, status| theme::picker(palette, status))
    .menu_style(move |_| theme::picker_menu(palette));

    let field = text_input(editor.kind.placeholder(), &editor.value)
        .on_input(Message::EditorValue)
        .on_submit(Message::EditorSave)
        .font(moonlight_design::mono())
        .padding([10, 14])
        .size(14.0)
        .width(Length::Fill)
        .style(move |_, status| theme::field(palette, status));
    let mut value = row![field].spacing(8).align_y(Alignment::Center);
    if editor.kind.family() == Family::Process {
        value = value.push(
            button(icon(Icon::Layers, 17.0, palette.text))
                .on_press(Message::EditorPicker)
                .padding(11)
                .style(move |_, status| theme::header_button(palette, status)),
        );
    }

    let mut body = column![
        label(S::RuleType),
        kind,
        vspace(Length::Fixed(6.0)),
        label(S::RuleValue),
        value,
    ]
    .spacing(8);
    if editor.picker {
        body = body.push(picker(app, editor));
    }
    if editor.kind.needs_process_matching() {
        body = body.push(
            text(t(S::RuleTunOnly, locale))
                .size(scale::META)
                .color(palette.text_muted),
        );
    }
    if let Some(invalid) = &editor.error {
        body = body.push(components::issue_line(
            localization::invalid(invalid, locale).to_string(),
            palette,
        ));
    }
    body = body
        .push(vspace(Length::Fixed(6.0)))
        .push(label(S::RuleTarget))
        .push(targets(app, editor))
        .push(vspace(Length::Fixed(6.0)))
        .push(label(S::RulePriority))
        .push(
            row![
                priority_card(
                    app,
                    editor,
                    Priority::Override,
                    S::PriorityOverride,
                    S::PriorityOverrideSub
                ),
                priority_card(
                    app,
                    editor,
                    Priority::Extend,
                    S::PriorityExtend,
                    S::PriorityExtendSub
                ),
            ]
            .spacing(10),
        );

    let title = if editor.editing.is_some() {
        S::RulesEdit
    } else {
        S::RulesAdd
    };
    let header = row![
        text(t(title, locale))
            .font(moonlight_design::display())
            .size(24.0)
            .color(palette.text),
        hspace(Length::Fill),
        button(icon(Icon::X, 16.0, palette.text_muted))
            .on_press(Message::EditorCancel)
            .padding(8)
            .style(move |_, status| theme::row_button(palette, false, status)),
    ]
    .align_y(Alignment::Center);

    let footer = row![
        hspace(Length::Fill),
        button(
            text(t(S::Cancel, locale))
                .size(14.5)
                .font(moonlight_design::ui(EMPHATIC))
        )
        .on_press(Message::EditorCancel)
        .padding([11, 22])
        .style(move |_, status| theme::header_button(palette, status)),
        button(
            text(t(
                if editor.editing.is_some() {
                    S::Save
                } else {
                    S::RulesAdd
                },
                locale
            ))
            .size(14.5)
            .font(moonlight_design::ui(EMPHATIC))
        )
        .on_press(Message::EditorSave)
        .padding([11, 22])
        .style(move |_, status| theme::accent_button(palette, status)),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    let card = container(
        column![
            container(header).padding([20, 28]),
            components::divider(palette),
            scrollable(container(body).padding([20, 28]))
                .height(Length::Shrink)
                .style(move |theme, _| theme::scroller(palette, theme)),
            components::divider(palette),
            container(footer).padding([14, 28]),
        ]
        .width(Length::Fill),
    )
    .width(Length::Fixed(620.0))
    .max_height(760.0)
    .style(move |_| container::Style {
        background: Some(Background::Color(palette.surface)),
        border: Border {
            radius: iced::border::Radius::from(radii::PANEL),
            width: 1.0,
            color: palette.hairline,
        },
        ..Default::default()
    });

    opaque(
        container(card)
            .center(Length::Fill)
            .padding(24)
            .style(|_| container::Style {
                background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.5))),
                ..Default::default()
            }),
    )
}

/// Running programs first, then installed ones, with a filter. A pick fills
/// the value in the form the rule's kind takes.
fn picker<'a>(app: &'a Moonlight, editor: &'a RuleEditor) -> Element<'a, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let filter = editor.picker_filter.trim().to_lowercase();
    let wanted = |name: &str, executable: &str| {
        filter.is_empty()
            || name.to_lowercase().contains(&filter)
            || executable.to_lowercase().contains(&filter)
    };
    let by_path = editor.kind == Kind::ProcessPath || editor.kind == Kind::ProcessPathRegex;
    let regex = editor.kind == Kind::ProcessNameRegex || editor.kind == Kind::ProcessPathRegex;
    let pick = move |executable: &str, path: &str| {
        let value = if by_path && !path.is_empty() {
            path
        } else {
            executable
        };
        Message::EditorPickApp(if regex {
            format!("(?i)^{}$", regex_escape(value))
        } else {
            value.to_string()
        })
    };

    let entry = |name: String, executable: String, path: String| -> Element<'a, Message> {
        let tile: Element<'a, Message> = match app.app_icon(&executable) {
            Some(handle) => iced::widget::image(handle.clone())
                .width(Length::Fixed(22.0))
                .height(Length::Fixed(22.0))
                .into(),
            None => components::letter_tile(&name, &executable, palette),
        };
        button(
            row![
                tile,
                text(name)
                    .size(13.5)
                    .color(palette.text)
                    .width(Length::Fill),
                text(executable.clone())
                    .font(moonlight_design::mono())
                    .size(12.0)
                    .color(palette.text_muted),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        )
        .on_press(pick(&executable, &path))
        .padding([6, 10])
        .width(Length::Fill)
        .style(move |_, status| theme::row_button(palette, false, status))
        .into()
    };

    let mut list = column![].spacing(2);
    let running: Vec<&String> = app
        .running()
        .iter()
        .filter(|exe| wanted(exe, exe))
        .collect();
    if !running.is_empty() {
        list = list.push(components::overline(t(S::RuleRunning, locale), palette));
        for executable in running {
            let known = app
                .apps()
                .iter()
                .find(|a| a.executable.eq_ignore_ascii_case(executable));
            list = list.push(entry(
                known.map_or_else(|| executable.clone(), |a| a.name.clone()),
                executable.clone(),
                known.map(|a| a.path.clone()).unwrap_or_default(),
            ));
        }
    }
    let installed: Vec<_> = app
        .apps()
        .iter()
        .filter(|a| wanted(&a.name, &a.executable))
        .collect();
    if !installed.is_empty() {
        list = list.push(components::overline(t(S::RuleInstalled, locale), palette));
        for a in installed {
            list = list.push(entry(a.name.clone(), a.executable.clone(), a.path.clone()));
        }
    }

    container(
        column![
            text_input(t(S::SearchApps, locale), &editor.picker_filter)
                .on_input(Message::EditorPickerFilter)
                .padding([8, 12])
                .size(13.5)
                .style(move |_, status| theme::field(palette, status)),
            scrollable(list)
                .height(Length::Fixed(220.0))
                .style(move |theme, _| theme::scroller(palette, theme)),
        ]
        .spacing(8),
    )
    .padding(10)
    .style(move |_| theme::card(palette))
    .into()
}

fn regex_escape(value: &str) -> String {
    value
        .chars()
        .flat_map(|c| {
            let special = r"\.+*?()|[]{}^$".contains(c);
            special
                .then_some('\\')
                .into_iter()
                .chain(std::iter::once(c))
        })
        .collect()
}

fn targets<'a>(app: &'a Moonlight, editor: &'a RuleEditor) -> Element<'a, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let option =
        |name: String, dot: Option<Color>, note: Option<&'static str>| -> Element<'a, Message> {
            let chosen = editor.target == name;
            let mut line = row![].spacing(10).align_y(Alignment::Center);
            if let Some(dot) = dot {
                line = line.push(
                    container(vspace(Length::Fixed(8.0)))
                        .width(Length::Fixed(8.0))
                        .style(move |_| container::Style {
                            background: Some(Background::Color(dot)),
                            border: Border {
                                radius: iced::border::Radius::from(radii::PILL),
                                ..Default::default()
                            },
                            ..Default::default()
                        }),
                );
            }
            line = line.push(
                text(name.clone())
                    .size(14.0)
                    .font(moonlight_design::ui(EMPHATIC))
                    .color(palette.text)
                    .width(Length::Fill),
            );
            if let Some(note) = note {
                line = line.push(text(note).size(12.5).color(palette.text_muted));
            }
            if chosen {
                line = line.push(icon(Icon::Check, 15.0, palette.text));
            }
            button(line)
                .on_press(Message::EditorTarget(name))
                .padding([9, 12])
                .width(Length::Fill)
                .style(move |_, status| theme::row_button(palette, chosen, status))
                .into()
        };

    let mut list = column![
        components::overline(t(S::TargetBuiltIn, locale), palette),
        option(
            rules::DIRECT.into(),
            Some(palette.st_up),
            Some(t(S::TargetDirect, locale))
        ),
        option(
            rules::REJECT.into(),
            Some(palette.danger),
            Some(t(S::TargetReject, locale))
        ),
    ]
    .spacing(4);
    if !app.routing_groups().is_empty() {
        list = list.push(components::overline(t(S::TargetGroups, locale), palette));
        for group in app.routing_groups() {
            list = list.push(option(group.clone(), None, None));
        }
    }
    // A rule being edited may point at a group the subscription has dropped;
    // it stays visible, marked, rather than silently changing its target.
    let known = editor.target == rules::DIRECT
        || editor.target == rules::REJECT
        || app.routing_groups().contains(&editor.target);
    if !known && !editor.target.is_empty() {
        list = list.push(option(editor.target.clone(), Some(palette.danger), None));
        list = list.push(
            text(t(S::TargetMissing, locale))
                .size(scale::META)
                .color(palette.danger),
        );
    }

    container(
        scrollable(list.padding(8))
            .height(Length::Fixed(220.0))
            .style(move |theme, _| theme::scroller(palette, theme)),
    )
    .style(move |_| theme::card(palette))
    .into()
}

fn priority_card<'a>(
    app: &'a Moonlight,
    editor: &'a RuleEditor,
    priority: Priority,
    title: S,
    note: S,
) -> Element<'a, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let chosen = editor.priority == priority;
    let ring = container(if chosen {
        container(vspace(Length::Fixed(8.0)))
            .width(Length::Fixed(8.0))
            .style(move |_| container::Style {
                background: Some(Background::Color(palette.text)),
                border: Border {
                    radius: iced::border::Radius::from(radii::PILL),
                    ..Default::default()
                },
                ..Default::default()
            })
    } else {
        container(vspace(Length::Fixed(0.0)))
    })
    .center(Length::Fixed(20.0))
    .style(move |_| container::Style {
        border: Border {
            radius: iced::border::Radius::from(radii::PILL),
            width: 2.0,
            color: if chosen {
                palette.text
            } else {
                palette.text_muted
            },
        },
        ..Default::default()
    });
    button(
        row![
            ring,
            column![
                text(t(title, locale))
                    .size(15.0)
                    .font(moonlight_design::ui(EMPHATIC))
                    .color(palette.text),
                text(t(note, locale)).size(12.5).color(palette.text_muted),
            ]
            .spacing(3),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .on_press(Message::EditorPriority(priority))
    .padding([14, 14])
    .width(Length::Fill)
    .style(move |_, status| theme::row_button(palette, chosen, status))
    .into()
}

#[cfg(test)]
mod tests {
    use super::regex_escape;

    #[test]
    fn a_picked_name_is_matched_literally_by_a_regex_rule() {
        assert_eq!(regex_escape("my.app(1).exe"), r"my\.app\(1\)\.exe");
        // And what it makes is a pattern the core's grammar accepts.
        let pattern = format!("(?i)^{}$", regex_escape("my.app(1).exe"));
        assert_eq!(
            moonlight_core::rules::validate(
                moonlight_core::rules::Kind::ProcessNameRegex,
                &pattern
            ),
            None
        );
    }
}
