//! The window's own title bar.
//!
//! The composition draws one rather than using the OS's, and specifies both
//! platforms: macOS gets traffic lights and a centred title, Windows gets the
//! logo on the left, a left-aligned title, and its own minimise / maximise /
//! close controls. This is the Windows one.
//!
//! ## Why draw it at all
//!
//! A native Windows caption is light grey with square corners and its own font.
//! Against a #0B111E rail with 8px corners it reads as a different application
//! wearing this one as a skin — which is the whole reason the design draws its
//! own. The cost is that dragging, maximising and closing all become this app's
//! job; `iced::window` provides each of them, and they are wired below.
//!
//! The title carries the connection state next to a status dot, so a user whose
//! window is behind something else can still read it from the taskbar preview.

use iced::widget::{button, canvas, container, row, text};
use iced::{Alignment, Background, Border, Element, Length};

use moonlight_core::ConnectionState;
use moonlight_design::motion::{metrics, radii};
use moonlight_design::typography::ROW_TITLE;
use moonlight_design::{icon, Icon, Palette};

use crate::localization::{t, S};
use crate::logo::Logo;
use crate::{hspace, Message, Moonlight};

/// Each caption control is a 40px square, as the composition sets them.
const CAPTION: f32 = 40.0;

pub fn view(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();

    let (label, tone) = match app.state() {
        ConnectionState::Connected => (S::StateSecure, palette.accent_ink),
        ConnectionState::Connecting => (S::Connecting, palette.text2),
        ConnectionState::Disconnecting => (S::Disconnecting, palette.text2),
        ConnectionState::Failed(_) => (S::StateFailed, palette.danger),
        ConnectionState::Disconnected => (S::StateDisconnected, palette.text_muted),
    };

    // 18px, radius 5 — the mark shrunk to caption size rather than the 32px one
    // from the rail.
    let mark = canvas(Logo::new(palette))
        .width(Length::Fixed(18.0))
        .height(Length::Fixed(18.0));

    let title = row![
        text("moonlight")
            .size(12.5)
            .font(moonlight_design::ui(ROW_TITLE))
            .color(palette.text_muted),
        container(crate::vspace(Length::Fixed(5.0)))
            .width(Length::Fixed(5.0))
            .style(move |_| container::Style {
                background: Some(Background::Color(tone)),
                border: Border {
                    radius: iced::border::Radius::from(radii::PILL),
                    ..Default::default()
                },
                ..Default::default()
            }),
        text(t(label, locale)).size(12.5).color(palette.text_muted),
    ]
    .spacing(9)
    .align_y(Alignment::Center);

    // The draggable region is everything that is not a control. A bar with no
    // drag region is a window that cannot be moved at all, since there is no
    // native caption behind it.
    let drag = button(
        row![mark, title]
            .spacing(12)
            .align_y(Alignment::Center)
            .width(Length::Fill),
    )
    .on_press(Message::DragWindow)
    .padding(0)
    .width(Length::Fill)
    .style(|_, _| button::Style {
        background: None,
        text_color: iced::Color::WHITE,
        ..Default::default()
    });

    let controls = row![
        caption(palette, Icon::Minus, 15.0, Message::MinimiseWindow, false),
        caption(palette, Icon::Square, 12.0, Message::MaximiseWindow, false),
        // Close is the one that gets a hover colour, because it is the one with
        // consequences — every Windows caption does this and its absence reads
        // as an unfinished control.
        caption(palette, Icon::X, 15.0, Message::CloseWindow, true),
    ]
    .spacing(2)
    .align_y(Alignment::Center);

    container(row![drag, hspace(Length::Fixed(8.0)), controls].align_y(Alignment::Center))
        // Centred rather than merely sized, for the same reason as the page
        // header: the caption controls happen to be exactly this tall today, so
        // top-alignment is invisible here until one of them changes.
        .center_y(Length::Fixed(metrics::TITLE_BAR))
        .padding(iced::Padding {
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
            left: 14.0,
        })
        .style(move |_| container::Style {
            background: Some(Background::Color(palette.bg_deep)),
            ..Default::default()
        })
        .into()
}

fn caption<'a>(
    palette: Palette,
    glyph: Icon,
    size: f32,
    message: Message,
    danger: bool,
) -> Element<'a, Message> {
    button(container(icon(glyph, size, palette.text_muted)).center(Length::Fill))
        .width(Length::Fixed(CAPTION))
        .height(Length::Fixed(CAPTION))
        .padding(0)
        .on_press(message)
        .style(move |_, status| {
            let background = match status {
                button::Status::Hovered | button::Status::Pressed => {
                    if danger {
                        palette.danger
                    } else {
                        palette.surface2
                    }
                }
                _ => iced::Color::TRANSPARENT,
            };
            button::Style {
                background: Some(Background::Color(background)),
                text_color: palette.text_muted,
                // Square, not rounded: these sit flush in the corner of the
                // window, and a radius there leaves a wedge of rail showing
                // through at the very corner.
                border: Border::default(),
                ..Default::default()
            }
        })
        .into()
}

/// The hairline under the bar.
pub fn rule<'a>(palette: Palette) -> Element<'a, Message> {
    container(crate::vspace(Length::Fixed(1.0)))
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(palette.hairline)),
            ..Default::default()
        })
        .into()
}
