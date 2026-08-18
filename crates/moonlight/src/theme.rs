//! Bridges the design tokens onto iced's styling.
//!
//! iced hands a `&Theme` to every style closure, and its built-in palette has
//! five roles where this design has forty. Rather than bend the tokens to fit,
//! the app carries its own [`Palette`] alongside and the style helpers below
//! take it explicitly. `Theme::Dark` is still set on the application so the
//! text-input caret and selection colours are sane, but nothing else reads it.

use iced::border::Radius;
use iced::widget::{button, container, overlay, pick_list, scrollable, text_input};
use iced::{Background, Border, Color, Shadow, Theme, Vector};

use moonlight_design::motion::{border, radii};
use moonlight_design::Palette;

/// A colour at a given alpha, for the hover and press washes.
pub fn alpha(color: Color, a: f32) -> Color {
    Color { a, ..color }
}

/// The page background.
pub fn page(palette: Palette) -> container::Style {
    container::Style {
        background: Some(Background::Color(palette.bg)),
        text_color: Some(palette.text),
        ..Default::default()
    }
}

/// A raised panel — the two big columns on the connect screen.
pub fn panel(palette: Palette) -> container::Style {
    container::Style {
        background: Some(Background::Color(palette.surface)),
        text_color: Some(palette.text),
        border: Border {
            radius: Radius::from(radii::PANEL),
            width: 1.0,
            color: palette.hairline,
        },
        ..Default::default()
    }
}

/// A card inside a panel — the stats strip, the traffic block.
///
/// Surface-2 with **no border and no shadow**. Moonlight is a flat system:
/// elevation is carried by a surface's value, not by blur, so a card is a
/// lighter slab and nothing else.
pub fn card(palette: Palette) -> container::Style {
    container::Style {
        background: Some(Background::Color(palette.surface2)),
        text_color: Some(palette.text),
        border: Border {
            radius: Radius::from(radii::CARD_SM),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        ..Default::default()
    }
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
/// A surface fill with a hairline, and the **border** is what changes on hover;
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
        background: Some(Background::Color(palette.surface)),
        text_color: palette.accent_ink,
        border: Border {
            radius: Radius::from(radii::PILL),
            width: border::HAIRLINE,
            color: border_color,
        },
        shadow: Shadow::default(),
        ..Default::default()
    }
}

/// The round theme button. Surface-2, no border, and a text-2 glyph — it is not
/// an accent action, so it does not take the accent.
pub fn icon_button(palette: Palette, status: button::Status) -> button::Style {
    let background = match status {
        button::Status::Hovered => palette.surface3,
        _ => palette.surface2,
    };
    button::Style {
        background: Some(Background::Color(background)),
        text_color: palette.text2,
        border: Border {
            radius: Radius::from(radii::PILL),
            ..Default::default()
        },
        shadow: Shadow::default(),
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
        background: Some(Background::Color(palette.surface)),
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
/// Selection is **surface-2**, not the accent wash. 13% lime over the panel
/// composites to a dark olive, and the composition uses it nowhere: what carries
/// the accent on a selected row is the row's *tile*, not its background. Getting
/// this backwards is what made the server list look mossy.
pub fn row_button(palette: Palette, selected: bool, status: button::Status) -> button::Style {
    let background = if selected {
        palette.surface2
    } else {
        match status {
            // Hover is the same surface-2 the selection uses, which is why a
            // selected row reads as "already there" rather than as highlighted.
            button::Status::Hovered | button::Status::Pressed => palette.surface2,
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

// Unused until the Import screen's URL field is wired up.
#[allow(dead_code)]
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
    overlay::menu::Style {
        background: Background::Color(palette.surface2),
        border: Border {
            radius: Radius::from(radii::FIELD),
            width: border::HAIRLINE,
            color: palette.hairline,
        },
        text_color: palette.text,
        selected_text_color: palette.text_on_accent,
        selected_background: Background::Color(palette.accent),
        // Flat system: the menu is a lighter surface, not a floated one.
        shadow: Shadow::default(),
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
            background: Background::Color(palette.surface3),
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

/// A translucent shadow used under the connect dial when it is live.
// Unused until the connected dial is wired up.
#[allow(dead_code)]
pub fn glow(color: Color) -> Shadow {
    Shadow {
        color: alpha(color, 0.35),
        offset: Vector::ZERO,
        blur_radius: 40.0,
    }
}

/// The one glow in the system, reserved for the status dot and other tiny accent
/// marks — `--ml-glow-lime-sm`. Never for a panel: this is a flat system, where
/// elevation is a surface's value rather than a blur.
pub fn glow_sm(color: Color) -> Shadow {
    Shadow {
        color: alpha(color, 0.7),
        offset: Vector::ZERO,
        // 8, not 34. The macOS client sets this dot's shadow at radius 5, and
        // 34 spread the 8px dot into a soft green disc a third the width of the
        // ring — a blob, not a glow.
        blur_radius: 8.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_accent_button_carries_ink_type_in_both_palettes() {
        // The accent is a bright fill in both themes, so the text on it is
        // always #101828 — never the theme's own text colour, which in dark
        // mode is white on lime.
        for palette in [Palette::DARK, Palette::LIGHT] {
            let style = accent_button(palette, button::Status::Active);
            assert_eq!(style.text_color, palette.text_on_accent);
            assert_eq!(style.text_color, moonlight_design::palette::hex(0x101828));
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
    fn the_theme_button_is_not_an_accent_action() {
        // Surface-2 with a text-2 glyph. Colouring it like Пинг and Обновить
        // would claim it does something to the tunnel.
        let style = icon_button(Palette::DARK, button::Status::Active);
        assert_eq!(style.text_color, Palette::DARK.text2);
        assert_eq!(style.border.width, 0.0);
    }

    #[test]
    fn a_selected_row_is_surface_two_and_never_the_accent_wash() {
        // 13% lime over the panel composites to a dark olive, and the
        // composition uses it nowhere. The accent on a selected row is carried
        // by its tile.
        let selected = row_button(Palette::DARK, true, button::Status::Active);
        assert_eq!(
            selected.background,
            Some(Background::Color(Palette::DARK.surface2))
        );
        assert_ne!(
            selected.background,
            Some(Background::Color(Palette::DARK.accent_quiet))
        );
    }

    #[test]
    fn a_card_carries_no_border_and_no_shadow() {
        // Flat system: elevation is a surface value, not a blur.
        let style = card(Palette::DARK);
        assert_eq!(style.border.width, 0.0);
        assert_eq!(style.shadow.blur_radius, 0.0);
    }

    #[test]
    fn a_selected_row_ignores_hover_so_the_selection_does_not_flicker() {
        let hovered = row_button(Palette::DARK, true, button::Status::Hovered);
        let active = row_button(Palette::DARK, true, button::Status::Active);
        assert_eq!(hovered.background, active.background);
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

    #[test]
    fn panels_and_cards_sit_on_different_surfaces() {
        // A card on a panel of the same colour has no edge, and the design
        // never draws a border on a card.
        let palette = Palette::DARK;
        assert_ne!(panel(palette).background, card(palette).background);
    }
}
