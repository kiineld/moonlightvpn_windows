//! One module per screen.
//!
//! Each exposes a single `view` that takes `&Moonlight` and returns an
//! `Element`. None of them hold state of their own: the app owns all of it, so
//! a screen change cannot strand a half-edited field somewhere the user cannot
//! get back to.

pub mod connect;
pub mod connections;
pub mod header;
pub mod import;
pub mod link;
pub mod logs;
pub mod resize;
pub mod rule_editor;
pub mod rules;
pub mod settings;
pub mod sidebar;
pub mod subscription;
pub mod titlebar;
pub mod tray;
pub mod update;
