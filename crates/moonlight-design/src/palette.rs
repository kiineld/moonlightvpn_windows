//! Moonlight colour system — lime on slate.
//!
//! Mapped one-for-one from `tokens/colors.css`, and kept identical to the macOS
//! client's `Palette.swift` so the two read as the same product. Base values
//! first, then semantic aliases. Light mode is the same system flipped, with two
//! deliberate departures the source calls out:
//!
//! 1. The accent is **yellow**, not lime — acid lime on near-white neither fills
//!    nor reads. Ink type stays on it, so the accent is a bright fill in both.
//! 2. Category fills keep their dark-theme hues, because ink on a dark purple or
//!    red slab fails contrast.
//!
//! The accent splits into four roles that must stay distinct, because light mode
//! depends on it: [`Palette::accent`] fills, [`Palette::accent_ink`] is accent as
//! type or a glyph, [`Palette::accent_ink_strong`] is accent type sitting *on* an
//! accent wash, and [`Palette::accent_line`] is accent as a thin mark. In dark
//! mode all four coincide.

use iced::Color;

/// Builds a colour from a packed `0xRRGGBB` literal, so the tokens below can be
/// read against the source CSS without arithmetic.
pub const fn hex(value: u32) -> Color {
    Color {
        r: ((value >> 16) & 0xFF) as f32 / 255.0,
        g: ((value >> 8) & 0xFF) as f32 / 255.0,
        b: (value & 0xFF) as f32 / 255.0,
        a: 1.0,
    }
}

/// The same, with an alpha. Washes and hairlines are all defined this way.
pub const fn hexa(value: u32, alpha: f32) -> Color {
    Color {
        a: alpha,
        ..hex(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    // Accents
    pub lime: Color,
    pub lime_deep: Color,
    pub purple: Color,
    pub yellow: Color,
    pub blue: Color,
    pub orange: Color,
    pub red: Color,

    // Washes + hairlines
    pub lime_wash: Color,
    pub lime_wash_soft: Color,
    pub red_wash: Color,
    pub ink_wash: Color,
    pub ink_wash_soft: Color,
    pub hairline: Color,
    pub hairline_soft: Color,

    // Surfaces
    pub bg: Color,
    pub bg_deep: Color,
    pub surface: Color,
    pub surface2: Color,
    pub surface3: Color,
    pub surface_nav: Color,

    // Text
    pub text: Color,
    pub text2: Color,
    pub text_muted: Color,
    pub text_on_accent: Color,
    pub text_link: Color,
    pub text_link_hover: Color,

    // Interactive
    pub accent: Color,
    pub accent_hover: Color,
    pub accent_quiet: Color,
    pub accent_ink: Color,
    pub accent_ink_strong: Color,
    pub accent_line: Color,

    // Status
    pub status_secure: Color,
    pub danger: Color,
    pub danger_quiet: Color,
    pub warning: Color,
    pub info: Color,

    // Category fills
    pub cat1: Color,
    pub cat2: Color,
    pub cat3: Color,
    pub cat4: Color,
    pub cat5: Color,
    pub hero_gold: Color,

    // Service-status severities.
    //
    // Two roles per state. `*_ink` is the readable one (pill text, dots, bars —
    // anything drawn ON the page). The plain token is a solid fill that always
    // carries #101828 text, so it stays light in both themes.
    pub st_up: Color,
    pub st_up_ink: Color,
    pub st_degraded: Color,
    pub st_degraded_ink: Color,
    pub st_maintenance: Color,
    pub st_maintenance_ink: Color,
    pub st_partial: Color,
    pub st_partial_ink: Color,
    pub st_down: Color,
    pub st_down_ink: Color,

    /// Telegram brand blue — the one third-party colour in the system.
    pub telegram_blue: Color,
}

impl Palette {
    pub const DARK: Palette = Palette {
        lime: hex(0xD2FF1F),
        lime_deep: hex(0xC2F015),
        purple: hex(0xAB93E1),
        yellow: hex(0xFFE078),
        blue: hex(0xB6CAEB),
        orange: hex(0xFB7A54),
        red: hex(0xFF6B5A),

        lime_wash: hexa(0xD2FF1F, 0.13),
        lime_wash_soft: hexa(0xD2FF1F, 0.06),
        red_wash: hexa(0xFF6B5A, 0.13),
        ink_wash: hexa(0x101828, 0.14),
        ink_wash_soft: hexa(0x101828, 0.06),
        hairline: hexa(0xFFFFFF, 0.09),
        hairline_soft: hexa(0xFFFFFF, 0.05),

        bg: hex(0x101828),
        bg_deep: hex(0x0B111E),
        surface: hex(0x182131),
        surface2: hex(0x212B3B),
        surface3: hex(0x2A3547),
        surface_nav: hexa(0x182131, 0.92),

        text: hex(0xFFFFFF),
        text2: hex(0xAEB7C7),
        text_muted: hex(0x878EA8),
        text_on_accent: hex(0x101828),
        text_link: hex(0xD2FF1F),
        text_link_hover: hex(0xE4FF6A),

        accent: hex(0xD2FF1F),
        accent_hover: hex(0xC2F015),
        accent_quiet: hexa(0xD2FF1F, 0.13),
        accent_ink: hex(0xD2FF1F),
        accent_ink_strong: hex(0xD2FF1F),
        accent_line: hex(0xD2FF1F),

        status_secure: hex(0xD2FF1F),
        danger: hex(0xFF6B5A),
        danger_quiet: hexa(0xFF6B5A, 0.13),
        warning: hex(0xFFE078),
        info: hex(0xB6CAEB),

        cat1: hex(0xD2FF1F),
        cat2: hex(0xAB93E1),
        cat3: hex(0xB6CAEB),
        cat4: hex(0xFFE078),
        cat5: hex(0xFB7A54),
        hero_gold: hex(0xEFAE2E),

        st_up: hex(0xD2FF1F),
        st_up_ink: hex(0xD2FF1F),
        st_degraded: hex(0xFFE078),
        st_degraded_ink: hex(0xFFE078),
        st_maintenance: hex(0xB6CAEB),
        st_maintenance_ink: hex(0xB6CAEB),
        st_partial: hex(0xFB7A54),
        st_partial_ink: hex(0xFB7A54),
        st_down: hex(0xFF6B5A),
        st_down_ink: hex(0xFF6B5A),

        telegram_blue: hex(0x29A0DA),
    };

    pub const LIGHT: Palette = Palette {
        lime: hex(0xFFE078),
        lime_deep: hex(0xF5CE52),
        purple: hex(0xAB93E1),
        yellow: hex(0xFFE078),
        blue: hex(0xB6CAEB),
        orange: hex(0xFB7A54),
        red: hex(0xFF6B5A),

        lime_wash: hexa(0xB07908, 0.16),
        lime_wash_soft: hexa(0xB07908, 0.07),
        red_wash: hexa(0xFF6B5A, 0.13),
        ink_wash: hexa(0x101828, 0.14),
        ink_wash_soft: hexa(0x101828, 0.06),
        hairline: hexa(0x101828, 0.11),
        hairline_soft: hexa(0x101828, 0.06),

        bg: hex(0xF2F3ED),
        bg_deep: hex(0xE6E8DF),
        surface: hex(0xFFFFFF),
        surface2: hex(0xF1F3EB),
        surface3: hex(0xE1E4D9),
        surface_nav: hexa(0xFFFFFF, 0.92),

        text: hex(0x101828),
        text2: hex(0x475467),
        text_muted: hex(0x667085),
        text_on_accent: hex(0x101828),
        text_link: hex(0x7A5600),
        text_link_hover: hex(0x5E4200),

        accent: hex(0xFFE078),
        accent_hover: hex(0xF5CE52),
        accent_quiet: hexa(0xB07908, 0.16),
        accent_ink: hex(0xEFAE2E),
        accent_ink_strong: hex(0x6B4A00),
        accent_line: hex(0xEFAE2E),

        status_secure: hex(0xFFE078),
        danger: hex(0xFF6B5A),
        danger_quiet: hexa(0xFF6B5A, 0.13),
        warning: hex(0x9A6A00),
        info: hex(0xB6CAEB),

        // cat-4 is deepened so the yellow category stays distinct from the
        // now-yellow accent.
        cat1: hex(0xFFE078),
        cat2: hex(0xAB93E1),
        cat3: hex(0xB6CAEB),
        cat4: hex(0xEFAE2E),
        cat5: hex(0xFB7A54),
        hero_gold: hex(0xFFE078),

        st_up: hex(0xC2EA45),
        st_up_ink: hex(0x4C7A0F),
        st_degraded: hex(0xFFD75C),
        st_degraded_ink: hex(0x9A6A00),
        st_maintenance: hex(0xAFC9EE),
        st_maintenance_ink: hex(0x3D6392),
        st_partial: hex(0xFB9B7C),
        st_partial_ink: hex(0xC2410C),
        st_down: hex(0xFF8A7A),
        st_down_ink: hex(0xB42318),

        telegram_blue: hex(0x29A0DA),
    };

    /// A palette part-way between two others, for the theme cross-fade.
    ///
    /// Every field is interpolated, including the washes and hairlines — a
    /// half-faded theme that kept the old hairlines would show the seams of the
    /// layout moving between two colour schemes.
    ///
    /// Interpolation is in straight sRGB. It is not perceptually uniform, and a
    /// slow fade between distant hues would show it; over 200ms between two
    /// palettes that share a structure it is indistinguishable from the right
    /// answer and costs no colour-space conversion per frame per field.
    pub fn lerp(from: &Palette, to: &Palette, t: f32) -> Palette {
        let t = t.clamp(0.0, 1.0);
        // Pinned rather than trusted to the arithmetic: `a + (b - a) * 1.0` is
        // not exactly `b` in binary floating point, so a fade left to run its
        // course would settle a bit-or-two off the theme it was heading for and
        // stay there for the life of the process.
        if t == 0.0 {
            return *from;
        }
        if t == 1.0 {
            return *to;
        }
        let mix = |a: Color, b: Color| Color {
            r: a.r + (b.r - a.r) * t,
            g: a.g + (b.g - a.g) * t,
            b: a.b + (b.b - a.b) * t,
            a: a.a + (b.a - a.a) * t,
        };

        macro_rules! blend {
            ($($field:ident),+ $(,)?) => {
                Palette { $($field: mix(from.$field, to.$field)),+ }
            };
        }

        blend!(
            lime,
            lime_deep,
            purple,
            yellow,
            blue,
            orange,
            red,
            lime_wash,
            lime_wash_soft,
            red_wash,
            ink_wash,
            ink_wash_soft,
            hairline,
            hairline_soft,
            bg,
            bg_deep,
            surface,
            surface2,
            surface3,
            surface_nav,
            text,
            text2,
            text_muted,
            text_on_accent,
            text_link,
            text_link_hover,
            accent,
            accent_hover,
            accent_quiet,
            accent_ink,
            accent_ink_strong,
            accent_line,
            status_secure,
            danger,
            danger_quiet,
            warning,
            info,
            cat1,
            cat2,
            cat3,
            cat4,
            cat5,
            hero_gold,
            st_up,
            st_up_ink,
            st_degraded,
            st_degraded_ink,
            st_maintenance,
            st_maintenance_ink,
            st_partial,
            st_partial_ink,
            st_down,
            st_down_ink,
            telegram_blue,
        )
    }

    /// The design keys ping colour off latency, not off a status enum.
    pub fn ping_color(&self, ms: u32) -> Color {
        if ms < 40 {
            self.st_up_ink
        } else if ms < 100 {
            self.st_degraded_ink
        } else {
            self.st_partial_ink
        }
    }
}

/// Which of the two palettes is in force.
///
/// `System` is resolved once at launch and on every window redraw against the
/// OS setting, so a user switching Windows to light mode mid-session is
/// followed rather than requiring a restart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Appearance {
    #[default]
    System,
    Dark,
    Light,
}

impl Appearance {
    pub fn palette(self, system_is_dark: bool) -> Palette {
        match self {
            Appearance::Dark => Palette::DARK,
            Appearance::Light => Palette::LIGHT,
            Appearance::System => {
                if system_is_dark {
                    Palette::DARK
                } else {
                    Palette::LIGHT
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_unpacks_channels() {
        let c = hex(0xD2FF1F);
        assert_eq!((c.r * 255.0).round() as u32, 0xD2);
        assert_eq!((c.g * 255.0).round() as u32, 0xFF);
        assert_eq!((c.b * 255.0).round() as u32, 0x1F);
        assert_eq!(c.a, 1.0);
    }

    #[test]
    fn the_four_accent_roles_diverge_in_light_mode() {
        // Dark collapses them; light must not, or accent type on an accent wash
        // disappears.
        let d = Palette::DARK;
        assert_eq!(d.accent, d.accent_ink);
        assert_eq!(d.accent, d.accent_ink_strong);

        let l = Palette::LIGHT;
        assert_ne!(l.accent, l.accent_ink);
        assert_ne!(l.accent_ink, l.accent_ink_strong);
    }

    #[test]
    fn a_lerp_lands_exactly_on_its_endpoints() {
        // Anything else leaves the theme fractionally wrong once the fade ends,
        // for as long as the app stays open.
        assert_eq!(Palette::lerp(&Palette::DARK, &Palette::LIGHT, 0.0), Palette::DARK);
        assert_eq!(Palette::lerp(&Palette::DARK, &Palette::LIGHT, 1.0), Palette::LIGHT);
    }

    #[test]
    fn a_lerp_is_clamped_outside_the_unit_range() {
        assert_eq!(Palette::lerp(&Palette::DARK, &Palette::LIGHT, -3.0), Palette::DARK);
        assert_eq!(Palette::lerp(&Palette::DARK, &Palette::LIGHT, 9.0), Palette::LIGHT);
    }

    #[test]
    fn a_half_lerp_sits_between_the_two_backgrounds() {
        // #101828 to #F2F3ED: the midpoint must be neither end.
        let middle = Palette::lerp(&Palette::DARK, &Palette::LIGHT, 0.5);
        assert!(middle.bg.r > Palette::DARK.bg.r);
        assert!(middle.bg.r < Palette::LIGHT.bg.r);
    }

    #[test]
    fn a_lerp_carries_the_translucent_tokens_too() {
        // The washes and hairlines have alphas below 1; a blend that only moved
        // the colour channels would hold the old theme's seams through the fade.
        let middle = Palette::lerp(&Palette::DARK, &Palette::LIGHT, 0.5);
        let (dark, light) = (Palette::DARK.hairline, Palette::LIGHT.hairline);
        assert!(dark.a < 1.0 && light.a < 1.0);
        assert!((middle.hairline.a - (dark.a + light.a) / 2.0).abs() < 1e-6);
    }

    #[test]
    fn ping_colour_is_keyed_off_latency() {
        let p = Palette::DARK;
        assert_eq!(p.ping_color(12), p.st_up_ink);
        assert_eq!(p.ping_color(40), p.st_degraded_ink);
        assert_eq!(p.ping_color(99), p.st_degraded_ink);
        assert_eq!(p.ping_color(100), p.st_partial_ink);
    }
}
