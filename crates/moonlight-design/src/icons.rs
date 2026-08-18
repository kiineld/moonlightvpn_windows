// Generated from scripts/gen-icons.py — do not edit by hand.
// Source: lucide-static 0.468.0 (ISC), the set the design is drawn with.
//
// Every element is carried across as path data, including the ones lucide
// draws as <circle>/<rect>/<line>/<polyline> — they are converted to path
// commands at generation time so the renderer only parses `d` strings. This is
// byte-for-byte the same geometry the macOS client ships, so the two clients
// cannot drift apart visually.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Icon {
    Activity,
    Check,
    ChevronLeft,
    ChevronDown,
    ChevronRight,
    CircleAlert,
    Copy,
    Download,
    ExternalLink,
    Globe,
    Headphones,
    Layers,
    Link2,
    LoaderCircle,
    Lock,
    MessageCircle,
    Minus,
    Monitor,
    Moon,
    PanelLeftClose,
    PanelLeftOpen,
    Play,
    Plus,
    Power,
    RefreshCw,
    Search,
    Send,
    Settings,
    Shield,
    Smartphone,
    Sparkles,
    Square,
    Sun,
    Trash2,
    WifiOff,
    X,
    Zap,
}

impl Icon {
    pub const ALL: &'static [Icon] = &[
        Icon::Activity,
        Icon::Check,
        Icon::ChevronLeft,
        Icon::ChevronDown,
        Icon::ChevronRight,
        Icon::CircleAlert,
        Icon::Copy,
        Icon::Download,
        Icon::ExternalLink,
        Icon::Globe,
        Icon::Headphones,
        Icon::Layers,
        Icon::Link2,
        Icon::LoaderCircle,
        Icon::Lock,
        Icon::MessageCircle,
        Icon::Minus,
        Icon::Monitor,
        Icon::Moon,
        Icon::PanelLeftClose,
        Icon::PanelLeftOpen,
        Icon::Play,
        Icon::Plus,
        Icon::Power,
        Icon::RefreshCw,
        Icon::Search,
        Icon::Send,
        Icon::Settings,
        Icon::Shield,
        Icon::Smartphone,
        Icon::Sparkles,
        Icon::Square,
        Icon::Sun,
        Icon::Trash2,
        Icon::WifiOff,
        Icon::X,
        Icon::Zap,
    ];

    /// lucide's own kebab-case name, as it appears in the icon set.
    pub fn name(self) -> &'static str {
        match self {
            Icon::Activity => "activity",
            Icon::Check => "check",
            Icon::ChevronLeft => "chevron-left",
            Icon::ChevronDown => "chevron-down",
            Icon::ChevronRight => "chevron-right",
            Icon::CircleAlert => "circle-alert",
            Icon::Copy => "copy",
            Icon::Download => "download",
            Icon::ExternalLink => "external-link",
            Icon::Globe => "globe",
            Icon::Headphones => "headphones",
            Icon::Layers => "layers",
            Icon::Link2 => "link-2",
            Icon::LoaderCircle => "loader-circle",
            Icon::Lock => "lock",
            Icon::MessageCircle => "message-circle",
            Icon::Minus => "minus",
            Icon::Monitor => "monitor",
            Icon::Moon => "moon",
            Icon::PanelLeftClose => "panel-left-close",
            Icon::PanelLeftOpen => "panel-left-open",
            Icon::Play => "play",
            Icon::Plus => "plus",
            Icon::Power => "power",
            Icon::RefreshCw => "refresh-cw",
            Icon::Search => "search",
            Icon::Send => "send",
            Icon::Settings => "settings",
            Icon::Shield => "shield",
            Icon::Smartphone => "smartphone",
            Icon::Sparkles => "sparkles",
            Icon::Square => "square",
            Icon::Sun => "sun",
            Icon::Trash2 => "trash-2",
            Icon::WifiOff => "wifi-off",
            Icon::X => "x",
            Icon::Zap => "zap",
        }
    }

    /// SVG path `d` strings in lucide's 24×24 viewBox, stroked, never filled.
    pub fn paths(self) -> &'static [&'static str] {
        match self {
            Icon::Activity => &["M22 12h-2.48a2 2 0 0 0-1.93 1.46l-2.35 8.36a.25.25 0 0 1-.48 0L9.24 2.18a.25.25 0 0 0-.48 0l-2.35 8.36A2 2 0 0 1 4.49 12H2"],
            Icon::Check => &["M20 6 9 17l-5-5"],
            Icon::ChevronLeft => &["m15 18-6-6 6-6"],
            Icon::ChevronDown => &["m6 9 6 6 6-6"],
            Icon::ChevronRight => &["m9 18 6-6-6-6"],
            Icon::CircleAlert => &["M2.0 12.0a10.0 10.0 0 1 0 20.0 0a10.0 10.0 0 1 0 -20.0 0Z", "M12.0 8.0L12.0 12.0", "M12.0 16.0L12.01 16.0"],
            Icon::Copy => &["M10.0 8.0H20.0a2.0 2.0 0 0 1 2.0 2.0V20.0a2.0 2.0 0 0 1 -2.0 2.0H10.0a2.0 2.0 0 0 1 -2.0 -2.0V10.0a2.0 2.0 0 0 1 2.0 -2.0Z", "M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"],
            Icon::Download => &["M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4", "M7 10L12 15L17 10", "M12.0 15.0L12.0 3.0"],
            Icon::ExternalLink => &["M15 3h6v6", "M10 14 21 3", "M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"],
            Icon::Globe => &["M2.0 12.0a10.0 10.0 0 1 0 20.0 0a10.0 10.0 0 1 0 -20.0 0Z", "M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20", "M2 12h20"],
            Icon::Headphones => &["M3 14h3a2 2 0 0 1 2 2v3a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-7a9 9 0 0 1 18 0v7a2 2 0 0 1-2 2h-1a2 2 0 0 1-2-2v-3a2 2 0 0 1 2-2h3"],
            Icon::Layers => &["M12.83 2.18a2 2 0 0 0-1.66 0L2.6 6.08a1 1 0 0 0 0 1.83l8.58 3.91a2 2 0 0 0 1.66 0l8.58-3.9a1 1 0 0 0 0-1.83z", "M2 12a1 1 0 0 0 .58.91l8.6 3.91a2 2 0 0 0 1.65 0l8.58-3.9A1 1 0 0 0 22 12", "M2 17a1 1 0 0 0 .58.91l8.6 3.91a2 2 0 0 0 1.65 0l8.58-3.9A1 1 0 0 0 22 17"],
            Icon::Link2 => &["M9 17H7A5 5 0 0 1 7 7h2", "M15 7h2a5 5 0 1 1 0 10h-2", "M8.0 12.0L16.0 12.0"],
            Icon::LoaderCircle => &["M21 12a9 9 0 1 1-6.219-8.56"],
            Icon::Lock => &["M5.0 11.0H19.0a2.0 2.0 0 0 1 2.0 2.0V20.0a2.0 2.0 0 0 1 -2.0 2.0H5.0a2.0 2.0 0 0 1 -2.0 -2.0V13.0a2.0 2.0 0 0 1 2.0 -2.0Z", "M7 11V7a5 5 0 0 1 10 0v4"],
            Icon::MessageCircle => &["M7.9 20A9 9 0 1 0 4 16.1L2 22Z"],
            Icon::Minus => &["M5 12h14"],
            Icon::Monitor => &["M4.0 3.0H20.0a2.0 2.0 0 0 1 2.0 2.0V15.0a2.0 2.0 0 0 1 -2.0 2.0H4.0a2.0 2.0 0 0 1 -2.0 -2.0V5.0a2.0 2.0 0 0 1 2.0 -2.0Z", "M8.0 21.0L16.0 21.0", "M12.0 17.0L12.0 21.0"],
            Icon::Moon => &["M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9Z"],
            Icon::PanelLeftClose => &["M5.0 3.0H19.0a2.0 2.0 0 0 1 2.0 2.0V19.0a2.0 2.0 0 0 1 -2.0 2.0H5.0a2.0 2.0 0 0 1 -2.0 -2.0V5.0a2.0 2.0 0 0 1 2.0 -2.0Z", "M9 3v18", "m16 15-3-3 3-3"],
            Icon::PanelLeftOpen => &["M5.0 3.0H19.0a2.0 2.0 0 0 1 2.0 2.0V19.0a2.0 2.0 0 0 1 -2.0 2.0H5.0a2.0 2.0 0 0 1 -2.0 -2.0V5.0a2.0 2.0 0 0 1 2.0 -2.0Z", "M9 3v18", "m14 9 3 3-3 3"],
            Icon::Play => &["M6 3L20 12L6 21L6 3Z"],
            Icon::Plus => &["M5 12h14", "M12 5v14"],
            Icon::Power => &["M12 2v10", "M18.4 6.6a9 9 0 1 1-12.77.04"],
            Icon::RefreshCw => &["M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8", "M21 3v5h-5", "M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16", "M8 16H3v5"],
            Icon::Search => &["M3.0 11.0a8.0 8.0 0 1 0 16.0 0a8.0 8.0 0 1 0 -16.0 0Z", "m21 21-4.3-4.3"],
            Icon::Send => &["M14.536 21.686a.5.5 0 0 0 .937-.024l6.5-19a.496.496 0 0 0-.635-.635l-19 6.5a.5.5 0 0 0-.024.937l7.93 3.18a2 2 0 0 1 1.112 1.11z", "m21.854 2.147-10.94 10.939"],
            Icon::Settings => &["M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z", "M9.0 12.0a3.0 3.0 0 1 0 6.0 0a3.0 3.0 0 1 0 -6.0 0Z"],
            Icon::Shield => &["M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z"],
            Icon::Smartphone => &["M7.0 2.0H17.0a2.0 2.0 0 0 1 2.0 2.0V20.0a2.0 2.0 0 0 1 -2.0 2.0H7.0a2.0 2.0 0 0 1 -2.0 -2.0V4.0a2.0 2.0 0 0 1 2.0 -2.0Z", "M12 18h.01"],
            Icon::Sparkles => &["M9.937 15.5A2 2 0 0 0 8.5 14.063l-6.135-1.582a.5.5 0 0 1 0-.962L8.5 9.936A2 2 0 0 0 9.937 8.5l1.582-6.135a.5.5 0 0 1 .963 0L14.063 8.5A2 2 0 0 0 15.5 9.937l6.135 1.581a.5.5 0 0 1 0 .964L15.5 14.063a2 2 0 0 0-1.437 1.437l-1.582 6.135a.5.5 0 0 1-.963 0z", "M20 3v4", "M22 5h-4", "M4 17v2", "M5 18H3"],
            Icon::Square => &["M5.0 3.0H19.0a2.0 2.0 0 0 1 2.0 2.0V19.0a2.0 2.0 0 0 1 -2.0 2.0H5.0a2.0 2.0 0 0 1 -2.0 -2.0V5.0a2.0 2.0 0 0 1 2.0 -2.0Z"],
            Icon::Sun => &["M8.0 12.0a4.0 4.0 0 1 0 8.0 0a4.0 4.0 0 1 0 -8.0 0Z", "M12 2v2", "M12 20v2", "m4.93 4.93 1.41 1.41", "m17.66 17.66 1.41 1.41", "M2 12h2", "M20 12h2", "m6.34 17.66-1.41 1.41", "m19.07 4.93-1.41 1.41"],
            Icon::Trash2 => &["M3 6h18", "M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6", "M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2", "M10.0 11.0L10.0 17.0", "M14.0 11.0L14.0 17.0"],
            Icon::WifiOff => &["M12 20h.01", "M8.5 16.429a5 5 0 0 1 7 0", "M5 12.859a10 10 0 0 1 5.17-2.69", "M19 12.859a10 10 0 0 0-2.007-1.523", "M2 8.82a15 15 0 0 1 4.177-2.643", "M22 8.82a15 15 0 0 0-11.288-3.764", "m2 2 20 20"],
            Icon::X => &["M18 6 6 18", "m6 6 12 12"],
            Icon::Zap => &["M4 14a1 1 0 0 1-.78-1.63l9.9-10.2a.5.5 0 0 1 .86.46l-1.92 6.02A1 1 0 0 0 13 10h7a1 1 0 0 1 .78 1.63l-9.9 10.2a.5.5 0 0 1-.86-.46l1.92-6.02A1 1 0 0 0 11 14z"],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::svg_path::SvgPath;

    #[test]
    fn every_icon_has_at_least_one_path() {
        for icon in Icon::ALL {
            assert!(
                !icon.paths().is_empty(),
                "{} carries no path data",
                icon.name()
            );
        }
    }

    #[test]
    fn every_icon_path_parses_to_commands() {
        // A `d` string that produces nothing is a glyph that renders blank —
        // silent in a way a missing icon is not.
        for icon in Icon::ALL {
            for d in icon.paths() {
                let parsed = SvgPath::parse(d);
                assert!(
                    !parsed.commands.is_empty(),
                    "{} produced no commands from {d:?}",
                    icon.name()
                );
            }
        }
    }

    #[test]
    fn every_icon_path_starts_with_a_move() {
        for icon in Icon::ALL {
            for d in icon.paths() {
                let parsed = SvgPath::parse(d);
                assert!(
                    matches!(parsed.commands[0], crate::svg_path::Command::Move(_)),
                    "{} has a subpath that does not open with a moveto",
                    icon.name()
                );
            }
        }
    }

    #[test]
    fn names_are_unique() {
        let mut names: Vec<&str> = Icon::ALL.iter().map(|i| i.name()).collect();
        names.sort_unstable();
        let count = names.len();
        names.dedup();
        assert_eq!(count, names.len(), "duplicate icon name");
    }
}
