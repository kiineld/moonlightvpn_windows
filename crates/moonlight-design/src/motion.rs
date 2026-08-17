//! Moonlight motion: short, eased, and mostly about *position*. Two curves do
//! almost all the work — a calm ease for colour/opacity, and an overshoot curve
//! for anything that slides into place (tab pill, segmented pill, toggle knob).
//! Presses shrink; hovers change colour or border, never scale up.
//!
//! SwiftUI takes a curve and drives the interpolation itself. iced has no
//! implicit animation, so the same tokens are expressed as cubic-bézier
//! easing functions the screens sample by hand against an elapsed fraction —
//! see [`Curve::at`]. The control points are the ones from the source tokens,
//! unchanged, which is what keeps the two clients feeling the same.

use std::time::Duration;

/// A CSS-style `cubic-bezier(x1, y1, x2, y2)` easing curve.
///
/// The first and last control points are fixed at (0,0) and (1,1), so only the
/// two middle ones are carried. `y` may leave [0,1] — that is what produces the
/// overshoot in [`Curve::SLIDE`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Curve {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

impl Curve {
    /// Colour, opacity and border changes.
    pub const EASE: Curve = Curve::new(0.2, 0.7, 0.3, 1.0);
    /// Anything that lands with a little overshoot.
    pub const BOUNCE: Curve = Curve::new(0.5, 1.4, 0.4, 1.0);
    /// Sliding selection pills.
    pub const SLIDE: Curve = Curve::new(0.5, 1.28, 0.32, 1.0);
    /// Page-open stagger.
    pub const RISE: Curve = Curve::new(0.22, 0.85, 0.3, 1.0);

    pub const fn new(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Curve { x1, y1, x2, y2 }
    }

    /// Eased progress for a linear fraction `t` in [0,1].
    ///
    /// The curve is parametric in an unknown `s`, so `x(s) = t` is solved first
    /// — Newton's method from a linear seed, falling back to bisection when the
    /// derivative goes flat. Ten iterations is comfortably inside a pixel for
    /// every curve in this file.
    pub fn at(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        if t == 0.0 || t == 1.0 {
            return t;
        }

        let bezier = |a: f32, b: f32, s: f32| {
            let u = 1.0 - s;
            3.0 * u * u * s * a + 3.0 * u * s * s * b + s * s * s
        };
        let slope = |a: f32, b: f32, s: f32| {
            let u = 1.0 - s;
            3.0 * u * u * (a) + 6.0 * u * s * (b - a) + 3.0 * s * s * (1.0 - b)
        };

        let mut s = t;
        for _ in 0..10 {
            let x = bezier(self.x1, self.x2, s) - t;
            if x.abs() < 1e-5 {
                break;
            }
            let d = slope(self.x1, self.x2, s);
            if d.abs() < 1e-6 {
                // Flat derivative: bisect the remaining interval instead of
                // dividing by ~zero and flinging `s` out of range.
                let (mut lo, mut hi) = (0.0_f32, 1.0_f32);
                for _ in 0..20 {
                    s = (lo + hi) / 2.0;
                    if bezier(self.x1, self.x2, s) < t {
                        lo = s;
                    } else {
                        hi = s;
                    }
                }
                break;
            }
            s -= x / d;
        }

        bezier(self.y1, self.y2, s)
    }
}

/// Durations, named as the source names them.
pub mod dur {
    use std::time::Duration;

    pub const PRESS: Duration = Duration::from_millis(180);
    /// Background / colour / border changes.
    pub const PAINT: Duration = Duration::from_millis(200);
    /// Pill glide.
    pub const SLIDE: Duration = Duration::from_millis(420);
    /// Screen change.
    pub const ENTER: Duration = Duration::from_millis(350);
    /// Staggered content entrance.
    pub const RISE: Duration = Duration::from_millis(520);
}

/// Linear progress through `duration`, clamped to [0,1]. The screens pair this
/// with a [`Curve`] rather than easing time directly, so a curve can be swapped
/// without touching the clock.
pub fn progress(elapsed: Duration, duration: Duration) -> f32 {
    if duration.is_zero() {
        return 1.0;
    }
    (elapsed.as_secs_f32() / duration.as_secs_f32()).clamp(0.0, 1.0)
}

// Press scales — the whole system uses exactly these three.
pub const PRESS_CARD: f32 = 0.985;
pub const PRESS_BUTTON: f32 = 0.97;
pub const PRESS_ICON: f32 = 0.92;

/// Corner radii, from `tokens/radii.css`.
///
/// Role-based, not a t-shirt scale: a radius is chosen by what the element *is*,
/// and it grows with the element's size. Nothing in the system is
/// square-cornered, and every button is a full pill.
pub mod radii {
    /// The keyboard-shortcut chip under the dial. The one 7px corner in the
    /// system, and the composition sets it literally.
    pub const CHIP: f32 = 7.0;
    /// 38px icon button.
    pub const ICON_SM: f32 = 11.0;
    /// 36–42px icon chip, header tile, avatar.
    pub const ICON: f32 = 12.0;
    /// 46px icon chip.
    pub const ICON_LG: f32 = 14.0;
    /// Input, link box, tab slot.
    pub const FIELD: f32 = 16.0;
    /// List row.
    pub const ROW: f32 = 18.0;
    /// The stats strip and the sidebar quota card, which the composition sets
    /// at 20 rather than at either neighbouring step.
    pub const CARD_SM: f32 = 20.0;
    /// The default card.
    pub const CARD: f32 = 22.0;
    /// Subscription card.
    pub const CARD_LG: f32 = 24.0;
    /// The screen panels. `--ml-r-hero` is 28, but the desktop composition sets
    /// its two big panels at 26, and the composition is what ships.
    pub const PANEL: f32 = 26.0;
    /// Accent hero slab.
    pub const HERO: f32 = 28.0;
    /// The window itself. macOS rounds at 12; Windows at 8.
    pub const WINDOW: f32 = 8.0;
    pub const PILL: f32 = 999.0;
}

/// Border weights. A 1px hairline is the default; 1.5px marks selection or a
/// focused input; 2px is only a tile.
pub mod border {
    pub const HAIRLINE: f32 = 1.0;
    pub const SELECTED: f32 = 1.5;
    pub const TILE: f32 = 2.0;
}

/// Shell metrics, from `tokens/spacing.css` and the desktop composition.
pub mod metrics {
    /// The desktop sidebar.
    pub const RAIL: f32 = 236.0;
    /// Collapsed to an icon rail.
    pub const RAIL_COLLAPSED: f32 = 76.0;
    /// The page header. The token says 56; the desktop composition sets 64.
    pub const HEADER: f32 = 64.0;
    /// The custom title bar.
    pub const TITLE_BAR: f32 = 40.0;
    /// A nav row.
    pub const NAV_ROW: f32 = 44.0;
    /// An icon button, and the sidebar logo tile.
    pub const CONTROL_SM: f32 = 38.0;
    /// A secondary control.
    pub const CONTROL_MD: f32 = 46.0;
    /// A primary button.
    pub const CONTROL: f32 = 54.0;
    pub const CONTROL_INPUT: f32 = 48.0;
    /// The connect dial.
    pub const DIAL: f32 = 238.0;
    /// The stats strip under it.
    pub const STATS_MAX: f32 = 420.0;

    /// Between cards in a page column.
    pub const GAP_STACK: f32 = 14.0;
    /// Between the screen's own columns.
    pub const GAP_COLUMNS: f32 = 16.0;
    /// Between tiles in a grid.
    pub const GAP_GRID: f32 = 12.0;
    pub const PAD_CARD: f32 = 18.0;
    pub const PAD_CARD_LG: f32 = 20.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curves_are_pinned_at_both_ends() {
        for curve in [Curve::EASE, Curve::BOUNCE, Curve::SLIDE, Curve::RISE] {
            assert_eq!(curve.at(0.0), 0.0);
            assert_eq!(curve.at(1.0), 1.0);
        }
    }

    #[test]
    fn the_ease_curve_front_loads() {
        // 0.2,0.7 pulls most of the movement into the first half — that is what
        // makes a colour change read as immediate rather than as a fade.
        assert!(Curve::EASE.at(0.5) > 0.5);
    }

    #[test]
    fn the_slide_curve_overshoots() {
        // y1 = 1.28 must actually produce a value above 1 somewhere, or the
        // pill lands flat and the overshoot token is decorative.
        let peak = (1..100)
            .map(|i| Curve::SLIDE.at(i as f32 / 100.0))
            .fold(f32::MIN, f32::max);
        assert!(peak > 1.0, "slide peaked at {peak}, never overshooting");
    }

    #[test]
    fn curves_are_monotonic_in_x() {
        // Not in y — SLIDE and BOUNCE deliberately come back down. But sampling
        // must never jump backwards in time.
        for curve in [Curve::EASE, Curve::RISE] {
            let mut previous = f32::MIN;
            for i in 0..=100 {
                let v = curve.at(i as f32 / 100.0);
                assert!(v >= previous - 1e-4, "{curve:?} went backwards at {i}");
                previous = v;
            }
        }
    }

    #[test]
    fn progress_clamps_past_the_end() {
        assert_eq!(progress(Duration::from_secs(10), dur::PAINT), 1.0);
        assert_eq!(progress(Duration::ZERO, dur::PAINT), 0.0);
        assert_eq!(progress(Duration::from_secs(1), Duration::ZERO), 1.0);
    }

    #[test]
    fn press_scales_shrink() {
        for scale in [PRESS_CARD, PRESS_BUTTON, PRESS_ICON] {
            assert!(scale < 1.0, "a press must never scale up");
        }
    }
}
