//! The app mark: a crescent and a star on a lime rounded square.
//!
//! Drawn rather than embedded as an image, for the same reason the icons are
//! path data — it has to be the right lime in both themes, and it has to stay
//! sharp on a 200% display without shipping four raster sizes. The geometry is
//! in a 40×40 box and scales to whatever the canvas is given.

use iced::widget::canvas::{self, Cache, Geometry, Path};
use iced::{mouse, Point, Rectangle, Renderer, Theme};

use moonlight_design::Palette;

/// The box the geometry below is expressed in.
const BOX: f32 = 40.0;

pub struct Logo {
    palette: Palette,
    cache: Cache,
}

impl Logo {
    pub fn new(palette: Palette) -> Self {
        Logo {
            palette,
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
            let scale = size / BOX;
            let at = |x: f32, y: f32| Point::new(x * scale, y * scale);

            // The lime slab. A generous radius, because the mark sits beside
            // 19px display type and a tight corner reads as a button.
            let slab = Path::rounded_rectangle(
                Point::ORIGIN,
                iced::Size::new(size, size),
                (11.0 * scale).into(),
            );
            frame.fill(&slab, self.palette.accent);

            // The crescent: a filled disc with a second disc knocked out of it.
            // Drawn as one path with two circles and the even-odd rule, so the
            // bite is transparent rather than painted in the slab's colour —
            // which would go wrong the moment the slab is not flat.
            let crescent = Path::new(|b| {
                b.circle(at(17.5, 21.0), 11.0 * scale);
                b.circle(at(24.0, 15.5), 9.5 * scale);
            });
            frame.fill(
                &crescent,
                canvas::Fill {
                    style: canvas::Style::Solid(self.palette.text_on_accent),
                    rule: canvas::fill::Rule::EvenOdd,
                },
            );

            // The star, as a four-pointed sparkle rather than a five-pointed
            // star: it reads at 24px, where a five-pointer turns to mush.
            let star = Path::new(|b| {
                let (cx, cy) = (28.5_f32, 27.5_f32);
                let (outer, inner) = (5.2_f32, 1.5_f32);
                b.move_to(at(cx, cy - outer));
                b.quadratic_curve_to(at(cx + inner, cy - inner), at(cx + outer, cy));
                b.quadratic_curve_to(at(cx + inner, cy + inner), at(cx, cy + outer));
                b.quadratic_curve_to(at(cx - inner, cy + inner), at(cx - outer, cy));
                b.quadratic_curve_to(at(cx - inner, cy - inner), at(cx, cy - outer));
                b.close();
            });
            frame.fill(&star, self.palette.text_on_accent);
        });

        vec![geometry]
    }
}
