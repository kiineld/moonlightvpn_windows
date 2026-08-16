//! One module per screen.
//!
//! Each exposes a single `view` that takes the palette, the locale and whatever
//! state it reads, and returns an `Element`. None of them hold state of their
//! own: the app owns all of it, so a screen change cannot strand a half-edited
//! field somewhere the user cannot get back to.

pub mod connect;
pub mod header;
pub mod placeholder;
pub mod sidebar;
