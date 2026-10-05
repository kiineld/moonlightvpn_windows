//! moonlight colour system — black and white.
//!
//! The interface is monochrome, as the macOS client's `Palette.swift` is: a
//! black canvas, near-black surfaces told apart by a step of grey and a
//! hairline, white type, and white as the one interactive colour — black, in
//! the light theme. Colour is spent in exactly two places: the logo's lime,
//! which is the brand and appears nowhere else ([`Palette::brand`]), and the
//! small signals that carry meaning — latency, errors, log levels.
//!
//! The token names are the ones every screen was written against; what they
//! resolve to is what changed. `accent` fills, `accent_ink` is the accent as
//! type or a glyph, `text_on_accent` sits on an accent fill.
//!
//! The surfaces are glass, not slabs. The macOS client's solid surface colours
//! are only its fallback for systems without Liquid Glass; what it ships is a
//! translucent sheet, lit from above and rimmed. So `surface` and its steps are
//! washes over whatever is behind — the canvas, and through it the desktop —
//! with `surface_lit` the same sheet where the light catches its head. What
//! has to hide what is under it (a menu, a dialog, the sidebar) takes `raised`
//! or `rail`, which are solid.

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
    // Brand — the logo, and nothing else
    pub brand: Color,
    pub brand_ink: Color,
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
    /// A glass surface's head, where the light falls on it.
    pub surface_lit: Color,
    /// The sidebar: structure, so the heavier, solid material.
    pub rail: Color,
    /// Solid, for what floats over the page and must hide it.
    pub raised: Color,
    /// The canvas where the light comes in, at the window's top.
    pub bg_lit: Color,
    /// The shadow a floating surface drops.
    pub shade: Color,

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
    /// The sidebar's selected row: a step up from the panel it sits on.
    pub selection: Color,

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
        brand: hex(0xD2FF1F),
        brand_ink: hex(0x0A0A0A),

        lime: hex(0xD2FF1F),
        lime_deep: hex(0xC2F015),
        purple: hex(0x8A8A8A),
        yellow: hex(0xFFD60A),
        blue: hex(0xA3A3A3),
        orange: hex(0xFF9F0A),
        red: hex(0xFF453A),

        lime_wash: hexa(0xFFFFFF, 0.08),
        lime_wash_soft: hexa(0xFFFFFF, 0.04),
        red_wash: hexa(0xFF453A, 0.14),
        ink_wash: hexa(0x000000, 0.10),
        ink_wash_soft: hexa(0x000000, 0.05),
        hairline: hexa(0xFFFFFF, 0.10),
        hairline_soft: hexa(0xFFFFFF, 0.06),

        bg: hex(0x0A0A0A),
        bg_deep: hex(0x000000),
        surface: hexa(0xFFFFFF, 0.085),
        surface2: hexa(0xFFFFFF, 0.10),
        surface3: hexa(0xFFFFFF, 0.17),
        surface_nav: hex(0x0A0A0A),
        surface_lit: hexa(0xFFFFFF, 0.15),
        rail: hex(0x121212),
        raised: hex(0x1C1C1C),
        bg_lit: hex(0x1B1B1B),
        shade: hexa(0x000000, 0.0),

        text: hex(0xF5F5F5),
        text2: hex(0xA3A3A3),
        text_muted: hex(0x737373),
        text_on_accent: hex(0x000000),
        text_link: hex(0xF5F5F5),
        text_link_hover: hex(0xFFFFFF),

        accent: hex(0xFFFFFF),
        accent_hover: hex(0xE5E5E5),
        accent_quiet: hexa(0xFFFFFF, 0.08),
        accent_ink: hex(0xFFFFFF),
        accent_ink_strong: hex(0xFFFFFF),
        accent_line: hexa(0xFFFFFF, 0.6),
        selection: hexa(0xFFFFFF, 0.14),

        status_secure: hex(0xFFFFFF),
        danger: hex(0xFF453A),
        danger_quiet: hexa(0xFF453A, 0.14),
        warning: hex(0xFFD60A),
        info: hex(0xA3A3A3),

        // Tiles behind a glyph: all one quiet glass now.
        cat1: hexa(0xFFFFFF, 0.10),
        cat2: hexa(0xFFFFFF, 0.10),
        cat3: hexa(0xFFFFFF, 0.10),
        cat4: hexa(0xFFFFFF, 0.10),
        cat5: hexa(0xFFFFFF, 0.10),
        hero_gold: hex(0xFFFFFF),

        st_up: hex(0x30D158),
        st_up_ink: hex(0x30D158),
        st_degraded: hex(0xFFD60A),
        st_degraded_ink: hex(0xFFD60A),
        st_maintenance: hex(0xA3A3A3),
        st_maintenance_ink: hex(0xA3A3A3),
        st_partial: hex(0xFF9F0A),
        st_partial_ink: hex(0xFF9F0A),
        st_down: hex(0xFF453A),
        st_down_ink: hex(0xFF453A),

        telegram_blue: hex(0x29A0DA),
    };

    pub const LIGHT: Palette = Palette {
        brand: hex(0xD2FF1F),
        brand_ink: hex(0x0A0A0A),

        lime: hex(0xD2FF1F),
        lime_deep: hex(0xC2F015),
        purple: hex(0x737373),
        yellow: hex(0xB58900),
        blue: hex(0x525252),
        orange: hex(0xC2410C),
        red: hex(0xD70015),

        lime_wash: hexa(0x000000, 0.06),
        lime_wash_soft: hexa(0x000000, 0.03),
        red_wash: hexa(0xD70015, 0.10),
        ink_wash: hexa(0xFFFFFF, 0.16),
        ink_wash_soft: hexa(0xFFFFFF, 0.08),
        hairline: hexa(0x000000, 0.10),
        hairline_soft: hexa(0x000000, 0.06),

        // The canvas is the off-white and the surfaces are the white: a
        // sheet of glass over a light desk is brighter than the desk.
        bg: hex(0xE9E9E6),
        bg_deep: hex(0xFFFFFF),
        surface: hexa(0xFFFFFF, 0.86),
        surface2: hexa(0xFFFFFF, 0.90),
        surface3: hexa(0x000000, 0.08),
        surface_nav: hex(0xFAFAFA),
        surface_lit: hex(0xFFFFFF),
        rail: hex(0xFCFCFB),
        raised: hex(0xFFFFFF),
        bg_lit: hex(0xF6F6F3),
        shade: hexa(0x000000, 0.07),

        text: hex(0x0A0A0A),
        text2: hex(0x525252),
        text_muted: hex(0x8A8A8A),
        text_on_accent: hex(0xFFFFFF),
        text_link: hex(0x0A0A0A),
        text_link_hover: hex(0x000000),

        accent: hex(0x0A0A0A),
        accent_hover: hex(0x262626),
        accent_quiet: hexa(0x000000, 0.06),
        accent_ink: hex(0x0A0A0A),
        accent_ink_strong: hex(0x0A0A0A),
        accent_line: hexa(0x000000, 0.5),
        selection: hexa(0x000000, 0.065),

        status_secure: hex(0x0A0A0A),
        danger: hex(0xD70015),
        danger_quiet: hexa(0xD70015, 0.10),
        warning: hex(0xB58900),
        info: hex(0x525252),

        cat1: hexa(0xFFFFFF, 0.90),
        cat2: hexa(0xFFFFFF, 0.90),
        cat3: hexa(0xFFFFFF, 0.90),
        cat4: hexa(0xFFFFFF, 0.90),
        cat5: hexa(0xFFFFFF, 0.90),
        hero_gold: hex(0x0A0A0A),

        st_up: hex(0x248A3D),
        st_up_ink: hex(0x248A3D),
        st_degraded: hex(0xB58900),
        st_degraded_ink: hex(0xB58900),
        st_maintenance: hex(0x525252),
        st_maintenance_ink: hex(0x525252),
        st_partial: hex(0xC2410C),
        st_partial_ink: hex(0xC2410C),
        st_down: hex(0xD70015),
        st_down_ink: hex(0xD70015),

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
            brand,
            brand_ink,
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
            surface_lit,
            rail,
            raised,
            bg_lit,
            shade,
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
            selection,
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

    /// Latency as a signal: fine, usable, slow. Tuned to what a tunnel out of
    /// Russia actually measures — 40/100 ms steps painted every server there
    /// the slow colour, which told nobody anything.
    pub fn ping_color(&self, ms: u32) -> Color {
        if ms < 150 {
            self.st_up_ink
        } else if ms < 300 {
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
    fn the_interface_is_black_and_white_and_the_lime_is_the_logos_alone() {
        // White is the one interactive colour in the dark theme, black in the
        // light one; the brand's lime is the same in both and nothing else
        // carries it.
        assert_eq!(Palette::DARK.accent, hex(0xFFFFFF));
        assert_eq!(Palette::LIGHT.accent, hex(0x0A0A0A));
        assert_eq!(Palette::DARK.brand, Palette::LIGHT.brand);
        for palette in [Palette::DARK, Palette::LIGHT] {
            assert_ne!(palette.accent, palette.brand);
            assert_ne!(palette.bg, palette.brand);
        }
    }

    #[test]
    fn a_lerp_lands_exactly_on_its_endpoints() {
        // Anything else leaves the theme fractionally wrong once the fade ends,
        // for as long as the app stays open.
        assert_eq!(
            Palette::lerp(&Palette::DARK, &Palette::LIGHT, 0.0),
            Palette::DARK
        );
        assert_eq!(
            Palette::lerp(&Palette::DARK, &Palette::LIGHT, 1.0),
            Palette::LIGHT
        );
    }

    #[test]
    fn a_lerp_is_clamped_outside_the_unit_range() {
        assert_eq!(
            Palette::lerp(&Palette::DARK, &Palette::LIGHT, -3.0),
            Palette::DARK
        );
        assert_eq!(
            Palette::lerp(&Palette::DARK, &Palette::LIGHT, 9.0),
            Palette::LIGHT
        );
    }

    #[test]
    fn a_half_lerp_sits_between_the_two_backgrounds() {
        // Near-black to off-white: the midpoint must be neither end.
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
    fn glass_is_a_wash_and_what_must_hide_the_page_is_solid() {
        // A card lets the canvas through; a menu, a dialog and the sidebar's
        // tab — which is laid over the sidebar's own edge — may not.
        for palette in [Palette::DARK, Palette::LIGHT] {
            assert!(palette.surface.a < 1.0);
            assert!(palette.surface2.a < 1.0);
            assert_eq!(palette.raised.a, 1.0);
            assert_eq!(palette.rail.a, 1.0);
        }
    }

    #[test]
    fn the_light_canvas_is_darker_than_what_sits_on_it() {
        // White glass on an off-white desk. The other way round — grey slabs
        // on a white page — is the flat fallback this replaced.
        let light = Palette::LIGHT;
        assert!(light.bg.r < light.raised.r);
        assert!(light.bg.r < light.rail.r);
        assert!(
            light.shade.a > 0.0,
            "a white sheet needs a shadow to have an edge"
        );
    }

    #[test]
    fn ping_colour_is_keyed_off_latency() {
        let p = Palette::DARK;
        assert_eq!(p.ping_color(12), p.st_up_ink);
        assert_eq!(p.ping_color(149), p.st_up_ink);
        assert_eq!(p.ping_color(150), p.st_degraded_ink);
        assert_eq!(p.ping_color(299), p.st_degraded_ink);
        assert_eq!(p.ping_color(300), p.st_partial_ink);
    }
}
