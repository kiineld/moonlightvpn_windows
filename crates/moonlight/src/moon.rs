//! The connect control: the moon from the logo.
//!
//! Disconnected it is the logo's crescent, dim, with its two stars; connected
//! the shadow slides off and it is a full moon, lit in the brand's own lime in
//! both themes, and the stars go as the sky brightens. Changing state is the
//! moon changing phase — the one moment in the interface allowed to be
//! expressive. While the tunnel is changing state a thin orbit turns round it.
//! The same drawing as the macOS client's `PowerButton`.

use std::f32::consts::{PI, TAU};

use iced::widget::canvas::{self, path, Geometry, LineCap, Path, Stroke};
use iced::{mouse, Color, Point, Radians, Rectangle, Renderer, Theme, Vector};

use moonlight_design::Palette;

use crate::theme::alpha;

pub struct Moon {
    pub palette: Palette,
    /// 0 a crescent, 1 a full moon, and in between while the phase changes.
    pub phase: f32,
    /// Where the orbit is, 0…1 round the circle; `None` while nothing is
    /// changing, which hides it.
    pub orbit: Option<f32>,
    /// Dimmed to half when there is nothing to connect to.
    pub enabled: bool,
}

impl<Message> canvas::Program<Message> for Moon {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let side = bounds.width.min(bounds.height);
        let centre = frame.center();
        let dim = |color: Color| {
            if self.enabled {
                color
            } else {
                alpha(color, color.a * 0.5)
            }
        };
        let palette = self.palette;
        let phase = self.phase.clamp(0.0, 1.0);

        // The disc the moon sits on, and its rim: the moon's colour while it
        // is lit, a quiet line under the pointer.
        let disc = Path::circle(centre, side / 2.0);
        frame.fill(&disc, dim(palette.surface2));
        let hovered = self.enabled
            && cursor
                .position_in(bounds)
                .is_some_and(|at| at.distance(Point::new(side / 2.0, side / 2.0)) <= side / 2.0);
        let rim = if phase > 0.0 {
            alpha(palette.brand, 0.75 * phase)
        } else if hovered {
            alpha(palette.text, 0.45)
        } else {
            palette.hairline
        };
        frame.stroke(
            &Path::circle(centre, side / 2.0 - 0.5),
            Stroke::default().with_width(1.0).with_color(dim(rim)),
        );

        if let Some(turn) = self.orbit {
            let start = TAU * turn - PI / 2.0;
            let orbit = Path::new(|b| {
                b.arc(path::Arc {
                    center: centre,
                    radius: side / 2.0 - side * 0.06,
                    start_angle: Radians(start),
                    end_angle: Radians(start + TAU * 0.22),
                })
            });
            frame.stroke(
                &orbit,
                Stroke::default()
                    .with_width(1.5)
                    .with_color(palette.text)
                    .with_line_cap(LineCap::Round),
            );
        }

        let moon = side * 0.5;
        let lit = mix(palette.text2, palette.brand, phase);
        frame.fill(&phase_path(centre, moon, phase), dim(lit));

        // The logo's two stars, up and to the right. They belong to the night,
        // so they go as the moon fills.
        let stars = alpha(palette.text2, 1.0 - phase);
        for (x, y, d) in [(0.5, -0.5, 0.1), (0.24, -0.72, 0.065)] {
            frame.fill(
                &Path::circle(centre + Vector::new(moon * x, moon * y), moon * d / 2.0),
                dim(stars),
            );
        }

        vec![frame.into_geometry()]
    }
}

/// The lit part of the moon: a disc with a second disc of the same size cut out
/// of it. The cut's offset is the phase — close in, a crescent lit to the lower
/// left as the logo draws it; slid off to the upper right, a full moon.
///
/// Drawn as the outline of what is left rather than by painting the cut over
/// it, so whatever is behind shows through the dark part.
fn phase_path(centre: Point, diameter: f32, phase: f32) -> Path {
    let r = diameter / 2.0;
    let offset = cut_offset(diameter, phase);
    let apart = (offset.x * offset.x + offset.y * offset.y).sqrt();
    if apart >= diameter {
        return Path::circle(centre, r);
    }
    // Where the two circles cross, as angles either side of the line between
    // their centres: ±θ from the moon's centre, π∓θ from the cut's.
    let towards = offset.y.atan2(offset.x);
    let theta = (apart / diameter).acos();
    let cut = centre + offset;
    Path::new(|b| {
        b.arc(path::Arc {
            center: centre,
            radius: r,
            start_angle: Radians(towards + theta),
            end_angle: Radians(towards + TAU - theta),
        });
        b.arc(path::Arc {
            center: cut,
            radius: r,
            start_angle: Radians(towards + PI + theta),
            end_angle: Radians(towards + PI - theta),
        });
        b.close();
    })
}

/// Where the cut sits from the moon's centre: close in to the upper right for
/// the crescent, slid well off for the full moon.
fn cut_offset(diameter: f32, phase: f32) -> Vector {
    Vector::new(
        diameter * (0.3 + (1.15 - 0.3) * phase),
        diameter * (-0.24 + (-0.9 + 0.24) * phase),
    )
}

fn mix(from: Color, to: Color, t: f32) -> Color {
    Color {
        r: from.r + (to.r - from.r) * t,
        g: from.g + (to.g - from.g) * t,
        b: from.b + (to.b - from.b) * t,
        a: from.a + (to.a - from.a) * t,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cut_leaves_the_moon_before_the_phase_is_full() {
        // Past the point where the circles stop crossing the moon must be a
        // whole disc, not an arc pair with nothing between them.
        let d = 100.0_f32;
        let apart = |phase: f32| {
            let v = cut_offset(d, phase);
            (v.x * v.x + v.y * v.y).sqrt()
        };
        assert!(apart(0.0) < d, "the crescent must have a bite in it");
        assert!(apart(1.0) >= d, "the full moon must have none");
    }
}
