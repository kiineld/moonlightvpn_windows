//! The window's resize edges.
//!
//! An undecorated window on Windows keeps `WS_THICKFRAME`, so the style bits
//! claim it is resizable — but the non-client area is collapsed to nothing, so
//! there is no border left for the OS to hit-test and the edges are dead. The
//! style bit being set is what made this look wired when it was not.
//!
//! So the app hit-tests them itself: eight transparent strips laid over the
//! window's own edges, each starting the OS resize loop in its direction. They
//! are `mouse_area`s rather than buttons because iced publishes a button's
//! `on_press` on *release*, by which time there is no drag left to start.

use iced::widget::{column, container, mouse_area, row, Space};
use iced::window::Direction;
use iced::{Element, Length};

use crate::Message;

/// How wide the grab strips are. Eight device pixels is the Windows convention
/// for a sizing border and is comfortably hittable without stealing clicks from
/// controls that sit near the window edge.
pub const GRAB: f32 = 8.0;

/// Wraps `content` in the eight resize zones.
///
/// The zones sit *over* the content rather than beside it, so they cost no
/// layout: the app underneath is laid out at full size and the strips are drawn
/// on top of its outermost 8px.
pub fn frame<'a>(content: Element<'a, Message>) -> Element<'a, Message> {
    let corner = |direction: Direction| -> Element<'a, Message> {
        mouse_area(
            container(Space::new())
                .width(Length::Fixed(GRAB))
                .height(Length::Fixed(GRAB)),
        )
        .on_press(Message::ResizeWindow(direction))
        .into()
    };

    let vertical_edge = |direction: Direction| -> Element<'a, Message> {
        mouse_area(
            container(Space::new())
                .width(Length::Fill)
                .height(Length::Fixed(GRAB)),
        )
        .on_press(Message::ResizeWindow(direction))
        .into()
    };

    let horizontal_edge = |direction: Direction| -> Element<'a, Message> {
        mouse_area(
            container(Space::new())
                .width(Length::Fixed(GRAB))
                .height(Length::Fill),
        )
        .on_press(Message::ResizeWindow(direction))
        .into()
    };

    let top = row![
        corner(Direction::NorthWest),
        vertical_edge(Direction::North),
        corner(Direction::NorthEast),
    ];

    let middle = row![
        horizontal_edge(Direction::West),
        // The interior is left untouched, so clicks fall through to the app.
        container(Space::new())
            .width(Length::Fill)
            .height(Length::Fill),
        horizontal_edge(Direction::East),
    ]
    .height(Length::Fill);

    let bottom = row![
        corner(Direction::SouthWest),
        vertical_edge(Direction::South),
        corner(Direction::SouthEast),
    ];

    let edges = column![top, middle, bottom]
        .width(Length::Fill)
        .height(Length::Fill);

    iced::widget::stack![content, edges].into()
}
