//! Every user-facing string, in both languages.
//!
//! Russian first, because the design is drawn in Russian and the product is
//! sold to a Russian-speaking audience. English exists so the app is legible to
//! anyone reading the source, and because the macOS client has it.
//!
//! A lookup returns a `&'static str` rather than a formatted `String` wherever
//! it can, because these are read inside view functions that run on every
//! redraw.

use moonlight_core::AppLocale;

macro_rules! strings {
    ($($name:ident => $ru:literal / $en:literal),* $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum S { $($name),* }

        impl S {
            pub fn get(self, locale: AppLocale) -> &'static str {
                match (self, locale) {
                    $(
                        (S::$name, AppLocale::Ru) => $ru,
                        (S::$name, AppLocale::En) => $en,
                    )*
                }
            }
        }

        #[allow(dead_code)] // read by the translation-coverage tests
        pub const ALL: &[S] = &[$(S::$name),*];
    };
}

strings! {
    // Navigation
    NavConnect      => "Подключение"        / "Connect",
    NavSubscription => "Подписка"           / "Subscription",
    NavApps         => "Приложения"         / "Apps",
    NavSettings     => "Настройки"          / "Settings",
    NavLogs         => "Логи"               / "Logs",
    NavConnections  => "Соединения"         / "Connections",

    // Connect screen
    ConnectSubtitle => "Выберите узел и включите туннель" / "Pick a node and switch the tunnel on",
    Connect         => "Подключить"         / "Connect",
    Disconnect      => "Отключить"          / "Disconnect",
    Connecting      => "Подключение…"       / "Connecting…",
    Disconnecting   => "Отключение…"        / "Disconnecting…",
    // "Защищено", not "Подключено": the dial reports what you have rather than
    // what happened, which is the composition's wording.
    StateSecure     => "Защищено"           / "Secure",
    // The dial's big label while the tunnel is up. "Соединение" is a noun that
    // reads as *connecting*, which is the one thing it is not; the macOS client
    // says "Подключено" and it is the state, not the act.
    Connection      => "Подключено"         / "Connected",
    StateConnected  => "ПОДКЛЮЧЕНО"         / "CONNECTED",
    StateDisconnected => "ОТКЛЮЧЕНО"        / "DISCONNECTED",
    StateFailed     => "ОШИБКА"             / "FAILED",
    PressToConnect  => "нажмите, чтобы подключиться" / "press to connect",
    PressToDisconnect => "нажмите, чтобы отключиться" / "press to disconnect",
    Ping            => "Пинг"               / "Ping",
    Measuring       => "Замер…"             / "Measuring…",
    Refresh         => "Обновить"           / "Refresh",
    Refreshing      => "Обновление"         / "Refreshing",
    Servers         => "СЕРВЕРЫ"            / "SERVERS",
    Auto            => "Авто"               / "Auto",
    AutoSubtitle    => "Ближайший узел по пингу" / "Lowest-latency node",
    AutoPicked      => "Выбран"             / "Using",
    Downloaded      => "СКАЧАНО"            / "DOWNLOADED",
    Uploaded        => "ОТДАНО"             / "UPLOADED",
    Remaining       => "ОСТАЛОСЬ"           / "REMAINING",
    TrafficLeft     => "ТРАФИКА"            / "TRAFFIC",

    // Subscription
    SubscriptionSubtitle => "Тариф, трафик и устройства" / "Plan, traffic and devices",
    Plan            => "Тариф"              / "Plan",
    Traffic         => "ТРАФИК"             / "TRAFFIC",
    Devices         => "УСТРОЙСТВА"         / "DEVICES",
    RefreshSubscription => "Обновить подписку" / "Refresh subscription",
    RefreshedJustNow => "Обновлено только что" / "Refreshed just now",
    OpensAccount    => "Откроется личный кабинет" / "Opens your account page",
    PasteFromBot    => "Вставить ссылку из бота" / "Paste a link from the bot",
    RemoveSubscription => "Удалить подписку" / "Remove subscription",
    ValidUntil      => "действует до"       / "valid until",
    Active          => "Активна"            / "Active",
    Expired         => "Истекла"            / "Expired",
    NoSubscription  => "Нет подписки"       / "No subscription",
    NoSubscriptionHint => "Добавьте ссылку из бота, чтобы увидеть серверы" / "Add a link from the bot to see servers",
    AddSubscription => "Добавить подписку"  / "Add a subscription",
    Unlimited       => "Безлимит"           / "Unlimited",
    ExtendSubscription => "Продлить подписку" / "Extend subscription",
    OfTraffic       => "трафика"            / "of traffic",

    // Import
    ImportTitle     => "Добавить подписку"  / "Add a subscription",
    ImportSubtitle  => "Ссылка из бота или личного кабинета" / "A link from the bot or your account page",
    ImportPlaceholder => "https://panel.example.com/sub/…" / "https://panel.example.com/sub/…",
    Import          => "Добавить"           / "Add",
    PasteFromClipboard => "Вставить из буфера" / "Paste from the clipboard",
    ImportHelp      => "Вставьте ссылку подписки из Telegram-бота или личного кабинета. Ключи останутся на этом компьютере." / "Paste the subscription link from your Telegram bot or account page. Keys stay on this computer.",
    BotSendsLink    => "Ссылка придёт в чат и добавится сама" / "The link arrives in the chat and adds itself",
    BackToSubscription => "Назад к подписке" / "Back to subscription",
    OpenTelegramBot => "Открыть Telegram-бот" / "Open the Telegram bot",

    // Apps / split tunnelling
    AppsSubtitle    => "Какой трафик идёт через туннель" / "Which traffic goes through the tunnel",
    SplitAll        => "Весь трафик"        / "All traffic",
    SplitOnly       => "Только эти"         / "Only these",
    SplitExcept     => "Кроме этих"         / "Except these",
    SplitNeedsTun   => "Правила по процессам работают только в режиме TUN" / "Process rules only work in TUN mode",
    SearchApps      => "Поиск"              / "Search",
    Programs        => "ПРОГРАММЫ"          / "PROGRAMS",
    RulesHeading    => "ПРАВИЛА"            / "RULES",
    RunningNow      => "Запущено"           / "Running",
    SplitAllNote    => "Через туннель идёт весь трафик компьютера." / "All traffic from this computer goes through the tunnel.",
    SplitOnlyNote   => "Через туннель пойдут только отмеченные программы — остальные напрямую." / "Only the ticked programs go through the tunnel; the rest go direct.",
    SplitExceptNote => "Отмеченные программы пойдут напрямую, весь остальной трафик — через туннель." / "The ticked programs go direct; all other traffic goes through the tunnel.",
    RulesFootnote   => "Правила по доменам, адресам и портам работают в обоих режимах. PROCESS-* требуют TUN." / "Domain, address and port rules work in both modes. PROCESS-* rules need TUN.",
    NoApps          => "Программы не найдены" / "No programs found",
    Rules           => "Правила"            / "Rules",
    AddRule         => "Добавить правило"   / "Add rule",

    // Settings
    SettingsSubtitle => "Система, приложение и поддержка" / "System, app and support",
    ModeSystemProxy => "Системный прокси"   / "System proxy",
    ModeTun         => "TUN"                / "TUN",
    ModeSystemProxyNote => "Не требует прав. Захватывает только приложения, которые уважают системный прокси." / "Needs no privileges. Captures only apps that honour the system proxy.",
    ModeTunNote     => "Захватывает весь трафик и включает правила по приложениям. Требует установки службы." / "Captures everything and enables per-app rules. Needs the helper service installed.",
    InstallHelper   => "Установить службу"  / "Install the helper",
    RemoveHelper    => "Удалить службу"     / "Remove the helper",
    HelperInstalled => "Служба установлена" / "Helper installed",
    Appearance      => "Оформление"         / "Appearance",
    ThemeSystem     => "Системное"          / "System",
    ThemeDark       => "Тёмное"             / "Dark",
    ThemeLight      => "Светлое"            / "Light",
    Language        => "Язык"               / "Language",
    LaunchAtLogin   => "Запускать при входе в систему" / "Launch at sign-in",
    LaunchAtLoginNote => "Moonlight запустится вместе с Windows" / "Moonlight starts with Windows",
    AutostartFailed => "Не удалось изменить автозапуск" / "Could not change the startup setting",
    MinimiseToTray  => "Свернуть в системный трей" / "Minimise to the system tray",
    MinimiseToTrayNote => "Окно закрывается в трей, туннель работает" / "Closing the window leaves the tunnel running",
    ThisDevice      => "Windows PC"         / "Windows PC",
    CheckForUpdates => "Проверить обновления" / "Check for updates",
    OurChannel      => "Наш канал"          / "Our channel",
    Support         => "Поддержка"          / "Support",
    Version         => "Версия"             / "Version",
    Checking        => "Проверяем…"         / "Checking…",
    Downloading     => "Загрузка обновления" / "Downloading the update",
    UpdateReady     => "Обновление загружено" / "The update is downloaded",
    InstallUpdate   => "Установить"          / "Install",
    LogBoth         => "Обе"                / "Both",
    LogClient       => "Клиент"             / "Client",
    LogCore         => "Ядро"               / "Core",
    SectionTunnel   => "ТУННЕЛЬ"            / "TUNNEL",
    SectionSystem   => "СИСТЕМА"            / "SYSTEM",
    SectionApp      => "ПРИЛОЖЕНИЕ"         / "APP",
    SectionSupport  => "ПОДДЕРЖКА"          / "SUPPORT",
    SectionDiagnostics => "ДИАГНОСТИКА"     / "DIAGNOSTICS",
    HelperNote      => "Один запрос прав администратора" / "One administrator prompt",
    HelperMissing   => "Служба не установлена" / "Helper not installed",
    HelperMissingBinary => "Рядом с приложением нет moonlight-helper.exe. Скачайте Moonlight-x86_64.zip целиком или установите через инсталлятор." / "moonlight-helper.exe is not next to the app. Download the full Moonlight-x86_64.zip, or use the installer.",
    HelperInstallFailed => "Не удалось установить службу. Подтвердите запрос администратора." / "Could not install the helper. Approve the administrator prompt and try again.",
    ChannelNote     => "Новости и обновления" / "News and updates",
    SupportNote     => "Мы на связи 24/7"    / "We are here 24/7",
    CoreLog         => "Журнал ядра"        / "Core log",
    CoreLogNote     => "Последние строки от mihomo" / "The last lines from mihomo",
    ConnectionsNote => "Куда идёт трафик прямо сейчас" / "Where traffic is going right now",
    KeysStayLocal   => "Ключи хранятся только на этом компьютере" / "Keys are kept only on this computer",

    // Logs / connections
    LogsSubtitle    => "Логи ядра и приложения на одной шкале" / "The core's log and the app's, on one timeline",
    ConnectionsSubtitle => "Какие программы идут через туннель" / "Which programs are going through the tunnel",
    FilterAll       => "Все"                / "All",
    CloseConnection => "Закрыть"            / "Close",
    NoConnections   => "Нет активных соединений" / "No open connections",
    ConnectionsNeedTunnel => "Подключения появятся, когда туннель заработает" / "Connections appear once the tunnel is up",
    ActiveConnections => "Активно"          / "Active",
    NoLogs          => "Пока ничего не записано" / "Nothing logged yet",
    CloseAll        => "Закрыть все"        / "Close all",
    ColProcess      => "ПРОЦЕСС"            / "PROCESS",
    ColHost         => "ХОСТ"               / "HOST",
    ClearLogs       => "Очистить"           / "Clear",
    FilterText      => "Фильтр"             / "Filter",

    // Shared
    Cancel          => "Отмена"             / "Cancel",
    Save            => "Сохранить"          / "Save",
    Delete          => "Удалить"            / "Delete",
    Copy            => "Скопировать"        / "Copy",
    Copied          => "Скопировано"        / "Copied",
    Unknown         => "—"                  / "—",
    Nodes           => "узлов"              / "nodes",
}

/// Shorthand so a view reads `t(S::Connect, locale)`.
pub fn t(string: S, locale: AppLocale) -> &'static str {
    string.get(locale)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_string_has_both_languages_and_neither_is_blank() {
        for string in ALL {
            assert!(
                !string.get(AppLocale::Ru).is_empty(),
                "{string:?} has no Russian"
            );
            assert!(
                !string.get(AppLocale::En).is_empty(),
                "{string:?} has no English"
            );
        }
    }

    #[test]
    fn the_russian_ui_is_actually_in_russian() {
        // A missing translation shows up as an English string in the Russian
        // column, which is easy to miss by eye across a hundred entries. The
        // few that are legitimately identical are listed here.
        const SAME_IN_BOTH: &[S] = &[
            S::ModeTun,
            S::Unknown,
            S::ImportPlaceholder,
            // A machine name, not a word.
            S::ThisDevice,
        ];

        for string in ALL {
            if SAME_IN_BOTH.contains(string) {
                continue;
            }
            assert_ne!(
                string.get(AppLocale::Ru),
                string.get(AppLocale::En),
                "{string:?} was never translated"
            );
        }
    }

    #[test]
    fn overline_labels_are_upper_case_in_both() {
        // The design sets these in caps rather than relying on a text
        // transform, so the strings themselves have to carry it.
        for string in [S::Servers, S::Downloaded, S::Uploaded, S::Remaining] {
            for locale in [AppLocale::Ru, AppLocale::En] {
                let value = string.get(locale);
                assert_eq!(value, value.to_uppercase(), "{string:?} is not in caps");
            }
        }
    }
}
