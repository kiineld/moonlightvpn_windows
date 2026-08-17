//! The app mark: a crescent moon with two stars on an accent rounded square.
//!
//! Drawn rather than embedded as an image, for the same reason the icons are
//! path data — it has to take the accent in both themes (lime on dark, yellow on
//! light) and stay sharp on a 200% display without shipping four raster sizes.
//!
//! The geometry is `assets/logo-tile.svg` verbatim, in its own 44×44 view box,
//! and is the same path the macOS client draws. It is not redrawn by eye: an
//! earlier pass here approximated the crescent as two overlapping circles with a
//! four-pointed sparkle beside it, which is a different mark — the products
//! stopped looking like the same product.

use iced::widget::canvas::{self, Cache, Geometry, Path};
use iced::{mouse, Point, Rectangle, Renderer, Size, Theme};

use moonlight_design::{Palette, SvgPath};

/// The view box the geometry below is expressed in.
const VIEW_BOX: f32 = 44.0;

/// The crescent. Its bite is part of the path rather than a knocked-out disc, so
/// it composites correctly on any slab colour.
const CRESCENT: &str = "M30 22a8.4 8.4 0 1 1-9.4-8.34A10 10 0 0 0 30 22Z";

/// The two stars, as centre/radius in the same 44×44 space.
const STARS: [(f32, f32, f32); 2] = [(30.5, 12.5, 1.7), (25.0, 8.0, 1.1)];

pub struct Logo {
    palette: Palette,
    /// The slab's corner radius, in the *rendered* size — 10 at 32pt in the
    /// sidebar, 5 at 18pt in the title bar.
    radius: f32,
    cache: Cache,
}

impl Logo {
    pub fn new(palette: Palette) -> Self {
        Logo::with_radius(palette, 10.0)
    }

    pub fn with_radius(palette: Palette, radius: f32) -> Self {
        Logo {
            palette,
            radius,
            cache: Cache::new(),
        }
    }
}

impl<Message> canvas::Program<Message> for Logo {
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
            let size = frame.width().min(frame.height());
            if size <= 0.0 {
                return;
            }
            let scale = size / VIEW_BOX;

            let slab = Path::rounded_rectangle(
                Point::ORIGIN,
                Size::new(size, size),
                self.radius.min(size / 2.0).into(),
            );
            frame.fill(&slab, self.palette.accent);

            let ink = self.palette.text_on_accent;
            let box_rect = Rectangle::new(Point::ORIGIN, Size::new(size, size));
            frame.fill(
                &SvgPath::parse(CRESCENT).to_canvas_path(box_rect, VIEW_BOX),
                ink,
            );

            for (cx, cy, r) in STARS {
                let star = Path::new(|b| b.circle(Point::new(cx * scale, cy * scale), r * scale));
                frame.fill(&star, ink);
            }
        });

        vec![geometry]
    }
}
