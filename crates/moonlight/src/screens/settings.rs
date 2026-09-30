//! Settings: tunnel mode, the helper, appearance, language, support, updates.

use iced::widget::{button, column, container, row, text};
use iced::{Alignment, Border, Element, Length};

use moonlight_core::preferences::AUTO_UPDATE_CHOICES;
use moonlight_core::{format, AppLocale, TunnelMode};
use moonlight_design::motion::radii;
use moonlight_design::typography::{scale, EMPHATIC};
use moonlight_design::{icon, Icon};

use crate::components;
use crate::localization::{t, S};
use crate::{
    hspace, theme, vspace, Message, Moonlight, Page, UpdateState, TELEGRAM_CHANNEL_URL, VERSION,
};

pub fn view(app: &Moonlight) -> Element<'_, Message> {
    row![
        column![tunnel(app), system(app)]
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
        .style(move |_, status| theme::header_button(palette, status))
        .into(),
        palette,
    ));

    let mut section = column![
        components::overline(t(S::SectionTunnel, locale), palette),
        vspace(Length::Fixed(12.0)),
        components::surface(panel, palette),
    ];

    // A refused UAC prompt, or a missing moonlight-helper.exe, has to say so
    // *here* — beside the button that failed. The connect screen carries the
    // tunnel's errors, and a user who pressed Установить службу never goes
    // looking there for the reason nothing happened.
    if let Some(error) = app.last_error() {
        section = section.push(vspace(Length::Fixed(10.0)));
        section = section.push(
            container(
                text(error.to_string())
                    .size(scale::META)
                    .color(palette.danger),
            )
            .padding([0, 4]),
        );
    }

    section.into()
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

/// The СИСТЕМА group.
///
/// Launch at sign-in and the subscription's schedule. Minimise-to-tray and
/// connect-on-launch, which the composition also draws, are not implemented,
/// and a switch that flips and changes nothing is worse than an absent one.
fn system(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();

    let panel = column![
        components::setting_row(
            t(S::LaunchAtLogin, locale).to_string(),
            Some(t(S::LaunchAtLoginNote, locale).to_string()),
            components::toggle(
                app.preferences().launch_at_login,
                Message::ToggleLaunchAtLogin,
                palette,
            ),
            palette,
        ),
        components::divider(palette),
        components::setting_row(
            t(S::Notifications, locale).to_string(),
            Some(t(S::NotificationsNote, locale).to_string()),
            components::toggle(
                app.preferences().notifications,
                Message::ToggleNotifications,
                palette,
            ),
            palette,
        ),
        components::divider(palette),
        auto_update(app),
    ];

    column![
        components::overline(t(S::SectionSystem, locale), palette),
        vspace(Length::Fixed(12.0)),
        components::surface(panel, palette),
    ]
    .into()
}

/// How often the subscription refreshes itself: off, or every 1/6/12/24 h.
/// Until the user picks, it follows what the service suggests.
fn auto_update(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();

    let suggested = app.info().update_interval_hours;
    let hours = app.preferences().auto_update_hours(suggested);
    let labels: [&'static str; 5] = match locale {
        AppLocale::Ru => ["Выкл", "1 ч", "6 ч", "12 ч", "24 ч"],
        AppLocale::En => ["Off", "1 h", "6 h", "12 h", "24 h"],
    };
    let options: Vec<(u32, &str)> = AUTO_UPDATE_CHOICES.into_iter().zip(labels).collect();

    let mut note = t(S::AutoUpdateSub, locale).to_string();
    if let Some(updated) = app.last_updated() {
        note = format!("{note} · {updated}");
    }

    column![
        text(t(S::AutoUpdate, locale))
            .size(14.5)
            .font(moonlight_design::ui(EMPHATIC))
            .color(palette.text),
        text(note).size(12.0).color(palette.text_muted),
        vspace(Length::Fixed(10.0)),
        components::segmented(&options, hours, Message::SetAutoUpdate, palette),
    ]
    .spacing(2)
    .padding([15, 18])
    .into()
}

fn application(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();

    let language = components::segmented_compact(
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
            .style(move |_, status| theme::header_button(palette, status))
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
            palette.text,
            t(S::Support, locale).to_string(),
            t(S::SupportNote, locale).to_string(),
            Some(Icon::ExternalLink),
            Some(Message::OpenSupport),
            palette,
        ),
        components::divider(palette),
        components::action_row(
            Icon::CircleAlert,
            palette.cat3,
            palette.text,
            t(S::CoreLog, locale).to_string(),
            t(S::CoreLogNote, locale).to_string(),
            Some(Icon::ChevronRight),
            Some(Message::Navigate(Page::Logs)),
            palette,
        ),
        // Connections is a rail destination now, so a second way in from here
        // would be the same screen listed twice.
    ]
    .spacing(2);

    column![
        components::overline(t(S::SectionSupport, locale), palette),
        vspace(Length::Fixed(12.0)),
        components::surface(panel, palette),
    ]
    .into()
}

/// The version, and the update: one control through every state, and while
/// one downloads, how far it has got in real units and what the wait ends in.
fn about(app: &Moonlight) -> Element<'_, Message> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let update = app.update_state();

    let version_line = format!("{} {VERSION}", t(S::Version, locale));
    let (status, status_color) = match update {
        UpdateState::UpToDate => (
            format!("{version_line} · {}", t(S::UpToDate, locale)),
            palette.text_muted,
        ),
        UpdateState::Checking => (t(S::Checking, locale).to_string(), palette.text2),
        UpdateState::Failed(why) => (why.clone(), palette.danger),
        _ => (version_line, palette.text_muted),
    };

    let (label, action, primary) = match update {
        UpdateState::Available(release) => (
            t(S::UpdateTo, locale).replace("{version}", &release.version),
            Some(Message::StartUpdate),
            true,
        ),
        // Busy: the same control, not pressable, so a second press cannot start
        // a second download and the page does not move under the pointer.
        state if state.is_busy() => (t(S::CheckForUpdates, locale).to_string(), None, false),
        _ => (
            t(S::CheckForUpdates, locale).to_string(),
            Some(Message::CheckForUpdates),
            false,
        ),
    };
    let control = button(
        text(label)
            .size(scale::BODY_SM)
            .font(moonlight_design::ui(EMPHATIC)),
    )
    .on_press_maybe(action)
    .padding([10, 16]);
    let control = if primary {
        control.style(move |_, status| theme::accent_button(palette, status))
    } else {
        control.style(move |_, status| theme::header_button(palette, status))
    };

    let mut panel = column![row![
        column![
            text("moonlight")
                .font(moonlight_design::display())
                .size(scale::LEAD)
                .color(palette.text),
            text(status).size(scale::META).color(status_color),
        ]
        .spacing(2),
        hspace(Length::Fill),
        control,
    ]
    .align_y(Alignment::Center)
    .padding([14, 16])];

    if let Some(progress) = progress(app) {
        panel = panel.push(container(progress).padding(iced::Padding {
            top: 0.0,
            right: 16.0,
            bottom: 14.0,
            left: 16.0,
        }));
    }

    panel = panel.push(components::divider(palette)).push(
        row![
            icon(Icon::Lock, 15.0, palette.accent_ink),
            text(t(S::KeysStayLocal, locale))
                .size(scale::META)
                .color(palette.text_muted),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .padding([12, 16]),
    );

    components::surface(panel, palette)
}

/// "Загружаем moonlight 0.12.0", a bar, "12,3 МБ из 36,6 МБ", and what the
/// wait ends in. A bare spinner here once made someone give up and delete the
/// app.
fn progress(app: &Moonlight) -> Option<Element<'_, Message>> {
    let palette = app.palette_of();
    let locale = app.locale_of();
    let line = |content: String, color| text(content).size(scale::META).color(color);
    let block = match app.update_state() {
        UpdateState::Downloading {
            release,
            received,
            total,
        } => {
            let got = format::bytes(Some(*received as i64), locale);
            let (fraction, amount) = match total {
                Some(total) if *total > 0 => (
                    (*received as f32 / *total as f32).clamp(0.0, 1.0),
                    format!(
                        "{got} {} {}",
                        t(S::UpdateOf, locale),
                        format::bytes(Some(*total as i64), locale)
                    ),
                ),
                // An unknown total draws the bar full rather than empty: the
                // download is happening either way, and empty reads as stalled.
                _ => (1.0, got),
            };
            column![
                line(
                    t(S::UpdateDownloadingTo, locale).replace("{version}", &release.version),
                    palette.text
                ),
                components::bar(fraction, palette, 6.0),
                line(amount, palette.text2),
                line(t(S::UpdateThen, locale).to_string(), palette.text_muted),
            ]
        }
        UpdateState::Verifying(_) => column![
            line(t(S::UpdateVerifying, locale).to_string(), palette.text),
            components::bar(1.0, palette, 6.0),
            line(t(S::UpdateThen, locale).to_string(), palette.text_muted),
        ],
        UpdateState::Installing(version) => column![line(
            t(S::UpdateInstalling, locale).replace("{version}", version),
            palette.text
        )],
        _ => return None,
    };
    Some(block.spacing(8).into())
}
