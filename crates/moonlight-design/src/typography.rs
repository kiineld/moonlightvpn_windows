//! Moonlight type — Onest carries every UI/body string, Unbounded is display
//! only (page titles, hero numbers, plan names, stat values, the wordmark).
//! Unbounded never appears below 15px and never in running text.
//!
//! Weights run heavy: 500 is the lightest body weight, 700 is a row title, 800
//! is the default for anything emphatic — labels, buttons, chips, numbers.
//!
//! The design ships `woff2`, which no native text stack reads, so
//! `scripts/fetch-fonts.ps1` downloads TTFs from Google Fonts into
//! `resources/fonts` and they are compiled into the binary with `include_bytes!`
//! — see [`FONT_BYTES`]. That is a deliberate departure from the macOS client,
//! which registers them from its bundle at launch: a single self-contained
//! `.exe` has nowhere to register *from*, and a portable build that silently
//! fell back to Segoe UI would not look like this product.
//!
//! **Static instances, not variable TTFs.** cosmic-text (the shaper underneath
//! iced) selects a face by weight from the font database; it does not set a
//! variable font's `wght` axis. A variable TTF therefore renders at its default
//! instance for every weight, which flattens the entire type hierarchy — the
//! 800 labels this design leans on come out looking like the 500 body. So each
//! weight is fetched as its own file.

use iced::font::{Family, Style, Weight};
use iced::Font;

pub const UI_FAMILY: &str = "Onest";
pub const DISPLAY_FAMILY: &str = "Unbounded";

/// Every face compiled into the binary, in load order.
///
/// Missing files are tolerated at build time through `include_bytes!` only if
/// the fetch script has run; `build.rs` writes a stub for each absent face so a
/// fresh clone still compiles and falls back to the system font.
pub const FONT_BYTES: &[&[u8]] = &[
    include_bytes!(concat!(env!("OUT_DIR"), "/Onest-Medium.ttf")),
    include_bytes!(concat!(env!("OUT_DIR"), "/Onest-Bold.ttf")),
    include_bytes!(concat!(env!("OUT_DIR"), "/Onest-ExtraBold.ttf")),
    include_bytes!(concat!(env!("OUT_DIR"), "/Unbounded-ExtraBold.ttf")),
];

/// Onest at a weight. The default body weight is 500 — the lightest the design
/// uses.
pub fn ui(weight: Weight) -> Font {
    Font {
        family: Family::Name(UI_FAMILY),
        weight,
        style: Style::Normal,
        ..Font::DEFAULT
    }
}

/// Unbounded. Display only — never below 15px, never in running text.
pub fn display() -> Font {
    Font {
        family: Family::Name(DISPLAY_FAMILY),
        weight: Weight::ExtraBold,
        style: Style::Normal,
        ..Font::DEFAULT
    }
}

/// The mono face carries timers, latency figures and the subscription URL.
///
/// Left as the platform monospace rather than bundled: on Windows that is
/// Consolas or Cascadia Mono, both of which have the tabular digits a ticking
/// `00:00:00` needs, and neither of which costs a megabyte in the binary.
pub fn mono() -> Font {
    Font::MONOSPACE
}

pub const BODY: Weight = Weight::Medium;
pub const ROW_TITLE: Weight = Weight::Bold;
pub const EMPHATIC: Weight = Weight::ExtraBold;

/// Type steps, named as the source names them.
pub mod scale {
    // Display steps (Unbounded, weight 800)
    pub const HERO: f32 = 40.0;
    pub const PLAN: f32 = 30.0;
    pub const TITLE: f32 = 24.0;
    pub const LEAD: f32 = 19.0;

    // Text steps (Onest)
    pub const BODY: f32 = 15.0;
    pub const BODY_SM: f32 = 14.0;
    pub const META: f32 = 12.5;
    pub const MICRO: f32 = 11.5;

    // Tracking, in `em` — the unit the source tokens use. Display type is
    // always negative-tracked, body is not.
    pub const TRACK_DISPLAY: f32 = -0.03;
    pub const TRACK_TITLE: f32 = -0.02;
    pub const TRACK_TIGHT: f32 = -0.01;
    pub const TRACK_OVERLINE: f32 = 0.1;
}

/// Tracking in `em` resolved against a size, because iced takes absolute
/// pixels where the source tokens are relative.
pub fn tracking(em: f32, size: f32) -> f32 {
    em * size
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_and_display_are_different_families() {
        assert_ne!(ui(BODY).family, display().family);
    }

    #[test]
    fn tracking_resolves_em_against_size() {
        assert!((tracking(scale::TRACK_DISPLAY, 40.0) - -1.2).abs() < 1e-5);
    }

    #[test]
    fn display_steps_never_fall_below_the_running_text_floor() {
        // Unbounded is not allowed under 15px anywhere in the system.
        for step in [scale::HERO, scale::PLAN, scale::TITLE, scale::LEAD] {
            assert!(step >= 15.0, "{step} is below the display floor");
        }
    }
}
