//! Renders a lucide glyph at its drawn geometry.
//!
//! Stroke, cap and join match lucide's own SVG attributes (`round`/`round`),
//! and the path is never filled — that is what keeps these identical to the
//! design rather than merely similar.

use std::cell::Cell;

use iced::widget::canvas::{self, Cache, Geometry, LineCap, LineJoin, Stroke};
use iced::{mouse, Color, Element, Length, Rectangle, Renderer, Size, Theme};

use crate::icons::Icon;
use crate::svg_path::SvgPath;

/// lucide's viewBox. Both the geometry and the stroke width are expressed in
/// it, so both scale together.
const VIEW_BOX: f32 = 24.0;

pub struct IconView {
    icon: Icon,
    stroke_width: f32,
    color: Color,
}

/// A glyph's drawing, and what it was drawn as.
///
/// Kept in the widget's state, which outlives a redraw — the `IconView` does
/// not: the view is rebuilt for every frame, so a cache held in it was a new,
/// empty one each time, and every glyph on screen was parsed and tessellated
/// again sixty times a second.
#[derive(Default)]
pub struct Drawn {
    cache: Cache,
    of: Cell<Option<(Icon, u32, [u32; 4])>>,
}

impl IconView {
    /// The glyph is drawn to fill whatever bounds the canvas is given, so the
    /// size lives on the element rather than in here.
    pub fn new(icon: Icon, color: Color) -> Self {
        IconView {
            icon,
            stroke_width: 2.0,
            color,
        }
    }

    /// lucide draws at 2px in a 24px box. The few places the design thins a
    /// glyph down go through here.
    pub fn stroke_width(mut self, width: f32) -> Self {
        self.stroke_width = width;
        self
    }
}

impl<Message> canvas::Program<Message> for IconView {
    type State = Drawn;

    fn draw(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        // Drawn again only when it is a different glyph, weight or colour; a
        // different size the cache notices itself.
        let Color { r, g, b, a } = self.color;
        let of = (
            self.icon,
            self.stroke_width.to_bits(),
            [r, g, b, a].map(f32::to_bits),
        );
        if state.of.replace(Some(of)) != Some(of) {
            state.cache.clear();
        }
        let geometry = state.cache.draw(renderer, bounds.size(), |frame| {
            let rect = Rectangle::new(iced::Point::ORIGIN, frame.size());
            // lucide's stroke-width is expressed in the 24×24 viewBox, so it
            // scales with the glyph rather than staying a fixed device width.
            let scaled = self.stroke_width * rect.width.min(rect.height) / VIEW_BOX;
            let stroke = Stroke::default()
                .with_width(scaled)
                .with_color(self.color)
                .with_line_cap(LineCap::Round)
                .with_line_join(LineJoin::Round);

            for d in self.icon.paths() {
                frame.stroke(&SvgPath::parse(d).to_canvas_path(rect, VIEW_BOX), stroke);
            }
        });

        vec![geometry]
    }
}

/// The glyph as an element sized to its own box.
pub fn icon<'a, Message: 'a>(icon: Icon, size: f32, color: Color) -> Element<'a, Message> {
    canvas::Canvas::new(IconView::new(icon, color))
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .into()
}

/// The same at a non-default stroke width.
pub fn icon_thin<'a, Message: 'a>(
    glyph: Icon,
    size: f32,
    color: Color,
    stroke_width: f32,
) -> Element<'a, Message> {
    canvas::Canvas::new(IconView::new(glyph, color).stroke_width(stroke_width))
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .into()
}

/// Whether a glyph would draw anything at this size — used by the tests to keep
/// a silently-blank icon from shipping.
pub fn is_drawable(glyph: Icon, size: f32) -> bool {
    let rect = Rectangle::new(iced::Point::ORIGIN, Size::new(size, size));
    glyph.paths().iter().any(|d| {
        let path = SvgPath::parse(d);
        !path.commands.is_empty() && rect.width > 0.0
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_glyph_draws_at_the_sizes_the_design_uses() {
        for glyph in Icon::ALL {
            for size in [14.0, 16.0, 18.0, 20.0, 24.0] {
                assert!(
                    is_drawable(*glyph, size),
                    "{} draws nothing at {size}",
                    glyph.name()
                );
            }
        }
    }
}
