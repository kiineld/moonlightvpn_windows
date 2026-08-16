//! Moonlight's design system: colour, type and motion tokens, the lucide icon
//! set, and the SVG path renderer that draws it.
//!
//! Everything here is a one-for-one port of the macOS client's
//! `MoonlightDesign` target, down to the hex literals and the bézier control
//! points, because the two are meant to be recognisably the same product. When
//! a token changes it changes in both or in neither.

pub mod icon_view;
pub mod icons;
pub mod motion;
pub mod palette;
pub mod svg_path;
pub mod typography;

pub use icon_view::{icon, icon_thin, IconView};
pub use icons::Icon;
pub use motion::{dur, radii, Curve};
pub use palette::{Appearance, Palette};
pub use svg_path::SvgPath;
pub use typography::{display, mono, ui, FONT_BYTES};
