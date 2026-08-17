//! The connect dial.
//!
//! The ring is **full when connected** and sweeps closed as it connects. It used
//! to show remaining quota, which meant a perfectly healthy tunnel drew a ring
//! with a gap in it — and a gap in a status ring reads as a fault, not as "you
//! have used some traffic". The quota has a bar of its own in the sidebar, where
//! a partial fill is the point.

use std::f32::consts::TAU;

use iced::widget::canvas::{self, Cache, Geometry, LineCap, Path, Stroke};
use iced::{mouse, Color, Point, Rectangle, Renderer, Theme};

use moonlight_core::ConnectionState;
use moonlight_design::Palette;

/// The sweep's thickness. The composition masks a conic gradient to a 6px ring.
const SWEEP_WIDTH: f32 = 6.0;

/// The track under it, which is thinner — 2px of hairline. Drawing both at one
/// weight loses the distinction between "the dial's extent" and "how far round
/// it has gone".
const TRACK_WIDTH: f32 = 2.0;

/// The halo sits 10px outside the dial and breathes while connected.
const HALO_INSET: f32 = 10.0;

/// The ring starts at twelve o'clock rather than at three, which is where a
/// canvas's zero angle sits.
const TWELVE_OCLOCK: f32 = -TAU / 4.0;

pub struct Dial {
    pub state: ConnectionState,
    pub palette: Palette,
    /// 0…1 through the connecting animation; ignored in every other state.
    pub progress: f32,
    /// 0…1 through the halo's 4.2s breath. Only read while connected.
    pub breath: f32,
    pub cache: Cache,
}

impl Dial {
    pub fn new(state: ConnectionState, palette: Palette, progress: f32, breath: f32) -> Self {
        Dial {
            state,
            palette,
            progress,
            breath,
            cache: Cache::new(),
        }
    }

    /// The halo's opacity. Zero unless connected, and it breathes between a half
    /// and a quarter — the composition runs it at .5 with a 4.2s cycle.
    pub fn halo_opacity(&self) -> f32 {
        if !self.state.is_connected() {
            return 0.0;
        }
        let phase = (self.breath.clamp(0.0, 1.0) * std::f32::consts::TAU).cos();
        // cos runs [-1,1]; map it onto [0.25, 0.5].
        0.375 + phase * 0.125
    }

    /// The fraction of the ring that is drawn, and in what colour.
    ///
    /// Split out from the drawing so the rule — full when connected — is
    /// testable without a renderer.
    pub fn sweep(&self) -> (f32, Color) {
        match &self.state {
            ConnectionState::Connected => (1.0, self.palette.accent_line),
            ConnectionState::Connecting => {
                (self.progress.clamp(0.0, 1.0), self.palette.accent_line)
            }
            ConnectionState::Disconnecting => {
                (1.0 - self.progress.clamp(0.0, 1.0), self.palette.text_muted)
            }
            ConnectionState::Failed(_) => (1.0, self.palette.danger),
            ConnectionState::Disconnected => (0.0, self.palette.text_muted),
        }
    }
}

impl<Message> canvas::Program<Message> for Dial {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let geometry = self.cache.draw(renderer, bounds.size(), |frame| {
            let centre = Point::new(frame.width() / 2.0, frame.height() / 2.0);
            // The halo lives outside the dial, so the dial's own radius is inset
            // by it as well as by half the sweep.
            let outer = frame.width().min(frame.height()) / 2.0;
            let radius = outer - HALO_INSET - SWEEP_WIDTH / 2.0;
            if radius <= 0.0 {
                return;
            }

            // The halo: a single hairline ring, 10px out, breathing.
            let halo = self.halo_opacity();
            if halo > 0.0 {
                frame.stroke(
                    &Path::circle(centre, radius + HALO_INSET),
                    Stroke::default().with_width(1.0).with_color(Color {
                        a: halo,
                        ..self.palette.accent
                    }),
                );
            }

            // The track is always drawn, so the ring reads as a dial with a
            // value rather than as an arc floating in space.
            frame.stroke(
                &Path::circle(centre, radius),
                Stroke::default()
                    .with_width(TRACK_WIDTH)
                    .with_color(self.palette.hairline),
            );

            let (fraction, color) = self.sweep();
            if fraction <= 0.0 {
                return;
            }

            let arc = Path::new(|builder| {
                builder.arc(canvas::path::Arc {
                    center: centre,
                    radius,
                    start_angle: iced::Radians(TWELVE_OCLOCK),
                    end_angle: iced::Radians(TWELVE_OCLOCK + TAU * fraction),
                });
            });
            frame.stroke(
                &arc,
                Stroke::default()
                    .with_width(SWEEP_WIDTH)
                    .with_color(color)
                    // Round caps, so a partial sweep does not end in a hard
                    // chisel that reads as a broken ring.
                    .with_line_cap(LineCap::Round),
            );
        });

        vec![geometry]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dial(state: ConnectionState, progress: f32) -> Dial {
        Dial::new(state, Palette::DARK, progress, 0.0)
    }

    #[test]
    fn a_connected_tunnel_draws_a_full_ring() {
        // A gap in a status ring reads as a fault. Quota lives in the sidebar.
        let (fraction, color) = dial(ConnectionState::Connected, 0.0).sweep();
        assert_eq!(fraction, 1.0);
        assert_eq!(color, Palette::DARK.accent_line);
    }

    #[test]
    fn a_disconnected_tunnel_draws_no_sweep_at_all() {
        let (fraction, _) = dial(ConnectionState::Disconnected, 0.5).sweep();
        assert_eq!(fraction, 0.0);
    }

    #[test]
    fn connecting_sweeps_closed_as_it_goes() {
        assert_eq!(dial(ConnectionState::Connecting, 0.0).sweep().0, 0.0);
        assert_eq!(dial(ConnectionState::Connecting, 0.5).sweep().0, 0.5);
        assert_eq!(dial(ConnectionState::Connecting, 1.0).sweep().0, 1.0);
    }

    #[test]
    fn disconnecting_runs_the_other_way() {
        assert_eq!(dial(ConnectionState::Disconnecting, 0.0).sweep().0, 1.0);
        assert_eq!(dial(ConnectionState::Disconnecting, 1.0).sweep().0, 0.0);
    }

    #[test]
    fn progress_outside_the_unit_range_cannot_overdraw_the_ring() {
        // A late frame can hand in a fraction past 1; the arc must not wrap
        // round and draw a second lap.
        assert_eq!(dial(ConnectionState::Connecting, 1.7).sweep().0, 1.0);
        assert_eq!(dial(ConnectionState::Connecting, -0.4).sweep().0, 0.0);
    }

    #[test]
    fn a_failure_is_a_full_ring_in_the_danger_colour() {
        // Full, not empty: the shape says "there is a state here", the colour
        // says which one.
        let (fraction, color) = dial(ConnectionState::Failed("x".into()), 0.0).sweep();
        assert_eq!(fraction, 1.0);
        assert_eq!(color, Palette::DARK.danger);
    }

    #[test]
    fn the_sweep_and_the_track_are_the_weights_the_composition_sets() {
        // One weight for both loses the distinction between the dial's extent
        // and how far round it has gone. The composition masks its conic
        // gradient to 6px and draws the track under it at 2.
        assert_eq!(SWEEP_WIDTH, 6.0);
        assert_eq!(TRACK_WIDTH, 2.0);
    }

    #[test]
    fn the_halo_is_dark_unless_connected() {
        for state in [
            ConnectionState::Disconnected,
            ConnectionState::Connecting,
            ConnectionState::Failed("x".into()),
        ] {
            assert_eq!(
                Dial::new(state, Palette::DARK, 0.5, 0.5).halo_opacity(),
                0.0
            );
        }
    }

    #[test]
    fn the_halo_breathes_without_ever_going_out_or_solid() {
        // A halo that reaches zero reads as a flicker, and one that reaches full
        // competes with the sweep.
        let mut seen_low = f32::MAX;
        let mut seen_high = f32::MIN;
        for i in 0..=100 {
            let o = Dial::new(
                ConnectionState::Connected,
                Palette::DARK,
                1.0,
                i as f32 / 100.0,
            )
            .halo_opacity();
            seen_low = seen_low.min(o);
            seen_high = seen_high.max(o);
        }
        assert!(seen_low > 0.2, "dipped to {seen_low}");
        assert!(seen_high < 0.55, "peaked at {seen_high}");
    }

    #[test]
    fn the_ring_starts_at_twelve_oclock() {
        // A canvas's zero angle is at three; a sweep that started there would
        // fill from the right-hand side and read as a progress bar bent round.
        assert!((TWELVE_OCLOCK - (-std::f32::consts::FRAC_PI_2)).abs() < 1e-6);
    }
}
