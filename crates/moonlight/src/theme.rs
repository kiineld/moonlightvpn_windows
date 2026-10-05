//! Bridges the design tokens onto iced's styling.
//!
//! iced hands a `&Theme` to every style closure, and its built-in palette has
//! five roles where this design has forty. Rather than bend the tokens to fit,
//! the app carries its own [`Palette`] alongside and the style helpers below
//! take it explicitly. `Theme::Dark` is still set on the application so the
//! text-input caret and selection colours are sane, but nothing else reads it.
//!
//! The surfaces are glass, as the macOS client's are: [`glass`] for a sheet
//! big enough to be lit from above, [`control`] for the small pieces on it,
//! and [`floating`] — solid — for what has to hide the page under it.

use iced::border::Radius;
use iced::widget::{button, container, overlay, pick_list, scrollable, text_input};
use iced::{Background, Border, Color, Gradient, Radians, Shadow, Theme, Vector};

use moonlight_design::motion::{border, radii};
use moonlight_design::Palette;

/// A colour at a given alpha, for the hover and press washes.
pub fn alpha(color: Color, a: f32) -> Color {
    Color { a, ..color }
}

/// One translucent colour laid over another, as one colour. For a hover wash
/// on glass: a button has one fill, and the wash and the sheet are two.
pub fn over(top: Color, under: Color) -> Color {
    let a = top.a + under.a * (1.0 - top.a);
    if a <= 0.0 {
        return Color::TRANSPARENT;
    }
    let mix = |t: f32, u: f32| (t * top.a + u * under.a * (1.0 - top.a)) / a;
    Color {
        r: mix(top.r, under.r),
        g: mix(top.g, under.g),
        b: mix(top.b, under.b),
        a,
    }
}

/// The page background.
pub fn page(palette: Palette) -> container::Style {
    container::Style {
        background: Some(Background::Color(palette.bg)),
        text_color: Some(palette.text),
        ..Default::default()
    }
}

/// `head` at the top, settling to `body` by `reach` of the way down.
fn lit(head: Color, body: Color, reach: f32) -> Background {
    Background::Gradient(Gradient::Linear(
        iced::gradient::Linear::new(Radians(std::f32::consts::PI))
            .add_stop(0.0, head)
            .add_stop(reach, body)
            .add_stop(1.0, body),
    ))
}

/// A window's canvas, with the light falling in from the top as the macOS
/// client draws it. `strength` is how heavily it is laid: solid at 1, and
/// less over one of Windows 11's materials, so the material has light to
/// show — as that client lays its canvas over the blurred desktop.
pub fn canvas(palette: Palette, strength: f32) -> container::Style {
    let mut style = page(palette);
    style.background = Some(lit(
        alpha(palette.bg_lit, strength),
        alpha(palette.bg, strength),
        0.6,
    ));
    style
}

/// The shadow under a floating surface, `blur` wide. Nothing in the dark
/// theme, where a shadow on black is not there to be seen.
pub fn lift(palette: Palette, blur: f32) -> Shadow {
    Shadow {
        color: palette.shade,
        offset: Vector::new(0.0, blur * 0.3),
        blur_radius: blur,
    }
}

/// Glass, as the macOS client's surfaces are: a translucent sheet lit from
/// above, rimmed with a hairline, and — where the palette has one — dropping
/// a soft shadow. In the light theme the sheet is white on an off-white
/// canvas, and the shadow is most of what gives it an edge.
pub fn glass(palette: Palette, radius: f32) -> container::Style {
    container::Style {
        background: Some(lit(palette.surface_lit, palette.surface, 0.35)),
        text_color: Some(palette.text),
        border: Border {
            radius: Radius::from(radius),
            width: border::HAIRLINE,
            color: palette.hairline,
        },
        shadow: lift(palette, 22.0),
        ..Default::default()
    }
}

/// A small piece of glass: a control's track, a tile, a chip. The sheet
/// without the lit head — these are too short for a falloff to read — and its
/// rim.
pub fn control(palette: Palette, radius: f32) -> container::Style {
    container::Style {
        background: Some(Background::Color(palette.surface2)),
        text_color: Some(palette.text),
        border: Border {
            radius: Radius::from(radius),
            width: border::HAIRLINE,
            color: palette.hairline,
        },
        ..Default::default()
    }
}

/// What floats over the page and has to hide it: a dialog, a tooltip, the
/// update banner. Solid, rimmed, and shadowed in both themes — over a dimmed
/// page the shadow is what lifts it.
pub fn floating(palette: Palette, radius: f32) -> container::Style {
    container::Style {
        background: Some(Background::Color(palette.raised)),
        text_color: Some(palette.text),
        border: Border {
            radius: Radius::from(radius),
            width: border::HAIRLINE,
            color: palette.hairline,
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.28),
            offset: Vector::new(0.0, 10.0),
            blur_radius: 32.0,
        },
        ..Default::default()
    }
}

/// A raised panel — a page's cards, the server list.
pub fn panel(palette: Palette) -> container::Style {
    glass(palette, radii::PANEL)
}

/// A card inside a panel — the stats strip, the traffic block. Glass on
/// glass, so only its rim and no second shadow.
pub fn card(palette: Palette) -> container::Style {
    control(palette, radii::CARD_SM)
}

/// A list row that can be selected.
// Unused until the Apps and Connections lists is wired up.
#[allow(dead_code)]
pub fn row(palette: Palette, selected: bool) -> container::Style {
    container::Style {
        background: Some(Background::Color(if selected {
            palette.accent_quiet
        } else {
            Color::TRANSPARENT
        })),
        text_color: Some(palette.text),
        border: Border {
            radius: Radius::from(radii::ROW),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        ..Default::default()
    }
}

/// The accent-filled button: the sidebar's active item, the primary action.
pub fn accent_button(palette: Palette, status: button::Status) -> button::Style {
    let background = match status {
        button::Status::Hovered | button::Status::Pressed => palette.accent_hover,
        _ => palette.accent,
    };
    button::Style {
        background: Some(Background::Color(background)),
        text_color: palette.text_on_accent,
        border: Border {
            radius: Radius::from(radii::PILL),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        shadow: Shadow::default(),
        ..Default::default()
    }
}

/// The header's action buttons — *Пинг*, *Обновить*.
///
/// A glass pill with a hairline, and the **border** is what changes on hover;
/// the fill stays put. That is the composition's rule, and it is what keeps a
/// row of these from flashing as the pointer crosses them.
///
/// The label takes `accent_ink`, not `text`: these are the two accent actions on
/// the page, and the design marks them by colouring the whole button rather than
/// only its glyph.
pub fn header_button(palette: Palette, status: button::Status) -> button::Style {
    let border_color = match status {
        button::Status::Hovered | button::Status::Pressed => palette.accent_line,
        _ => palette.hairline,
    };
    button::Style {
        background: Some(Background::Color(palette.surface2)),
        text_color: palette.accent_ink,
        border: Border {
            radius: Radius::from(radii::PILL),
            width: border::HAIRLINE,
            color: border_color,
        },
        shadow: lift(palette, 10.0),
        ..Default::default()
    }
}

/// A round glyph button. Glass with its rim and a text-2 glyph — it is not an
/// accent action, so it does not take the accent; under the pointer the sheet
/// brightens.
pub fn icon_button(palette: Palette, status: button::Status) -> button::Style {
    let wash = match status {
        button::Status::Hovered | button::Status::Pressed => alpha(palette.text, 0.08),
        _ => Color::TRANSPARENT,
    };
    button::Style {
        background: Some(Background::Color(over(wash, palette.surface2))),
        text_color: palette.text2,
        border: Border {
            radius: Radius::from(radii::PILL),
            width: border::HAIRLINE,
            color: palette.hairline,
        },
        shadow: lift(palette, 10.0),
        ..Default::default()
    }
}

/// A bordered button whose fill stays and whose border lifts — the sidebar quota
/// card and the settings actions.
pub fn outlined(palette: Palette, status: button::Status) -> button::Style {
    let border_color = match status {
        button::Status::Hovered | button::Status::Pressed => palette.accent_line,
        _ => palette.hairline,
    };
    button::Style {
        background: Some(Background::Color(palette.surface2)),
        text_color: palette.text,
        border: Border {
            radius: Radius::from(radii::CARD_SM),
            width: border::HAIRLINE,
            color: border_color,
        },
        shadow: Shadow::default(),
        ..Default::default()
    }
}

/// A sidebar item that is not the current page.
pub fn nav_button(palette: Palette, status: button::Status) -> button::Style {
    let background = match status {
        button::Status::Hovered => alpha(palette.text, 0.05),
        button::Status::Pressed => alpha(palette.text, 0.09),
        _ => Color::TRANSPARENT,
    };
    button::Style {
        background: Some(Background::Color(background)),
        text_color: palette.text2,
        border: Border {
            radius: Radius::from(radii::PILL),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        shadow: Shadow::default(),
        ..Default::default()
    }
}

/// A row that behaves as a button but must not look like one.
///
/// Selection is the quiet `selection` step, not the accent: what carries the
/// accent on a selected row is the row's *tile*, not its background. Under the
/// pointer a row takes only a wash of the text colour, lighter than the
/// selection, so the two never read as the same thing.
pub fn row_button(palette: Palette, selected: bool, status: button::Status) -> button::Style {
    let background = if selected {
        palette.selection
    } else {
        match status {
            button::Status::Hovered => alpha(palette.text, 0.05),
            button::Status::Pressed => alpha(palette.text, 0.08),
            _ => Color::TRANSPARENT,
        }
    };
    button::Style {
        background: Some(Background::Color(background)),
        text_color: palette.text,
        border: Border {
            radius: Radius::from(radii::ROW),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        shadow: Shadow::default(),
        ..Default::default()
    }
}

pub fn field(palette: Palette, status: text_input::Status) -> text_input::Style {
    let border_color = match status {
        text_input::Status::Focused { .. } => palette.accent_line,
        text_input::Status::Hovered => alpha(palette.text, 0.18),
        _ => palette.hairline,
    };
    text_input::Style {
        background: Background::Color(palette.surface2),
        border: Border {
            radius: Radius::from(radii::FIELD),
            width: 1.0,
            color: border_color,
        },
        icon: palette.text_muted,
        placeholder: palette.text_muted,
        value: palette.text,
        selection: alpha(palette.accent, 0.28),
    }
}

/// The rule-kind dropdown.
///
/// Without an explicit style iced falls back to whatever `Theme` the application
/// carries — which is `Theme::Dark` here, purely so the text-input caret is sane.
/// That painted a near-black slab with white type in the middle of a near-white
/// panel: the one control on the page that belonged to a different application.
pub fn picker(palette: Palette, status: pick_list::Status) -> pick_list::Style {
    let border_color = match status {
        pick_list::Status::Hovered | pick_list::Status::Opened { .. } => palette.accent_line,
        _ => palette.hairline,
    };
    pick_list::Style {
        text_color: palette.text,
        placeholder_color: palette.text_muted,
        handle_color: palette.text_muted,
        background: Background::Color(palette.surface2),
        border: Border {
            radius: Radius::from(radii::FIELD),
            width: border::HAIRLINE,
            color: border_color,
        },
    }
}

/// The dropdown's own list, which is a separate surface from the closed control.
pub fn picker_menu(palette: Palette) -> overlay::menu::Style {
    let floating = floating(palette, radii::FIELD);
    overlay::menu::Style {
        // Solid: a list that let the form under it show through its rows
        // could not be read.
        background: Background::Color(palette.raised),
        border: floating.border,
        text_color: palette.text,
        selected_text_color: palette.text_on_accent,
        selected_background: Background::Color(palette.accent),
        shadow: floating.shadow,
    }
}

pub fn scroller(palette: Palette, _theme: &Theme) -> scrollable::Style {
    let rail = scrollable::Rail {
        background: None,
        border: Border::default(),
        scroller: scrollable::Scroller {
            background: Background::Color(alpha(palette.text, 0.16)),
            border: Border {
                radius: Radius::from(radii::PILL),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
        },
    };
    scrollable::Style {
        container: container::Style::default(),
        vertical_rail: rail,
        horizontal_rail: rail,
        gap: None,
        // The drag-to-auto-scroll overlay. Given the surface colours rather
        // than left at a default that assumes iced's own palette.
        auto_scroll: scrollable::AutoScroll {
            background: Background::Color(palette.raised),
            border: Border {
                radius: Radius::from(radii::PILL),
                width: 1.0,
                color: palette.hairline,
            },
            shadow: Shadow::default(),
            icon: palette.text2,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_accent_button_carries_the_opposite_ink_in_both_palettes() {
        // A white fill with black type in the dark theme, a black fill with
        // white type in the light one — never the theme's own text colour,
        // which is the fill's own.
        for palette in [Palette::DARK, Palette::LIGHT] {
            let style = accent_button(palette, button::Status::Active);
            assert_eq!(style.text_color, palette.text_on_accent);
            assert_ne!(style.text_color, palette.accent);
            assert_ne!(style.text_color, palette.text);
        }
    }

    #[test]
    fn a_header_button_lifts_its_border_and_keeps_its_fill() {
        // The composition changes the border on hover, not the background —
        // which is what keeps a row of these from flashing as the pointer
        // crosses them.
        let active = header_button(Palette::DARK, button::Status::Active);
        let hovered = header_button(Palette::DARK, button::Status::Hovered);
        assert_eq!(active.background, hovered.background);
        assert_ne!(active.border.color, hovered.border.color);
        assert_eq!(hovered.border.color, Palette::DARK.accent_line);
        // And the geometry never moves.
        assert_eq!(active.border.radius, hovered.border.radius);
        assert_eq!(active.border.width, hovered.border.width);
    }

    #[test]
    fn a_header_button_labels_itself_in_the_accent() {
        assert_eq!(
            header_button(Palette::DARK, button::Status::Active).text_color,
            Palette::DARK.accent_ink
        );
    }

    #[test]
    fn a_glyph_button_is_not_an_accent_action() {
        // Glass with a text-2 glyph. Colouring it like Пинг and Обновить
        // would claim it does something to the tunnel.
        let style = icon_button(Palette::DARK, button::Status::Active);
        assert_eq!(style.text_color, Palette::DARK.text2);
        assert_eq!(
            style.background,
            Some(Background::Color(Palette::DARK.surface2))
        );
    }

    #[test]
    fn a_selected_row_is_the_quiet_step_and_never_the_accent() {
        // The accent on a selected row is carried by its tile.
        for palette in [Palette::DARK, Palette::LIGHT] {
            let selected = row_button(palette, true, button::Status::Active);
            assert_eq!(
                selected.background,
                Some(Background::Color(palette.selection))
            );
            assert_ne!(selected.background, Some(Background::Color(palette.accent)));
        }
    }

    #[test]
    fn a_hovered_row_is_quieter_than_a_selected_one() {
        let palette = Palette::DARK;
        let hovered = row_button(palette, false, button::Status::Hovered);
        let Some(Background::Color(wash)) = hovered.background else {
            panic!("a hovered row has a wash");
        };
        assert!(wash.a < palette.selection.a);
    }

    #[test]
    fn a_selected_row_ignores_hover_so_the_selection_does_not_flicker() {
        let hovered = row_button(Palette::DARK, true, button::Status::Hovered);
        let active = row_button(Palette::DARK, true, button::Status::Active);
        assert_eq!(hovered.background, active.background);
    }

    #[test]
    fn glass_is_rimmed_in_both_themes_and_shadowed_only_where_it_shows() {
        // The rim is the sheet's edge everywhere; the shadow is for the light
        // theme, where white on off-white has no other.
        for palette in [Palette::DARK, Palette::LIGHT] {
            let style = glass(palette, radii::PANEL);
            assert_eq!(style.border.width, border::HAIRLINE);
            assert!(matches!(style.background, Some(Background::Gradient(_))));
        }
        assert_eq!(glass(Palette::DARK, radii::PANEL).shadow.color.a, 0.0);
        assert!(glass(Palette::LIGHT, radii::PANEL).shadow.color.a > 0.0);
    }

    #[test]
    fn what_floats_over_the_page_hides_it() {
        // A dialog or a menu on translucent glass lets the page's own text
        // through its own.
        for palette in [Palette::DARK, Palette::LIGHT] {
            let Some(Background::Color(fill)) = floating(palette, radii::PANEL).background else {
                panic!("a floating surface is one solid colour");
            };
            assert_eq!(fill.a, 1.0);
            assert_eq!(picker_menu(palette).background, Background::Color(fill));
        }
    }

    #[test]
    fn a_wash_over_glass_is_one_colour_between_the_two() {
        let sheet = Color::from_rgba(1.0, 1.0, 1.0, 0.10);
        let wash = Color::from_rgba(1.0, 1.0, 1.0, 0.08);
        let both = over(wash, sheet);
        assert!((both.a - 0.172).abs() < 1e-4);
        assert_eq!(over(Color::TRANSPARENT, sheet), sheet);
    }

    #[test]
    fn a_card_on_a_panel_keeps_an_edge() {
        // Glass on glass: the rim is what tells the two apart.
        for palette in [Palette::DARK, Palette::LIGHT] {
            assert_ne!(panel(palette).background, card(palette).background);
            assert_eq!(card(palette).border.width, border::HAIRLINE);
        }
    }

    #[test]
    fn an_input_takes_the_field_radius_the_tokens_name() {
        assert_eq!(
            field(Palette::DARK, text_input::Status::Active)
                .border
                .radius,
            Radius::from(radii::FIELD)
        );
    }

    #[test]
    fn a_focused_field_takes_the_accent_line_role_not_the_fill() {
        // accent_line is the thin-mark role; using `accent` here would put a
        // 1px lime hairline at full fill weight around every input.
        let focused = field(
            Palette::LIGHT,
            text_input::Status::Focused { is_hovered: false },
        );
        assert_eq!(focused.border.color, Palette::LIGHT.accent_line);
        assert_ne!(focused.border.color, Palette::LIGHT.accent);
    }
}
