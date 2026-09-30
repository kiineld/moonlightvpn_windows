//! Every user-facing string, in both languages.
//!
//! Russian first, because the design is drawn in Russian and the product is
//! sold to a Russian-speaking audience. English exists so the app is legible to
//! anyone reading the source, and because the macOS client has it.
//!
//! A lookup returns a `&'static str` rather than a formatted `String` wherever
//! it can, because these are read inside view functions that run on every
//! redraw.

use moonlight_core::{AppLocale, Issue};

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
    NavSettings     => "Настройки"          / "Settings",
    NavLogs         => "Логи"               / "Logs",
    NavConnections  => "Подключения"        / "Connections",

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
    ExtendSubtitle  => "В Telegram-боте"    / "In the Telegram bot",
    PersonalAccount => "Личный кабинет"     / "Personal account",
    PersonalAccountSub => "Устройства и подписка на сайте" / "Devices and subscription on the website",
    RemoveSubscriptionSub => "Удаляет ссылку с этого компьютера" / "Removes the link from this computer",
    RefreshMetaIdle => "Проверить серверы, дни и трафик" / "Check servers, days and traffic",
    RefreshMetaSyncing => "Синхронизация с сервером…" / "Syncing with the server…",
    LastUpdated     => "Обновлено"          / "Updated",
    Ago             => "назад"              / "ago",
    TrafficResets   => "Трафик обновится"   / "Traffic resets",
    RefreshDone     => "Подписка обновлена" / "Subscription updated",
    RefreshFailed   => "Подписка не обновлена" / "Subscription not updated",
    OfTraffic       => "трафика"            / "of traffic",

    // Problems, worded by the app — never the service's or the core's text,
    // which can name the service or carry the link.
    IssueInvalidLink => "Это не похоже на ссылку подписки" / "This doesn't look like a subscription link",
    IssueNoSubscription => "Сначала добавьте подписку" / "Add a subscription first",
    IssueServerUnavailable => "Сервер подписки временно недоступен" / "The subscription server is temporarily unavailable",
    IssueErrorCode  => "ошибка"             / "error",
    IssueTryLater   => "Попробуйте позже."  / "Try again later.",
    IssueLinkRejected => "Ссылка больше не действует. Возьмите новую в боте." / "This link no longer works. Get a new one from the bot.",
    IssueEmpty      => "В подписке нет серверов" / "The subscription has no servers",
    IssueNoUsable   => "В подписке нет серверов, которые поддерживает приложение" / "None of the subscription's servers work with this app",
    IssueDeviceLimit => "Достигнут лимит устройств. Отключите другое устройство в личном кабинете." / "Device limit reached. Remove another device in your account.",
    IssueDeviceNotSupported => "Подписка не принимает это устройство" / "The subscription doesn't accept this device",
    IssueCoreFailed => "Не удалось запустить VPN. Подробности — в логах." / "Couldn't start the VPN. See the logs for details.",
    IssueRoutesTaken => "Маршруты заняты другим VPN. Закройте его или включите режим системного прокси." / "Another VPN owns the system routes. Quit it or use system proxy mode.",
    IssueTunFailed  => "Не удалось создать TUN-интерфейс. Подробности — в логах." / "Couldn't create the TUN interface. See the logs for details.",
    IssueHelperMissing => "Для TUN нужна служба — установите её в настройках" / "TUN needs the helper service — install it in Settings",

    // Import
    ImportTitle     => "Добавить подписку"  / "Add a subscription",
    ImportSubtitle  => "Ссылка из бота или личного кабинета" / "A link from the bot or your account page",
    ImportPlaceholder => "https://panel.example.com/sub/…" / "https://panel.example.com/sub/…",
    Import          => "Добавить"           / "Add",
    PasteFromClipboard => "Вставить из буфера" / "Paste from the clipboard",
    ImportHelp      => "Вставьте ссылку подписки из Telegram-бота или личного кабинета. Ключи останутся на этом компьютере." / "Paste the subscription link from your Telegram bot or account page. Keys stay on this computer.",
    BotSendsLink    => "Ссылка придёт в чат и добавится сама" / "The link arrives in the chat and adds itself",
    BackToSubscription => "Назад к подписке" / "Back to subscription",
    SubscriptionActivated => "Подписка активирована!" / "Subscription activated!",
    ConnectNow      => "Подключиться"       / "Connect",
    OpenTelegramBot => "Открыть Telegram-бот" / "Open the Telegram bot",

    SearchApps      => "Поиск"              / "Search",

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
    AutoUpdate      => "Автообновление подписки" / "Update the subscription automatically",
    AutoUpdateSub   => "Как часто проверять серверы, дни и трафик" / "How often to check servers, days and traffic",
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
    ConnectionsSubtitle => "Какие программы и куда идут прямо сейчас" / "Which programs are talking, and where to, right now",
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

    // Rules
    NavRules        => "Правила"            / "Rules",
    RulesSubtitle   => "Куда идёт трафик: ваши правила и правила подписки" / "Where traffic goes: your rules and the subscription's",
    RulesMine       => "Мои правила"        / "My rules",
    RulesProfile    => "Правила подписки"   / "Subscription rules",
    RulesFilter     => "Фильтр по типу, значению или цели" / "Filter by type, value or target",
    RulesAdd        => "Добавить правило"   / "Add rule",
    RulesEdit       => "Изменить правило"   / "Edit rule",
    RulesOwnCount   => "Своих"              / "Own",
    RulesProfileCount => "Правил"           / "Rules",
    ColType         => "ТИП"                / "TYPE",
    ColValue        => "ЗНАЧЕНИЕ"           / "VALUE",
    ColTarget       => "ЦЕЛЬ"               / "TARGET",
    ColPriority     => "ПРИОРИТЕТ"          / "PRIORITY",
    RuleType        => "Тип"                / "Type",
    RuleValue       => "Значение"           / "Value",
    RuleTarget      => "Цель"               / "Target",
    RulePriority    => "Приоритет"          / "Priority",
    RuleRunning     => "ЗАПУЩЕННЫЕ"         / "RUNNING",
    RuleInstalled   => "ПРОГРАММЫ"          / "APPS",
    TargetBuiltIn   => "ВСТРОЕННЫЕ"         / "BUILT-IN",
    TargetGroups    => "ГРУППЫ ПОДПИСКИ"    / "SUBSCRIPTION GROUPS",
    TargetDirect    => "Мимо VPN"           / "Bypass the VPN",
    TargetReject    => "Блокировать соединение" / "Block the connection",
    TargetMissing   => "Этой группы больше нет в подписке — правило пропускается" / "This group is no longer in the subscription — the rule is skipped",
    PriorityOverride => "Override"          / "Override",
    PriorityOverrideSub => "Применяется до правил подписки" / "Applied before the subscription's rules",
    PriorityExtend  => "Extend"             / "Extend",
    PriorityExtendSub => "Применяется после правил подписки" / "Applied after the subscription's rules",
    RuleTunOnly     => "Работает только в режиме TUN" / "Works in TUN mode only",
    RulesUnsaved    => "Изменения не применены" / "Changes not applied",
    RulesReset      => "Сбросить"           / "Reset",
    RulesApply      => "Применить"          / "Apply",
    RulesApplying   => "Применяются…"       / "Applying…",
    RulesEmpty      => "Своих правил пока нет" / "No rules of your own yet",
    RulesEmptyHint  => "Пустите сайт мимо VPN, заблокируйте его или направьте через группу. Свои правила переживают обновление подписки." / "Send a site around the VPN, block it or route it through a group. Your rules survive subscription updates.",
    RulesProfileEmpty => "В подписке нет правил" / "The subscription has no rules",
    RulesModeNote   => "Правила действуют в режиме «По правилам» — сейчас выбран другой" / "Rules apply in Rule mode — another mode is selected",
    InvalidEmpty    => "Укажите значение"   / "Enter a value",
    InvalidComma    => "В значении не может быть запятой" / "The value cannot contain a comma",
    InvalidRegex    => "Это не регулярное выражение" / "Not a regular expression",
    InvalidPort     => "Порт от 1 до 65535, диапазон через «-» или несколько через «/»" / "A port from 1 to 65535, a range with “-” or several joined by “/”",
    InvalidCidr     => "Нужна подсеть, например 192.168.1.0/24" / "A subnet, e.g. 192.168.1.0/24",
    InvalidAsn      => "Только номер AS, например 13335" / "The AS number alone, e.g. 13335",
    InvalidNetwork  => "tcp или udp"        / "tcp or udp",
    IssueRulesRefused => "Ядро не приняло эти правила — подробности в логах" / "The core rejected these rules — the log has the details",

    // Tray panel
    TrayStateOn     => "Подключено"         / "Connected",
    TrayStateOff    => "Отключено"          / "Disconnected",
    TrayStateFailed => "Ошибка"             / "Failed",
    TrayRules       => "По правилам"        / "Rules",
    TrayGlobal      => "Глобальный"         / "Global",
    TrayDirect      => "Напрямую"           / "Direct",
    SearchServers   => "Поиск серверов"     / "Search servers",
    PingAll         => "Пинг всех"          / "Ping all",
    NothingFound    => "Ничего не найдено"  / "Nothing found",
    OpenWindow      => "Открыть"            / "Open",
    DisconnectVerb  => "Отключиться"        / "Disconnect",

    // A moonlight:// link
    LinkTitle       => "Добавить подписку?" / "Add this subscription?",
    LinkBody        => "Ссылка с сайта или из чата хочет добавить подписку в moonlight." / "A link from a website or a chat wants to add a subscription to moonlight.",
    LinkReplaces    => "Она заменит текущую подписку." / "It will replace your current subscription.",
    LinkSame        => "Это ваша текущая подписка — она обновится." / "This is your current subscription — it will be updated.",
    LinkAdd         => "Добавить"           / "Add",
    LinkAdding      => "Загружаем подписку…" / "Loading the subscription…",
    LinkClose       => "Закрыть"            / "Close",
    LinkRetry       => "Ещё раз"            / "Try again",
    LinkFailedTitle => "Подписка не добавлена" / "Subscription not added",
    LinkInvalidTitle => "Эту ссылку не открыть" / "This link can't be opened",
    LinkInvalidBody => "В ней нет ссылки на подписку." / "It doesn't contain a subscription link.",

    // Notifications
    Notifications   => "Уведомления"        / "Notifications",
    NotificationsNote => "Когда заканчивается тариф или трафик" / "When the plan or traffic runs out",
    NotifyExpiringTitle => "Подписка заканчивается" / "Your subscription is ending",
    NotifyExpiringBody => "Осталось {days}. Продлите её в боте, чтобы не остаться без VPN." / "{days} left. Renew it in the bot to stay connected.",
    NotifyExpiredTitle => "Подписка закончилась" / "Your subscription has ended",
    NotifyExpiredBody => "Продлите её в боте, чтобы снова подключиться." / "Renew it in the bot to connect again.",
    NotifyTrafficLowTitle => "Трафик почти закончился" / "Traffic is running out",
    NotifyTrafficLowBody => "Осталось {left} из {total}." / "{left} left of {total}.",
    NotifyTrafficOutTitle => "Трафик закончился" / "You are out of traffic",
    NotifyTrafficOutBody => "Продлите подписку в боте, чтобы снова подключиться." / "Renew the subscription in the bot to connect again.",

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

/// The app's own words for a problem.
pub fn issue(issue: &Issue, locale: AppLocale) -> String {
    let s = |key| t(key, locale).to_string();
    match issue {
        Issue::InvalidLink => s(S::IssueInvalidLink),
        Issue::NoSubscription => s(S::IssueNoSubscription),
        Issue::ServerUnavailable(code) => {
            let status = code
                .map(|code| format!(" ({} {code})", t(S::IssueErrorCode, locale)))
                .unwrap_or_default();
            format!(
                "{}{status}. {}",
                t(S::IssueServerUnavailable, locale),
                t(S::IssueTryLater, locale)
            )
        }
        Issue::LinkRejected => s(S::IssueLinkRejected),
        Issue::EmptySubscription => s(S::IssueEmpty),
        Issue::NoUsableServers => s(S::IssueNoUsable),
        // The service's own explanation is the one text shown as it came: it
        // is written for users, and knows the plan's limit.
        Issue::DeviceLimit(message) => message.clone().unwrap_or_else(|| s(S::IssueDeviceLimit)),
        Issue::DeviceNotSupported => s(S::IssueDeviceNotSupported),
        Issue::CoreFailed => s(S::IssueCoreFailed),
        Issue::RoutesTaken => s(S::IssueRoutesTaken),
        Issue::TunFailed => s(S::IssueTunFailed),
        Issue::HelperMissing => s(S::IssueHelperMissing),
        Issue::RulesRefused => s(S::IssueRulesRefused),
    }
}

/// Why a rule's value will not do, in the app's words.
pub fn invalid(invalid: &moonlight_core::rules::Invalid, locale: AppLocale) -> &'static str {
    use moonlight_core::rules::Invalid;
    t(
        match invalid {
            Invalid::Empty => S::InvalidEmpty,
            Invalid::ContainsComma => S::InvalidComma,
            Invalid::BadRegex(_) => S::InvalidRegex,
            Invalid::BadPort => S::InvalidPort,
            Invalid::BadCidr => S::InvalidCidr,
            Invalid::BadAsn => S::InvalidAsn,
            Invalid::BadNetwork => S::InvalidNetwork,
        },
        locale,
    )
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
            // Flowvy's own terms for a rule's place, which the macOS client
            // keeps in Russian too.
            S::PriorityOverride,
            S::PriorityExtend,
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
