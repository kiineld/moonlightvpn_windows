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
use moonlight_design::typography::{scale, EMPHATIC, ROW_TITLE};
use moonlight_design::{icon, Icon, Palette};

use crate::theme;

/// A list-row title. The composition sets these at 14.5/700 — a step below
/// `--ml-t-body` and a weight below the emphatic default, which is what keeps a
/// stack of settings rows from reading as a stack of buttons.
const ROW_TITLE_SIZE: f32 = 14.5;
/// The sub-line under it: 12px, not `--ml-t-meta`.
const SUB_SIZE: f32 = 12.0;

/// The toggle switch, from the source's own geometry. The knob's travel is
/// `TOGGLE_W - 2*TOGGLE_INSET - TOGGLE_KNOB`, which must come to the
/// `translateX(18px)` the composition animates — see the test below.
const TOGGLE_W: f32 = 44.0;
const TOGGLE_H: f32 = 26.0;
const TOGGLE_KNOB: f32 = 20.0;
const TOGGLE_INSET: f32 = 3.0;

/// The category ramp, in the order the tiles cycle through it.
fn category_fills(palette: Palette) -> [Color; 5] {
    [
        palette.cat1,
        palette.cat2,
        palette.cat3,
        palette.cat4,
        palette.cat5,
    ]
}

/// Which category colour an app row's tile takes.
///
/// Keyed off the executable rather than the display name, because that is the
/// stable identity — a programme that renames itself between releases keeps its
/// colour, and two builds of the same executable do not drift apart.
fn tile_fill(executable: &str, palette: Palette) -> Color {
    let fills = category_fills(palette);
    let hash = executable
        .bytes()
        .fold(0u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
    fills[(hash % fills.len() as u32) as usize]
}

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

/// The softer hairline — `--ml-hairline-soft`. Separates items inside one group,
/// where the full hairline would read as a boundary between two.
pub fn soft_divider<'a, M: 'a>(palette: Palette) -> Element<'a, M> {
    container(Space::new().height(Length::Fixed(1.0)))
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(palette.hairline_soft)),
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
    // 42×42 around a 19px glyph, cornered at 13 — the value the composition sets
    // literally for a tile this size, between `--ml-r-icon` and `--ml-r-icon-lg`.
    container(icon(glyph, 19.0, ink))
        .center(Length::Fixed(42.0))
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

/// The 42px tile at the head of an app row: the programme's initial, set in the
/// display face on a category fill.
///
/// The composition hand-picks a colour per application, which a list of whatever
/// is actually installed cannot do. The fill is chosen by hashing the executable
/// instead, so a given programme keeps its colour between launches and between
/// re-scans — a colour that moved every time the inventory was rebuilt would
/// read as the list re-sorting itself.
pub fn letter_tile<'a, M: 'a>(name: &str, executable: &str, palette: Palette) -> Element<'a, M> {
    let letter: String = name
        .chars()
        .find(|c| c.is_alphanumeric())
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "?".into());

    let fill = tile_fill(executable, palette);

    container(
        text(letter)
            .font(moonlight_design::display())
            .size(17.0)
            .color(palette.text_on_accent),
    )
    .center(Length::Fixed(42.0))
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
            .size(ROW_TITLE_SIZE)
            .font(moonlight_design::ui(ROW_TITLE))
            .color(palette.text),
        text(subtitle).size(SUB_SIZE).color(palette.text_muted),
    ]
    .spacing(2)
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
    // As in `setting_row`: the label stack fills, so a long sub-line wraps rather
    // than crowding the trailing glyph off the row.
    let mut content = row![
        tile(glyph, fill, ink),
        container(titled(title, subtitle, palette)).width(Length::Fill)
    ]
    .spacing(14)
    .align_y(Alignment::Center);

    if let Some(trailing) = trailing {
        content = content.push(icon(trailing, 17.0, palette.text_muted));
    }

    let mut element = button(content)
        .padding([15, 18])
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
    let knob = container(Space::new().width(Length::Fixed(TOGGLE_KNOB)))
        .height(Length::Fixed(TOGGLE_KNOB))
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
            // 44×26 with a 20px knob and 3px inset — the geometry the source
            // animates, where the knob's travel is exactly its `translateX(18px)`.
            .width(Length::Fixed(TOGGLE_W))
            .height(Length::Fixed(TOGGLE_H))
            .padding(TOGGLE_INSET)
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
        .size(ROW_TITLE_SIZE)
        .font(moonlight_design::ui(ROW_TITLE))
        .color(palette.text)]
    .spacing(2);
    if let Some(subtitle) = subtitle {
        labels = labels.push(text(subtitle).size(SUB_SIZE).color(palette.text_muted));
    }

    // The labels take the slack, not a spacer beside them. A `Shrink` column
    // asks for the full intrinsic width of its longest line, which on a row like
    // *Служба не установлена* leaves nothing for the control: the button
    // collapses to a sliver and its label spills out past the card. Filling here
    // resolves after the control's intrinsic width, so the subtitle wraps
    // instead.
    row![labels.width(Length::Fill), control]
        .spacing(14)
        .align_y(Alignment::Center)
        .padding([15, 18])
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
    track(options, selected, on_select, palette, 34.0, 12.5, 14.0)
}

/// The smaller track — the *RU / EN* switch, which the composition sets at 28px
/// with 12px labels so it sits inside a settings row without setting its height.
pub fn segmented_compact<'a, T: Copy + PartialEq + 'a, M: Clone + 'a>(
    options: &[(T, &'a str)],
    selected: T,
    on_select: impl Fn(T) -> M + 'a,
    palette: Palette,
) -> Element<'a, M> {
    track(options, selected, on_select, palette, 28.0, 12.0, 13.0)
}

/// The segmented track both sizes are cut from.
///
/// Surface-2 with **no border** and 3px of padding, per the composition. Giving
/// it the panel surface and a hairline — as an earlier pass did — draws a second
/// bordered box inside a bordered card, which is what made the language switch
/// read as a nested panel rather than as a control.
fn track<'a, T: Copy + PartialEq + 'a, M: Clone + 'a>(
    options: &[(T, &'a str)],
    selected: T,
    on_select: impl Fn(T) -> M + 'a,
    palette: Palette,
    height: f32,
    size: f32,
    pad_x: f32,
) -> Element<'a, M> {
    let mut segments = row![].spacing(3);
    for (value, label) in options {
        let is_selected = *value == selected;
        // Unselected labels take `text-muted`, not `text-2`: the track already
        // sits on a lighter surface, and text-2 there reads as a second
        // selection.
        let ink = if is_selected {
            palette.text_on_accent
        } else {
            palette.text_muted
        };
        segments = segments.push(
            button(
                container(
                    text(*label)
                        .size(size)
                        .font(moonlight_design::ui(EMPHATIC))
                        .color(ink),
                )
                .center_y(Length::Fixed(height)),
            )
            .on_press(on_select(*value))
            .padding([0.0, pad_x])
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

    container(segments)
        .padding(3)
        .style(move |_| container::Style {
            background: Some(Background::Color(palette.surface2)),
            border: Border {
                radius: iced::border::Radius::from(radii::PILL),
                ..Default::default()
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

/// A panel: the outer surface a screen's columns sit on.
pub const SURFACE_PADDING: f32 = 18.0;

pub fn surface<'a, M: 'a>(content: impl Into<Element<'a, M>>, palette: Palette) -> Element<'a, M> {
    container(content)
        .padding(SURFACE_PADDING)
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

/// Centres a button's label in a button whose height is set explicitly.
///
/// iced lays a `button`'s content out at the **top** of its content box, so a
/// 44pt nav row or a 38pt pill puts its label against the upper edge with all
/// the slack below — which is what made every control in the app sit high in its
/// own pill. Only buttons that size themselves from their padding are exempt.
pub fn centre<'a, M: 'a>(content: impl Into<Element<'a, M>>) -> Element<'a, M> {
    container(content).center_y(Length::Fill).into()
}

/// A count chip on the accent wash — *Активно: 0*.
pub fn count_pill<'a, M: 'a>(label: String, palette: Palette) -> Element<'a, M> {
    container(
        text(label)
            .size(scale::META)
            .font(moonlight_design::ui(EMPHATIC))
            .color(palette.accent_ink),
    )
    .padding([0, 12])
    .height(Length::Fixed(30.0))
    .align_y(Alignment::Center)
    .style(move |_| container::Style {
        background: Some(Background::Color(palette.accent_quiet)),
        border: Border {
            radius: iced::border::Radius::from(radii::PILL),
            ..Default::default()
        },
        ..Default::default()
    })
    .into()
}

/// A glyph over one muted line, centred — the empty state for a panel where
/// there is nothing the user can usefully do yet.
pub fn empty_state_icon<'a, M: 'a>(
    glyph: Icon,
    message: &'a str,
    palette: Palette,
) -> Element<'a, M> {
    container(
        column![
            icon(glyph, 30.0, palette.text_muted),
            text(message)
                .size(scale::META)
                .color(palette.text_muted)
                .align_x(Alignment::Center),
        ]
        .spacing(10)
        .align_x(Alignment::Center),
    )
    .center_x(Length::Fill)
    .padding(40)
    .into()
}

/// The full empty state: a large muted glyph, a title, a hint, and — where there
/// is something to *do* about it — one accent button.
///
/// A single line of grey text in the middle of an otherwise empty panel says
/// what is happening but offers no way out of it. This is the shape the macOS
/// client uses everywhere a panel can be legitimately empty.
pub fn empty_state_full<'a, M: Clone + 'a>(
    glyph: Icon,
    title: String,
    hint: String,
    action: Option<(String, M)>,
    palette: Palette,
) -> Element<'a, M> {
    let mut stack = column![
        icon(glyph, 34.0, palette.text_muted),
        text(title)
            .size(scale::BODY)
            .font(moonlight_design::ui(EMPHATIC))
            .color(palette.text),
        text(hint)
            .size(scale::META)
            .color(palette.text_muted)
            .align_x(Alignment::Center),
    ]
    .spacing(12)
    .align_x(Alignment::Center);

    if let Some((label, message)) = action {
        stack = stack.push(
            button(centre(
                text(label)
                    .size(13.0)
                    .font(moonlight_design::ui(EMPHATIC))
                    .color(palette.text_on_accent),
            ))
            .on_press(message)
            .padding([0, 18])
            .height(Length::Fixed(38.0))
            .style(move |_, status| theme::accent_button(palette, status)),
        );
    }

    container(stack)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .padding([28, 12])
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_toggle_knob_travels_the_distance_the_source_animates() {
        // The composition slides the knob by exactly `translateX(18px)`. That
        // number is not free — it is what the track's own geometry leaves once
        // the knob and its two insets are taken out. A 48px track (which an
        // earlier pass had) leaves 22 and the knob overshoots its own end.
        let travel = TOGGLE_W - 2.0 * TOGGLE_INSET - TOGGLE_KNOB;
        assert_eq!(travel, 18.0);
    }

    #[test]
    fn the_knob_fits_the_track_with_its_inset_on_both_edges() {
        assert_eq!(TOGGLE_H - 2.0 * TOGGLE_INSET, TOGGLE_KNOB);
    }

    #[test]
    fn a_row_title_sits_below_the_body_step_and_its_sub_line_below_that() {
        // 14.5/12, the pair the composition sets — not the 15/12.5 that the
        // emphatic body steps would give. Rows set at the body step read as a
        // stack of buttons rather than as a list.
        assert_eq!(ROW_TITLE_SIZE, 14.5);
        assert_eq!(SUB_SIZE, 12.0);
        assert_eq!(scale::BODY, 15.0);
        assert_eq!(scale::META, 12.5);
    }

    #[test]
    fn an_app_tile_keeps_its_colour_across_rescans() {
        // The inventory is rebuilt on every scan; a fill that moved with it
        // would read as the list re-sorting itself.
        let palette = Palette::DARK;
        assert_eq!(
            tile_fill("chrome.exe", palette),
            tile_fill("chrome.exe", palette)
        );
    }

    #[test]
    fn an_app_tile_only_ever_takes_a_category_colour() {
        // Never the accent: a column of lime tiles loses the only thing telling
        // one row from the next.
        let palette = Palette::DARK;
        let fills = category_fills(palette);
        for executable in [
            "chrome.exe",
            "Telegram.exe",
            "steam.exe",
            "Code.exe",
            "7zFM.exe",
            "javacpl.exe",
        ] {
            let fill = tile_fill(executable, palette);
            assert!(
                fills.contains(&fill),
                "{executable} took a colour outside the category ramp"
            );
        }
    }

    #[test]
    fn the_category_ramp_spreads_across_a_real_inventory() {
        // A hash that collapsed onto one colour would compile and look wrong.
        let palette = Palette::DARK;
        let names = [
            "chrome.exe",
            "Telegram.exe",
            "steam.exe",
            "Code.exe",
            "7zFM.exe",
            "javacpl.exe",
            "Spotify.exe",
            "zoom.exe",
            "explorer.exe",
            "notepad.exe",
        ];
        let mut seen: Vec<Color> = Vec::new();
        for name in names {
            let fill = tile_fill(name, palette);
            if !seen.contains(&fill) {
                seen.push(fill);
            }
        }
        assert!(
            seen.len() >= 3,
            "ten programmes produced only {} distinct tile colours",
            seen.len()
        );
    }

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
