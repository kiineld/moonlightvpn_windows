//! The two controls that move: the segmented track, whose capsule glides from
//! one option to the next, and the switch, whose knob slides.
//!
//! As the macOS client's `SegmentedPill` and `MLToggle` do, on the one spring.
//! Each keeps its own motion in its widget state and asks for frames only while
//! it is moving, so no screen has to remember what it last showed.

use std::time::Instant;

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer::{self, Quad};
use iced::advanced::widget::{tree, Operation, Tree, Widget};
use iced::advanced::{mouse, overlay, Clipboard, Shell};
use iced::{window, Border, Color, Element, Event, Length, Rectangle, Renderer, Size, Theme};

use moonlight_design::motion::spring;

/// How far along the spring a motion started at `started` is, at `now`.
fn eased(started: Option<Instant>, now: Instant) -> f32 {
    started.map_or(1.0, |at| {
        spring::at(
            now.saturating_duration_since(at).as_secs_f32(),
            spring::STANDARD,
        )
    })
}

fn moving(started: Option<Instant>, now: Instant) -> bool {
    started.is_some_and(|at| now.saturating_duration_since(at) < spring::SETTLE)
}

fn mix(from: Color, to: Color, t: f32) -> Color {
    Color {
        r: from.r + (to.r - from.r) * t,
        g: from.g + (to.g - from.g) * t,
        b: from.b + (to.b - from.b) * t,
        a: from.a + (to.a - from.a) * t,
    }
}

fn lerp(from: Rectangle, to: Rectangle, t: f32) -> Rectangle {
    Rectangle {
        x: from.x + (to.x - from.x) * t,
        y: from.y + (to.y - from.y) * t,
        width: from.width + (to.width - from.width) * t,
        height: from.height + (to.height - from.height) * t,
    }
}

/// A row of options with one capsule under the chosen one, gliding when the
/// choice changes.
///
/// The labels are given twice, laid out the same: `options` in their resting
/// colour, which is what is clicked, and `lit` in the colour they take on the
/// capsule, drawn clipped to it. A label is therefore exactly as inverted as the
/// capsule is under it — switching its colour outright showed a dark label on
/// the dark track for the moment before the capsule arrived.
pub struct Sliding<'a, Message> {
    options: Element<'a, Message>,
    lit: Element<'a, Message>,
    selected: usize,
    fill: Color,
}

impl<'a, Message> Sliding<'a, Message> {
    pub fn new(
        options: impl Into<Element<'a, Message>>,
        lit: impl Into<Element<'a, Message>>,
        selected: usize,
        fill: Color,
    ) -> Self {
        Sliding {
            options: options.into(),
            lit: lit.into(),
            selected,
            fill,
        }
    }
}

#[derive(Default)]
struct SlidingState {
    /// The option the capsule is heading for; `None` until first drawn, so
    /// it starts where it belongs instead of gliding in from nowhere.
    target: Option<usize>,
    /// Where it was when it set off.
    from: Rectangle,
    started: Option<Instant>,
}

impl SlidingState {
    fn capsule(&self, options: Layout<'_>, target: usize, now: Instant) -> Rectangle {
        let to = options.children().nth(target).map_or(
            Rectangle::new(options.bounds().position(), Size::ZERO),
            |option| option.bounds(),
        );
        match self.started {
            Some(_) => lerp(self.from, to, eased(self.started, now)),
            None => to,
        }
    }
}

impl<Message> Widget<Message, Theme, Renderer> for Sliding<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<SlidingState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(SlidingState::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.options), Tree::new(&self.lit)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&[self.options.as_widget(), self.lit.as_widget()]);
    }

    fn size(&self) -> Size<Length> {
        self.options.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let options = self
            .options
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits);
        let lit = self
            .lit
            .as_widget_mut()
            .layout(&mut tree.children[1], renderer, limits);
        layout::Node::with_children(options.size(), vec![options, lit])
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let options = layout.children().next().expect("laid out with its options");
        self.options
            .as_widget_mut()
            .operate(&mut tree.children[0], options, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let options = layout.children().next().expect("laid out with its options");
        self.options.as_widget_mut().update(
            &mut tree.children[0],
            event,
            options,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );

        if let Event::Window(window::Event::RedrawRequested(now)) = event {
            let state = tree.state.downcast_mut::<SlidingState>();
            if state.target != Some(self.selected) {
                if let Some(previous) = state.target {
                    state.from = state.capsule(options, previous, *now);
                    state.started = Some(*now);
                }
                state.target = Some(self.selected);
            }
            if moving(state.started, *now) {
                shell.request_redraw();
            }
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let options = layout.children().next().expect("laid out with its options");
        self.options.as_widget().mouse_interaction(
            &tree.children[0],
            options,
            cursor,
            viewport,
            renderer,
        )
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        use iced::advanced::Renderer as _;

        let mut children = layout.children();
        let options = children.next().expect("laid out with its options");
        let lit = children.next().expect("laid out with its lit labels");
        let state = tree.state.downcast_ref::<SlidingState>();
        let capsule = state.capsule(
            options,
            state.target.unwrap_or(self.selected),
            Instant::now(),
        );

        renderer.fill_quad(
            Quad {
                bounds: capsule,
                border: Border {
                    radius: (capsule.height / 2.0).into(),
                    ..Border::default()
                },
                ..Quad::default()
            },
            self.fill,
        );
        self.options.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            options,
            cursor,
            viewport,
        );
        renderer.with_layer(capsule, |renderer| {
            self.lit.as_widget().draw(
                &tree.children[1],
                renderer,
                theme,
                style,
                lit,
                cursor,
                viewport,
            );
        });
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: iced::Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        let options = layout.children().next().expect("laid out with its options");
        self.options.as_widget_mut().overlay(
            &mut tree.children[0],
            options,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message: 'a> From<Sliding<'a, Message>> for Element<'a, Message> {
    fn from(sliding: Sliding<'a, Message>) -> Self {
        Element::new(sliding)
    }
}

/// The switch: a track that fills and a knob that slides across it, both on
/// the spring.
pub struct Switch {
    pub on: bool,
    pub size: Size,
    pub inset: f32,
    pub knob: f32,
    pub track: (Color, Color),
    pub ink: (Color, Color),
}

#[derive(Default)]
struct SwitchState {
    target: Option<bool>,
    from: f32,
    started: Option<Instant>,
}

impl SwitchState {
    /// 0 off … 1 on.
    fn position(&self, target: bool, now: Instant) -> f32 {
        let to = if target { 1.0 } else { 0.0 };
        match self.started {
            Some(_) => self.from + (to - self.from) * eased(self.started, now),
            None => to,
        }
    }
}

impl<Message> Widget<Message, Theme, Renderer> for Switch {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<SwitchState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(SwitchState::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(
            Length::Fixed(self.size.width),
            Length::Fixed(self.size.height),
        )
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        _limits: &layout::Limits,
    ) -> layout::Node {
        layout::Node::new(self.size)
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        if let Event::Window(window::Event::RedrawRequested(now)) = event {
            let state = tree.state.downcast_mut::<SwitchState>();
            if state.target != Some(self.on) {
                if let Some(previous) = state.target {
                    state.from = state.position(previous, *now);
                    state.started = Some(*now);
                }
                state.target = Some(self.on);
            }
            if moving(state.started, *now) {
                shell.request_redraw();
            }
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        use iced::advanced::Renderer as _;

        let state = tree.state.downcast_ref::<SwitchState>();
        let t = state.position(state.target.unwrap_or(self.on), Instant::now());
        let bounds = layout.bounds();
        renderer.fill_quad(
            Quad {
                bounds,
                border: Border {
                    radius: (bounds.height / 2.0).into(),
                    ..Border::default()
                },
                ..Quad::default()
            },
            mix(self.track.0, self.track.1, t),
        );
        let travel = bounds.width - 2.0 * self.inset - self.knob;
        let knob = Rectangle {
            x: bounds.x + self.inset + travel * t,
            y: bounds.y + (bounds.height - self.knob) / 2.0,
            width: self.knob,
            height: self.knob,
        };
        renderer.fill_quad(
            Quad {
                bounds: knob,
                border: Border {
                    radius: (self.knob / 2.0).into(),
                    ..Border::default()
                },
                ..Quad::default()
            },
            mix(self.ink.0, self.ink.1, t),
        );
    }
}

impl<'a, Message: 'a> From<Switch> for Element<'a, Message> {
    fn from(switch: Switch) -> Self {
        Element::new(switch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn a_capsule_sets_off_from_where_it_is_not_from_where_it_was_going() {
        let a = Rectangle::new(iced::Point::new(0.0, 0.0), Size::new(40.0, 20.0));
        let b = Rectangle::new(iced::Point::new(100.0, 0.0), Size::new(60.0, 20.0));
        assert_eq!(lerp(a, b, 0.0), a);
        assert_eq!(lerp(a, b, 1.0), b);
        let half = lerp(a, b, 0.5);
        assert_eq!((half.x, half.width), (50.0, 50.0));
    }

    #[test]
    fn a_switch_lands_where_it_was_sent_and_stops_asking_for_frames() {
        let now = Instant::now();
        let state = SwitchState {
            target: Some(true),
            from: 0.0,
            started: now.checked_sub(Duration::from_secs(2)),
        };
        assert_eq!(state.position(true, now), 1.0);
        assert!(!moving(state.started, now));
        let fresh = SwitchState {
            target: Some(true),
            from: 0.0,
            started: Some(now),
        };
        assert!(fresh.position(true, now) < 0.01);
        assert!(moving(fresh.started, now));
    }
}
