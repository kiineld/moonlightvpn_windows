//! A parser for the SVG path `d` grammar, sufficient for the whole lucide set.
//!
//! [`crate::icons`] carries lucide's geometry as `d` strings — including the
//! icons lucide draws with `<circle>`/`<rect>`/`<line>`, which the generator
//! rewrites into path commands. That leaves exactly one thing to parse here.
//!
//! Everything in the grammar is supported, absolute and relative: `M L H V C S
//! Q T A Z`. The smooth variants (`S`/`T`) need the previous control point, and
//! arcs (`A`) are converted to cubic segments, so this is not a subset parser —
//! getting either wrong shows up as a visibly wrong glyph rather than an error.

use iced::widget::canvas;
use iced::{Point, Rectangle};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Command {
    Move(Point),
    Line(Point),
    Curve {
        to: Point,
        control1: Point,
        control2: Point,
    },
    Close,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SvgPath {
    pub commands: Vec<Command>,
}

impl SvgPath {
    pub fn parse(d: &str) -> Self {
        SvgPath {
            commands: Scanner::new(d).parse(),
        }
    }

    /// The path in lucide's own 24×24 space, scaled into `rect` and stroked by
    /// the caller. Lucide icons are never filled.
    pub fn to_canvas_path(&self, rect: Rectangle, view_box: f32) -> canvas::Path {
        let scale = rect.width.min(rect.height) / view_box;
        let dx = rect.x + (rect.width - view_box * scale) / 2.0;
        let dy = rect.y + (rect.height - view_box * scale) / 2.0;
        let t = |p: Point| Point::new(p.x * scale + dx, p.y * scale + dy);

        canvas::Path::new(|b| {
            for command in &self.commands {
                match *command {
                    Command::Move(p) => b.move_to(t(p)),
                    Command::Line(p) => b.line_to(t(p)),
                    Command::Curve {
                        to,
                        control1,
                        control2,
                    } => b.bezier_curve_to(t(control1), t(control2), t(to)),
                    Command::Close => b.close(),
                }
            }
        })
    }
}

struct Scanner {
    chars: Vec<char>,
    i: usize,
    current: Point,
    start: Point,
    /// Last cubic control point, for `S`. `None` resets the reflection to
    /// `current`.
    last_cubic_control: Option<Point>,
    /// Last quadratic control point, for `T`.
    last_quad_control: Option<Point>,
    out: Vec<Command>,
}

impl Scanner {
    fn new(d: &str) -> Self {
        Scanner {
            chars: d.chars().collect(),
            i: 0,
            current: Point::ORIGIN,
            start: Point::ORIGIN,
            last_cubic_control: None,
            last_quad_control: None,
            out: Vec::new(),
        }
    }

    fn parse(mut self) -> Vec<Command> {
        let mut command = ' ';
        loop {
            self.skip_separators();
            if self.i >= self.chars.len() {
                break;
            }

            if self.chars[self.i].is_alphabetic() {
                command = self.chars[self.i];
                self.i += 1;
            } else if command == ' ' {
                // Numbers before any command letter: malformed, give up rather
                // than guess at an implied command.
                break;
            } else if command == 'M' {
                command = 'L'; // repeated moveto pairs are implicit linetos
            } else if command == 'm' {
                command = 'l';
            }

            if !self.step(command) {
                break;
            }
        }
        self.out
    }

    fn step(&mut self, command: char) -> bool {
        let relative = command.is_lowercase();
        let kind = command.to_ascii_lowercase();

        macro_rules! num {
            () => {
                match self.number() {
                    Some(v) => v,
                    None => return false,
                }
            };
        }

        match kind {
            'm' => {
                let (x, y) = (num!(), num!());
                self.current = self.point(relative, x, y);
                self.start = self.current;
                self.out.push(Command::Move(self.current));
                self.last_cubic_control = None;
                self.last_quad_control = None;
            }
            'l' => {
                let (x, y) = (num!(), num!());
                self.current = self.point(relative, x, y);
                self.out.push(Command::Line(self.current));
                self.last_cubic_control = None;
                self.last_quad_control = None;
            }
            'h' => {
                let x = num!();
                self.current = Point::new(
                    if relative { self.current.x + x } else { x },
                    self.current.y,
                );
                self.out.push(Command::Line(self.current));
                self.last_cubic_control = None;
                self.last_quad_control = None;
            }
            'v' => {
                let y = num!();
                self.current = Point::new(
                    self.current.x,
                    if relative { self.current.y + y } else { y },
                );
                self.out.push(Command::Line(self.current));
                self.last_cubic_control = None;
                self.last_quad_control = None;
            }
            'c' => {
                let (x1, y1) = (num!(), num!());
                let (x2, y2) = (num!(), num!());
                let (x, y) = (num!(), num!());
                let c1 = self.point(relative, x1, y1);
                let c2 = self.point(relative, x2, y2);
                let end = self.point(relative, x, y);
                self.out.push(Command::Curve {
                    to: end,
                    control1: c1,
                    control2: c2,
                });
                self.current = end;
                self.last_cubic_control = Some(c2);
                self.last_quad_control = None;
            }
            's' => {
                let (x2, y2) = (num!(), num!());
                let (x, y) = (num!(), num!());
                let c1 = self.reflect(self.last_cubic_control);
                let c2 = self.point(relative, x2, y2);
                let end = self.point(relative, x, y);
                self.out.push(Command::Curve {
                    to: end,
                    control1: c1,
                    control2: c2,
                });
                self.current = end;
                self.last_cubic_control = Some(c2);
                self.last_quad_control = None;
            }
            'q' => {
                let (x1, y1) = (num!(), num!());
                let (x, y) = (num!(), num!());
                let q = self.point(relative, x1, y1);
                let end = self.point(relative, x, y);
                self.append_quad(q, end);
            }
            't' => {
                let (x, y) = (num!(), num!());
                let q = self.reflect(self.last_quad_control);
                let end = self.point(relative, x, y);
                self.append_quad(q, end);
            }
            'a' => {
                let (rx, ry, rotation) = (num!(), num!(), num!());
                let large_arc = match self.flag() {
                    Some(f) => f,
                    None => return false,
                };
                let sweep = match self.flag() {
                    Some(f) => f,
                    None => return false,
                };
                let (x, y) = (num!(), num!());
                let end = self.point(relative, x, y);
                self.append_arc(rx, ry, rotation, large_arc, sweep, end);
                self.current = end;
                self.last_cubic_control = None;
                self.last_quad_control = None;
            }
            'z' => {
                self.out.push(Command::Close);
                self.current = self.start;
                self.last_cubic_control = None;
                self.last_quad_control = None;
            }
            _ => return false,
        }
        true
    }

    fn point(&self, relative: bool, x: f32, y: f32) -> Point {
        if relative {
            Point::new(self.current.x + x, self.current.y + y)
        } else {
            Point::new(x, y)
        }
    }

    /// A smooth curve's first control point is the previous one mirrored through
    /// the current point; with no previous curve it *is* the current point.
    fn reflect(&self, control: Option<Point>) -> Point {
        match control {
            None => self.current,
            Some(c) => Point::new(2.0 * self.current.x - c.x, 2.0 * self.current.y - c.y),
        }
    }

    fn append_quad(&mut self, control: Point, end: Point) {
        // Exact degree elevation — a quadratic is a cubic with these controls.
        let c1 = Point::new(
            self.current.x + 2.0 / 3.0 * (control.x - self.current.x),
            self.current.y + 2.0 / 3.0 * (control.y - self.current.y),
        );
        let c2 = Point::new(
            end.x + 2.0 / 3.0 * (control.x - end.x),
            end.y + 2.0 / 3.0 * (control.y - end.y),
        );
        self.out.push(Command::Curve {
            to: end,
            control1: c1,
            control2: c2,
        });
        self.current = end;
        self.last_quad_control = Some(control);
        self.last_cubic_control = None;
    }

    /// Endpoint-parameterised arc → centre parameterisation → cubic segments,
    /// following the SVG 1.1 implementation notes (F.6.5).
    fn append_arc(
        &mut self,
        rx: f32,
        ry: f32,
        rotation: f32,
        large_arc: bool,
        sweep: bool,
        end: Point,
    ) {
        let (mut rx, mut ry) = (rx.abs(), ry.abs());
        let p0 = self.current;
        if rx == 0.0 || ry == 0.0 || (p0.x == end.x && p0.y == end.y) {
            self.out.push(Command::Line(end));
            return;
        }

        let phi = rotation * std::f32::consts::PI / 180.0;
        let (cos_phi, sin_phi) = (phi.cos(), phi.sin());
        let dx2 = (p0.x - end.x) / 2.0;
        let dy2 = (p0.y - end.y) / 2.0;
        let x1 = cos_phi * dx2 + sin_phi * dy2;
        let y1 = -sin_phi * dx2 + cos_phi * dy2;

        // Scale up radii that are too small to span the endpoints (F.6.6).
        let lambda = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
        if lambda > 1.0 {
            let s = lambda.sqrt();
            rx *= s;
            ry *= s;
        }

        let sign = if large_arc == sweep { -1.0 } else { 1.0 };
        let numerator =
            (rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1).max(0.0);
        let denominator = rx * rx * y1 * y1 + ry * ry * x1 * x1;
        let coef = if denominator == 0.0 {
            0.0
        } else {
            sign * (numerator / denominator).sqrt()
        };
        let cx1 = coef * rx * y1 / ry;
        let cy1 = -coef * ry * x1 / rx;

        let cx = cos_phi * cx1 - sin_phi * cy1 + (p0.x + end.x) / 2.0;
        let cy = sin_phi * cx1 + cos_phi * cy1 + (p0.y + end.y) / 2.0;

        fn angle(ux: f32, uy: f32, vx: f32, vy: f32) -> f32 {
            let dot = ux * vx + uy * vy;
            let len = (ux * ux + uy * uy).sqrt() * (vx * vx + vy * vy).sqrt();
            if len == 0.0 {
                return 0.0;
            }
            let a = (dot / len).clamp(-1.0, 1.0).acos();
            if ux * vy - uy * vx < 0.0 {
                -a
            } else {
                a
            }
        }

        let theta = angle(1.0, 0.0, (x1 - cx1) / rx, (y1 - cy1) / ry);
        let mut delta = angle(
            (x1 - cx1) / rx,
            (y1 - cy1) / ry,
            (-x1 - cx1) / rx,
            (-y1 - cy1) / ry,
        );
        let tau = std::f32::consts::TAU;
        if !sweep && delta > 0.0 {
            delta -= tau;
        }
        if sweep && delta < 0.0 {
            delta += tau;
        }

        // A cubic approximates at most a quarter turn within tolerance.
        let segments = ((delta.abs() / std::f32::consts::FRAC_PI_2).ceil() as i32).max(1);
        let step = delta / segments as f32;
        let alpha = 4.0 / 3.0 * (step / 4.0).tan();

        let on_arc = |t: f32| {
            Point::new(
                cx + rx * t.cos() * cos_phi - ry * t.sin() * sin_phi,
                cy + rx * t.cos() * sin_phi + ry * t.sin() * cos_phi,
            )
        };
        let derivative = |t: f32| {
            Point::new(
                -rx * t.sin() * cos_phi - ry * t.cos() * sin_phi,
                -rx * t.sin() * sin_phi + ry * t.cos() * cos_phi,
            )
        };

        let mut angle_start = theta;
        let mut from = p0;
        for _ in 0..segments {
            let angle_end = angle_start + step;
            let to = on_arc(angle_end);
            let d0 = derivative(angle_start);
            let d1 = derivative(angle_end);
            self.out.push(Command::Curve {
                to,
                control1: Point::new(from.x + alpha * d0.x, from.y + alpha * d0.y),
                control2: Point::new(to.x - alpha * d1.x, to.y - alpha * d1.y),
            });
            from = to;
            angle_start = angle_end;
        }
    }

    // Lexing

    fn skip_separators(&mut self) {
        while self.i < self.chars.len()
            && matches!(self.chars[self.i], ' ' | ',' | '\n' | '\r' | '\t')
        {
            self.i += 1;
        }
    }

    fn number(&mut self) -> Option<f32> {
        self.skip_separators();
        let begin = self.i;
        if self.i < self.chars.len() && matches!(self.chars[self.i], '-' | '+') {
            self.i += 1;
        }
        while self.i < self.chars.len() && self.chars[self.i].is_ascii_digit() {
            self.i += 1;
        }
        if self.i < self.chars.len() && self.chars[self.i] == '.' {
            self.i += 1;
            while self.i < self.chars.len() && self.chars[self.i].is_ascii_digit() {
                self.i += 1;
            }
        }
        if self.i < self.chars.len() && matches!(self.chars[self.i], 'e' | 'E') {
            let mark = self.i;
            self.i += 1;
            if self.i < self.chars.len() && matches!(self.chars[self.i], '-' | '+') {
                self.i += 1;
            }
            if self.i < self.chars.len() && self.chars[self.i].is_ascii_digit() {
                while self.i < self.chars.len() && self.chars[self.i].is_ascii_digit() {
                    self.i += 1;
                }
            } else {
                self.i = mark;
            }
        }
        if self.i <= begin {
            return None;
        }
        let text: String = self.chars[begin..self.i].iter().collect();
        text.parse::<f32>().ok()
    }

    /// Arc flags are single characters and may run together with the numbers
    /// around them (`a5 5 0 1 0 10 0` is also legal as `a5 5 0 1010 0`), so they
    /// cannot go through [`Scanner::number`].
    fn flag(&mut self) -> Option<bool> {
        self.skip_separators();
        if self.i < self.chars.len() && matches!(self.chars[self.i], '0' | '1') {
            let value = self.chars[self.i] == '1';
            self.i += 1;
            Some(value)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f32, y: f32) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn absolute_move_and_line() {
        let path = SvgPath::parse("M1 2L3 4");
        assert_eq!(
            path.commands,
            vec![Command::Move(p(1.0, 2.0)), Command::Line(p(3.0, 4.0))]
        );
    }

    #[test]
    fn relative_commands_accumulate() {
        let path = SvgPath::parse("m1 1 l2 2");
        assert_eq!(
            path.commands,
            vec![Command::Move(p(1.0, 1.0)), Command::Line(p(3.0, 3.0))]
        );
    }

    #[test]
    fn repeated_moveto_pairs_become_linetos() {
        // "M1 1 2 2" is a moveto then an implicit lineto, not two movetos.
        let path = SvgPath::parse("M1 1 2 2");
        assert_eq!(
            path.commands,
            vec![Command::Move(p(1.0, 1.0)), Command::Line(p(2.0, 2.0))]
        );
    }

    #[test]
    fn horizontal_and_vertical_hold_the_other_axis() {
        let path = SvgPath::parse("M5 5H10V20");
        assert_eq!(
            path.commands,
            vec![
                Command::Move(p(5.0, 5.0)),
                Command::Line(p(10.0, 5.0)),
                Command::Line(p(10.0, 20.0)),
            ]
        );
    }

    #[test]
    fn smooth_cubic_reflects_the_previous_control() {
        // After C with c2 at (3,3) ending at (4,4), S's first control must be
        // (4,4) mirrored through c2 => (5,5).
        let path = SvgPath::parse("M0 0C1 1 3 3 4 4S7 7 8 8");
        match path.commands[2] {
            Command::Curve { control1, .. } => {
                assert!((control1.x - 5.0).abs() < 1e-4);
                assert!((control1.y - 5.0).abs() < 1e-4);
            }
            ref other => panic!("expected a curve, got {other:?}"),
        }
    }

    #[test]
    fn smooth_cubic_with_no_predecessor_uses_the_current_point() {
        let path = SvgPath::parse("M2 2S5 5 6 6");
        match path.commands[1] {
            Command::Curve { control1, .. } => {
                assert_eq!(control1, p(2.0, 2.0));
            }
            ref other => panic!("expected a curve, got {other:?}"),
        }
    }

    #[test]
    fn quadratic_is_elevated_exactly() {
        // Degree elevation of Q(0,0)-(3,3)-(6,0): controls at 2/3 of the way.
        let path = SvgPath::parse("M0 0Q3 3 6 0");
        match path.commands[1] {
            Command::Curve {
                control1, control2, ..
            } => {
                assert!((control1.x - 2.0).abs() < 1e-4);
                assert!((control1.y - 2.0).abs() < 1e-4);
                assert!((control2.x - 4.0).abs() < 1e-4);
                assert!((control2.y - 2.0).abs() < 1e-4);
            }
            ref other => panic!("expected a curve, got {other:?}"),
        }
    }

    #[test]
    fn arc_flags_may_run_together_with_their_numbers() {
        // "1010" is largeArc=1, sweep=0, then x=1, y=0 — the compact form real
        // lucide files use.
        let compact = SvgPath::parse("M0 0a5 5 0 1010 0");
        let spaced = SvgPath::parse("M0 0a5 5 0 1 0 10 0");
        assert_eq!(compact.commands.len(), spaced.commands.len());
        assert!(compact.commands.len() > 1);
    }

    #[test]
    fn a_full_circle_arc_pair_returns_to_its_start() {
        // The lucide circle idiom: two half-arcs back to the origin.
        let path = SvgPath::parse("M2 12a10 10 0 1 0 20 0a10 10 0 1 0 -20 0");
        let last = path.commands.last().unwrap();
        match last {
            Command::Curve { to, .. } => {
                assert!((to.x - 2.0).abs() < 1e-2, "x came back to {}", to.x);
                assert!((to.y - 12.0).abs() < 1e-2, "y came back to {}", to.y);
            }
            other => panic!("expected a curve, got {other:?}"),
        }
    }

    #[test]
    fn zero_radius_arc_degenerates_to_a_line() {
        let path = SvgPath::parse("M0 0A0 0 0 0 0 10 10");
        assert_eq!(
            path.commands,
            vec![Command::Move(p(0.0, 0.0)), Command::Line(p(10.0, 10.0))]
        );
    }

    #[test]
    fn close_returns_to_the_subpath_start() {
        let path = SvgPath::parse("M5 5L10 10ZL1 1");
        assert_eq!(path.commands[2], Command::Close);
        // After Z the current point is the subpath start, so the next absolute
        // lineto is unaffected but a relative one would key off (5,5).
        assert_eq!(path.commands[3], Command::Line(p(1.0, 1.0)));
    }

    #[test]
    fn numbers_before_a_command_are_refused_rather_than_guessed() {
        assert!(SvgPath::parse("5 5 L10 10").commands.is_empty());
    }

    #[test]
    fn exponent_notation_parses() {
        let path = SvgPath::parse("M1e1 2e-1");
        assert_eq!(path.commands, vec![Command::Move(p(10.0, 0.2))]);
    }

    #[test]
    fn a_truncated_command_stops_cleanly() {
        // Half a lineto: keep what parsed, drop the fragment.
        let path = SvgPath::parse("M1 1L5");
        assert_eq!(path.commands, vec![Command::Move(p(1.0, 1.0))]);
    }
}
