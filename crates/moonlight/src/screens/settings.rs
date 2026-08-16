//! Settings: tunnel mode, the helper, appearance, language, support, updates.

use iced::widget::{button, column, container, row, text};
use iced::{Alignment, Border, Element, Length};

use moonlight_core::{AppLocale, TunnelMode};
use moonlight_design::motion::radii;
use moonlight_design::typography::{scale, EMPHATIC};
use moonlight_design::{icon, Icon};

use crate::components;
use crate::localization::{t, S};
use crate::{
    hspace, theme, vspace, Message, Moonlight, Page, SUPPORT_URL, TELEGRAM_CHANNEL_URL, VERSION,
};

pub fn view(app: &Moonlight) -> Element<'_, Message> {
    row![
        column![tunnel(app)]
            .spacing(16)
            .width(Length::FillPortion(1)),
        column![application(app), support(app), about(app)]
            .spacing(16)
            .width(Length::FillPortion(1)),
    ]
    .spacing(20)
    .into()
}

fn tunnel(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let mode = app.preferences().mode;

    let mut panel = column![
        mode_row(
            app,
            TunnelMode::SystemProxy,
            S::ModeSystemProxy,
            S::ModeSystemProxyNote,
            mode == TunnelMode::SystemProxy,
            true,
        ),
        components::divider(palette),
        mode_row(
            app,
            TunnelMode::Tun,
            S::ModeTun,
            S::ModeTunNote,
            mode == TunnelMode::Tun,
            // TUN cannot be chosen without the service, so the row says so
            // rather than accepting a choice that fails on every connect.
            app.helper_installed(),
        ),
        components::divider(palette),
    ]
    .spacing(0);

    let (helper_title, helper_note, helper_action) = if app.helper_installed() {
        (
            t(S::HelperInstalled, locale),
            t(S::HelperNote, locale),
            (t(S::RemoveHelper, locale), Message::RemoveHelper),
        )
    } else {
        (
            t(S::HelperMissing, locale),
            t(S::ModeTunNote, locale),
            (t(S::InstallHelper, locale), Message::InstallHelper),
        )
    };

    panel = panel.push(components::setting_row(
        helper_title.to_string(),
        Some(helper_note.to_string()),
        button(
            text(helper_action.0)
                .size(scale::BODY_SM)
                .font(moonlight_design::ui(EMPHATIC)),
        )
        .on_press(helper_action.1)
        .padding([10, 16])
        .style(move |_, status| theme::ghost_button(palette, status))
        .into(),
        palette,
    ));

    column![
        components::overline(t(S::SectionTunnel, locale), palette),
        vspace(Length::Fixed(12.0)),
        components::surface(panel, palette),
    ]
    .into()
}

/// A radio row. The mark is drawn rather than using iced's radio, because that
/// one is a system control and this design's is a ring with an accent dot.
fn mode_row<'a>(
    app: &'a Moonlight,
    mode: TunnelMode,
    title: S,
    note: S,
    selected: bool,
    enabled: bool,
) -> Element<'a, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();

    let ring = container(if selected {
        container(hspace(Length::Fixed(10.0)))
            .height(Length::Fixed(10.0))
            .style(move |_| container::Style {
                background: Some(iced::Background::Color(palette.accent)),
                border: Border {
                    radius: iced::border::Radius::from(radii::PILL),
                    ..Default::default()
                },
                ..Default::default()
            })
    } else {
        container(hspace(Length::Fixed(0.0)))
    })
    .center_x(Length::Fixed(22.0))
    .center_y(Length::Fixed(22.0))
    .style(move |_| container::Style {
        border: Border {
            radius: iced::border::Radius::from(radii::PILL),
            width: 2.0,
            color: if selected {
                palette.accent_line
            } else {
                palette.hairline
            },
        },
        ..Default::default()
    });

    let ink = if enabled {
        palette.text
    } else {
        palette.text_muted
    };

    let content = row![
        ring,
        column![
            text(t(title, locale))
                .size(scale::BODY)
                .font(moonlight_design::ui(EMPHATIC))
                .color(ink),
            text(t(note, locale))
                .size(scale::META)
                .color(palette.text_muted),
        ]
        .spacing(2),
    ]
    .spacing(13)
    .align_y(Alignment::Center);

    button(content)
        .on_press_maybe(enabled.then_some(Message::SetMode(mode)))
        .padding([14, 16])
        .width(Length::Fill)
        .style(move |_, status| theme::row_button(palette, false, status))
        .into()
}

fn application(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();

    let language = components::segmented(
        &[(AppLocale::Ru, "RU"), (AppLocale::En, "EN")],
        locale,
        Message::SetLocale,
        palette,
    );

    let appearance_label = match app.preferences().appearance.as_deref() {
        Some("dark") => S::ThemeDark,
        Some("light") => S::ThemeLight,
        _ => S::ThemeSystem,
    };

    let panel = column![
        components::setting_row(t(S::Language, locale).to_string(), None, language, palette),
        components::divider(palette),
        components::setting_row(
            t(S::Appearance, locale).to_string(),
            None,
            button(
                text(t(appearance_label, locale))
                    .size(scale::BODY_SM)
                    .font(moonlight_design::ui(EMPHATIC))
            )
            .on_press(Message::CycleAppearance)
            .padding([10, 16])
            .style(move |_, status| theme::ghost_button(palette, status))
            .into(),
            palette,
        ),
    ];

    column![
        components::overline(t(S::SectionApp, locale), palette),
        vspace(Length::Fixed(12.0)),
        components::surface(panel, palette),
    ]
    .into()
}

fn support(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();

    let panel = column![
        components::action_row(
            Icon::MessageCircle,
            palette.accent,
            palette.text_on_accent,
            t(S::OurChannel, locale).to_string(),
            t(S::ChannelNote, locale).to_string(),
            Some(Icon::ExternalLink),
            Some(Message::OpenUrl(TELEGRAM_CHANNEL_URL)),
            palette,
        ),
        components::divider(palette),
        components::action_row(
            Icon::Headphones,
            palette.cat4,
            palette.text_on_accent,
            t(S::Support, locale).to_string(),
            t(S::SupportNote, locale).to_string(),
            Some(Icon::ExternalLink),
            Some(Message::OpenUrl(SUPPORT_URL)),
            palette,
        ),
        components::divider(palette),
        components::action_row(
            Icon::CircleAlert,
            palette.cat3,
            palette.text_on_accent,
            t(S::CoreLog, locale).to_string(),
            t(S::CoreLogNote, locale).to_string(),
            Some(Icon::ChevronRight),
            Some(Message::Navigate(Page::Logs)),
            palette,
        ),
        components::divider(palette),
        components::action_row(
            Icon::Globe,
            palette.cat2,
            palette.text_on_accent,
            t(S::NavConnections, locale).to_string(),
            t(S::ConnectionsNote, locale).to_string(),
            Some(Icon::ChevronRight),
            Some(Message::Navigate(Page::Connections)),
            palette,
        ),
    ]
    .spacing(2);

    column![
        components::overline(t(S::SectionSupport, locale), palette),
        vspace(Length::Fixed(12.0)),
        components::surface(panel, palette),
    ]
    .into()
}

fn about(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();

    let status: Element<'_, Message> = match app.update_status() {
        Some(status) => text(status.to_string())
            .size(scale::META)
            .color(palette.text2)
            .into(),
        None => text(format!("{} {VERSION}", t(S::Version, locale)))
            .size(scale::META)
            .color(palette.text_muted)
            .into(),
    };

    let panel = column![
        row![
            column![
                text("moonlight")
                    .font(moonlight_design::display())
                    .size(scale::LEAD)
                    .color(palette.text),
                status,
            ]
            .spacing(2),
            hspace(Length::Fill),
            button(
                text(t(S::CheckForUpdates, locale))
                    .size(scale::BODY_SM)
                    .font(moonlight_design::ui(EMPHATIC))
            )
            .on_press(Message::CheckForUpdates)
            .padding([10, 16])
            .style(move |_, status| theme::ghost_button(palette, status)),
        ]
        .align_y(Alignment::Center)
        .padding([14, 16]),
        components::divider(palette),
        row![
            icon(Icon::Lock, 15.0, palette.accent_ink),
            text(t(S::KeysStayLocal, locale))
                .size(scale::META)
                .color(palette.text_muted),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .padding([12, 16]),
    ];

    components::surface(panel, palette)
}
