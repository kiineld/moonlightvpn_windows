//! Bridges the design tokens onto iced's styling.
//!
//! iced hands a `&Theme` to every style closure, and its built-in palette has
//! five roles where this design has forty. Rather than bend the tokens to fit,
//! the app carries its own [`Palette`] alongside and the style helpers below
//! take it explicitly. `Theme::Dark` is still set on the application so the
//! text-input caret and selection colours are sane, but nothing else reads it.

use iced::border::Radius;
use iced::widget::{button, container, scrollable, text_input};
use iced::{Background, Border, Color, Shadow, Theme, Vector};

use moonlight_design::motion::radii;
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

/// A card inside a panel — the stat strip, the quota block.
pub fn card(palette: Palette) -> container::Style {
    container::Style {
        background: Some(Background::Color(palette.surface2)),
        text_color: Some(palette.text),
        border: Border {
            radius: Radius::from(radii::CARD),
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

/// The quiet button: an outline that fills on hover. Hovers change colour and
/// border, never scale.
pub fn ghost_button(palette: Palette, status: button::Status) -> button::Style {
    let background = match status {
        button::Status::Hovered => alpha(palette.text, 0.06),
        button::Status::Pressed => alpha(palette.text, 0.10),
        _ => Color::TRANSPARENT,
    };
    button::Style {
        background: Some(Background::Color(background)),
        text_color: palette.text,
        border: Border {
            radius: Radius::from(radii::PILL),
            width: 1.0,
            color: palette.hairline,
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
pub fn row_button(palette: Palette, selected: bool, status: button::Status) -> button::Style {
    let background = if selected {
        palette.accent_quiet
    } else {
        match status {
            button::Status::Hovered => alpha(palette.text, 0.05),
            button::Status::Pressed => alpha(palette.text, 0.09),
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
    fn a_hover_changes_colour_and_never_geometry() {
        // The motion tokens allow a press to shrink; a hover may only repaint.
        let active = ghost_button(Palette::DARK, button::Status::Active);
        let hovered = ghost_button(Palette::DARK, button::Status::Hovered);
        assert_ne!(active.background, hovered.background);
        assert_eq!(active.border.radius, hovered.border.radius);
        assert_eq!(active.border.width, hovered.border.width);
    }

    #[test]
    fn a_selected_row_ignores_hover_so_the_selection_does_not_flicker() {
        let hovered = row_button(Palette::DARK, true, button::Status::Hovered);
        let active = row_button(Palette::DARK, true, button::Status::Active);
        assert_eq!(hovered.background, active.background);
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
