//! The parts every screen is built from.
//!
//! The macOS client's screens are composed of the same half-dozen shapes — a
//! panel, a card, a coloured icon tile, a list row, a toggle, a segmented
//! control, an overline. Building them once here is what keeps six screens
//! looking like one product, and it is also the only way a token change lands
//! everywhere rather than in the five places somebody remembered.

use iced::widget::{button, column, container, row, text, Space};
use iced::{Alignment, Background, Border, Color, Element, Length};

use moonlight_design::motion::radii;
use moonlight_design::typography::{scale, EMPHATIC};
use moonlight_design::{icon, Icon, Palette};

use crate::theme;

/// A section heading: small, muted, letterspaced, upper case.
///
/// The design sets these in caps in the string itself rather than through a
/// transform, so the localisation carries the case and this only styles it.
pub fn overline<'a, M: 'a>(label: &'a str, palette: Palette) -> Element<'a, M> {
    text(label)
        .size(scale::MICRO)
        .font(moonlight_design::ui(EMPHATIC))
        .color(palette.text_muted)
        .into()
}

/// A hairline divider.
pub fn divider<'a, M: 'a>(palette: Palette) -> Element<'a, M> {
    container(Space::new().height(Length::Fixed(1.0)))
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(palette.hairline)),
            ..Default::default()
        })
        .into()
}

/// The rounded square that carries an icon at the head of a row.
///
/// Its fill is a *category* colour, not the accent: the Subscription and
/// Settings screens use five of them side by side, and making them all lime
/// would lose the only thing distinguishing one action from the next.
pub fn tile<'a, M: 'a>(glyph: Icon, fill: Color, ink: Color) -> Element<'a, M> {
    container(icon(glyph, 20.0, ink))
        .padding(11)
        .style(move |_| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                radius: iced::border::Radius::from(radii::TILE),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

/// A small filled pill — *Активна*, *Запущено*.
pub fn pill<'a, M: 'a>(label: String, fill: Color, ink: Color) -> Element<'a, M> {
    container(
        text(label)
            .size(scale::MICRO)
            .font(moonlight_design::ui(EMPHATIC))
            .color(ink),
    )
    .padding([3, 9])
    .style(move |_| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
            radius: iced::border::Radius::from(radii::PILL),
            ..Default::default()
        },
        ..Default::default()
    })
    .into()
}

/// A title-and-subtitle stack, which is most of every list row.
pub fn titled<'a, M: 'a>(title: String, subtitle: String, palette: Palette) -> Element<'a, M> {
    column![
        text(title)
            .size(scale::BODY)
            .font(moonlight_design::ui(EMPHATIC))
            .color(palette.text),
        text(subtitle).size(scale::META).color(palette.text_muted),
    ]
    .spacing(1)
    .into()
}

/// A row that opens something: tile, title, subtitle, and a trailing glyph.
///
/// The trailing glyph says where it goes — a chevron for a screen inside the
/// app, an external-link mark for a browser. Getting that wrong is how a user
/// ends up surprised by a browser window.
#[allow(clippy::too_many_arguments)]
pub fn action_row<'a, M: Clone + 'a>(
    glyph: Icon,
    fill: Color,
    ink: Color,
    title: String,
    subtitle: String,
    trailing: Option<Icon>,
    on_press: Option<M>,
    palette: Palette,
) -> Element<'a, M> {
    let mut content = row![tile(glyph, fill, ink), titled(title, subtitle, palette)]
        .spacing(13)
        .align_y(Alignment::Center);

    content = content.push(Space::new().width(Length::Fill));
    if let Some(trailing) = trailing {
        content = content.push(icon(trailing, 17.0, palette.text_muted));
    }

    let mut element = button(content)
        .padding([12, 14])
        .width(Length::Fill)
        .style(move |_, status| theme::row_button(palette, false, status));
    if let Some(message) = on_press {
        element = element.on_press(message);
    }
    element.into()
}

/// The toggle switch: a track that fills with the accent, and a knob that
/// slides.
///
/// Drawn as a button rather than a checkbox because iced's checkbox is a tick
/// in a box and this design's is a switch — and because the whole row should be
/// the target, not a 20pt square at the end of it.
pub fn toggle<'a, M: Clone + 'a>(on: bool, message: M, palette: Palette) -> Element<'a, M> {
    let track_fill = if on { palette.accent } else { palette.surface3 };
    // The knob is white in both states and both themes: on the accent it is the
    // only thing that reads, and on the empty track it is what says "switch"
    // rather than "empty pill".
    let knob = container(Space::new().width(Length::Fixed(20.0)))
        .height(Length::Fixed(20.0))
        .style(|_| container::Style {
            background: Some(Background::Color(Color::WHITE)),
            border: Border {
                radius: iced::border::Radius::from(radii::PILL),
                ..Default::default()
            },
            ..Default::default()
        });

    let inner = if on {
        row![Space::new().width(Length::Fill), knob]
    } else {
        row![knob, Space::new().width(Length::Fill)]
    };

    button(
        container(inner.align_y(Alignment::Center))
            .width(Length::Fixed(48.0))
            .height(Length::Fixed(26.0))
            .padding(3)
            .style(move |_| container::Style {
                background: Some(Background::Color(track_fill)),
                border: Border {
                    radius: iced::border::Radius::from(radii::PILL),
                    ..Default::default()
                },
                ..Default::default()
            }),
    )
    .on_press(message)
    .padding(0)
    .style(|_, _| button::Style {
        background: None,
        text_color: Color::WHITE,
        ..Default::default()
    })
    .into()
}

/// A row with a label, an optional explanation, and a control on the right.
pub fn setting_row<'a, M: 'a>(
    title: String,
    subtitle: Option<String>,
    control: Element<'a, M>,
    palette: Palette,
) -> Element<'a, M> {
    let mut labels = column![text(title)
        .size(scale::BODY)
        .font(moonlight_design::ui(EMPHATIC))
        .color(palette.text)]
    .spacing(2);
    if let Some(subtitle) = subtitle {
        labels = labels.push(text(subtitle).size(scale::META).color(palette.text_muted));
    }

    row![labels, Space::new().width(Length::Fill), control]
        .align_y(Alignment::Center)
        .padding([14, 16])
        .into()
}

/// The segmented control — *Весь трафик · Только эти · Кроме этих*.
///
/// The selected segment is an accent pill inside a surface track.
pub fn segmented<'a, T: Copy + PartialEq + 'a, M: Clone + 'a>(
    options: &[(T, &'a str)],
    selected: T,
    on_select: impl Fn(T) -> M + 'a,
    palette: Palette,
) -> Element<'a, M> {
    let mut track = row![].spacing(0);
    for (value, label) in options {
        let is_selected = *value == selected;
        let ink = if is_selected {
            palette.text_on_accent
        } else {
            palette.text2
        };
        track = track.push(
            button(
                text(*label)
                    .size(scale::BODY_SM)
                    .font(moonlight_design::ui(EMPHATIC))
                    .color(ink),
            )
            .on_press(on_select(*value))
            .padding([9, 18])
            .style(move |_, status| {
                if is_selected {
                    theme::accent_button(palette, status)
                } else {
                    button::Style {
                        background: None,
                        text_color: ink,
                        border: Border {
                            radius: iced::border::Radius::from(radii::PILL),
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                }
            }),
        );
    }

    container(track)
        .padding(4)
        .style(move |_| container::Style {
            background: Some(Background::Color(palette.surface)),
            border: Border {
                radius: iced::border::Radius::from(radii::PILL),
                width: 1.0,
                color: palette.hairline,
            },
            ..Default::default()
        })
        .into()
}

/// A quota bar. A partial fill is the point here, which is exactly why the
/// connect dial does not carry one.
pub fn bar<'a, M: 'a>(fraction: f32, palette: Palette, height: f32) -> Element<'a, M> {
    let fraction = fraction.clamp(0.0, 1.0);
    // FillPortion needs integers, and a zero portion collapses the filled half
    // entirely — which is the right shape for an untouched quota.
    let filled = (fraction * 1000.0) as u16;
    let empty = 1000_u16.saturating_sub(filled);

    let mut track = row![].spacing(0);
    if filled > 0 {
        track = track.push(
            container(Space::new().height(Length::Fixed(height)))
                .width(Length::FillPortion(filled))
                .style(move |_| container::Style {
                    background: Some(Background::Color(palette.accent_line)),
                    border: Border {
                        radius: iced::border::Radius::from(radii::PILL),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
        );
    }
    if empty > 0 {
        track = track.push(
            container(Space::new().height(Length::Fixed(height)))
                .width(Length::FillPortion(empty))
                .style(move |_| container::Style {
                    background: Some(Background::Color(palette.surface3)),
                    border: Border {
                        radius: iced::border::Radius::from(radii::PILL),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
        );
    }
    track.height(Length::Fixed(height)).into()
}

/// A card: surface, rounded, padded.
pub fn card<'a, M: 'a>(content: impl Into<Element<'a, M>>, palette: Palette) -> Element<'a, M> {
    container(content)
        .padding(16)
        .width(Length::Fill)
        .style(move |_| theme::card(palette))
        .into()
}

/// A panel: the outer surface a screen's columns sit on.
pub fn surface<'a, M: 'a>(content: impl Into<Element<'a, M>>, palette: Palette) -> Element<'a, M> {
    container(content)
        .padding(18)
        .width(Length::Fill)
        .style(move |_| theme::panel(palette))
        .into()
}

/// An empty-state line: centred, muted, and never blank — a blank area reads as
/// a bug where a sentence reads as a state.
pub fn empty_state<'a, M: 'a>(message: &'a str, palette: Palette) -> Element<'a, M> {
    container(text(message).size(scale::BODY_SM).color(palette.text_muted))
        .center_x(Length::Fill)
        .padding(28)
        .into()
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_full_bar_has_no_empty_half_and_an_empty_bar_no_filled_half() {
        // FillPortion(0) still lays out, so the halves are added conditionally;
        // this is the arithmetic that decides.
        let full = (1.0_f32 * 1000.0) as u16;
        assert_eq!(full, 1000);
        assert_eq!(1000_u16.saturating_sub(full), 0);

        let empty = (0.0_f32 * 1000.0) as u16;
        assert_eq!(empty, 0);
        assert_eq!(1000_u16.saturating_sub(empty), 1000);
    }

    #[test]
    fn a_bar_fraction_past_one_cannot_overflow_the_track() {
        let over = (1.7_f32.clamp(0.0, 1.0) * 1000.0) as u16;
        assert_eq!(over, 1000);
        assert_eq!(1000_u16.saturating_sub(over), 0);
    }
}
