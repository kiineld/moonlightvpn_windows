//! The window's caption: a band to drag it by and the three controls, floating
//! over the canvas.
//!
//! There is no title strip. The macOS client dropped its own for a flat canvas
//! with the sidebar floating under the traffic lights; here the minimise,
//! maximise and close controls sit in the top-right corner the same way, and
//! the band across the top is where the window is picked up. The state the old
//! strip carried stays in the window title, which the taskbar preview shows.

use iced::widget::{button, container, mouse_area, row};
use iced::{Alignment, Background, Border, Element, Length};

use moonlight_design::motion::metrics;
use moonlight_design::{icon, Icon, Palette};

use crate::{vspace, Message, Moonlight};

/// Each caption control, as Windows 11 sizes its own: 46 wide, the band tall.
const CAPTION_W: f32 = 46.0;

pub fn view(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();

    // `mouse_area`, not `button`: iced publishes a button's `on_press` on mouse
    // *release*, and Windows starts a move only while the button is still held.
    // Only what nothing else answers reaches it — the sidebar's logo, a page's
    // title — so the band never swallows a control.
    let drag = mouse_area(container(vspace(Length::Fixed(metrics::TITLE_BAR))).width(Length::Fill))
        .on_press(Message::DragWindow);

    let controls = row![
        caption(palette, Icon::Minus, 15.0, Message::MinimiseWindow, false),
        caption(palette, Icon::Square, 12.0, Message::MaximiseWindow, false),
        // Close alone gets a colour on hover, as every Windows caption does: it
        // is the one with consequences.
        caption(palette, Icon::X, 15.0, Message::CloseWindow, true),
    ]
    .align_y(Alignment::Center);

    row![drag, controls]
        .height(Length::Fixed(metrics::TITLE_BAR))
        .into()
}

fn caption<'a>(
    palette: Palette,
    glyph: Icon,
    size: f32,
    message: Message,
    danger: bool,
) -> Element<'a, Message> {
    button(container(icon(glyph, size, palette.text2)).center(Length::Fill))
        .width(Length::Fixed(CAPTION_W))
        .height(Length::Fixed(metrics::TITLE_BAR))
        .padding(0)
        .on_press(message)
        .style(move |_, status| {
            let background = match status {
                button::Status::Hovered | button::Status::Pressed => {
                    if danger {
                        palette.danger
                    } else {
                        palette.hairline
                    }
                }
                _ => iced::Color::TRANSPARENT,
            };
            button::Style {
                background: Some(Background::Color(background)),
                text_color: palette.text2,
                // Square: they sit flush in the window's corner, as the
                // system's own do.
                border: Border::default(),
                ..Default::default()
            }
        })
        .into()
}
