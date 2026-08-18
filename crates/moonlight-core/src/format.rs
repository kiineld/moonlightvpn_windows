//! Byte, duration and date formatting, matching the strings the design shows.
//!
//! The design is Russian-first and writes sizes as `24,8 ГБ` — decimal comma,
//! one fraction digit, a non-breaking space before the unit. English is
//! `24.8 GB`. Both come out of here so a screen never hand-rolls a number.

use time::{Month, OffsetDateTime};

use crate::models::AppLocale;

const NBSP: char = '\u{00A0}';

const UNITS_RU: [&str; 6] = ["Б", "КБ", "МБ", "ГБ", "ТБ", "ПБ"];
const UNITS_EN: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];

fn units(locale: AppLocale) -> [&'static str; 6] {
    match locale {
        AppLocale::Ru => UNITS_RU,
        AppLocale::En => UNITS_EN,
    }
}

/// Binary units (1024), which is what a panel's `subscription-userinfo` byte
/// counts mean.
/// A panel-provided name with emoji and stray symbols removed.
///
/// The subscription title comes from the panel — "moonlight vpn 🌙" — and the
/// emoji renders through the OS emoji font as a colour glyph that has nothing to
/// do with this flat, two-colour design: on the lime hero card it is a bright
/// orange blob beside the wordmark. The drawn crescent in the logo is already
/// the product's moon; the one in the string is noise.
///
/// Kept deliberately narrow: it strips the pictographic ranges and their
/// joiners and variation selectors, and trims the space they leave, so a name in
/// any actual script — Cyrillic, Latin, anything `is_alphanumeric` — is
/// untouched.
pub fn without_emoji(text: &str) -> String {
    let stripped: String = text
        .chars()
        .filter(|c| {
            let u = *c as u32;
            let pictographic = (0x1F000..=0x1FAFF).contains(&u) // symbols, emoji, supplemental
                || (0x2600..=0x27BF).contains(&u)               // misc symbols and dingbats
                || (0x2B00..=0x2BFF).contains(&u)               // misc symbols and arrows
                || (0x1F1E6..=0x1F1FF).contains(&u)             // regional indicators
                || u == 0x200D                                  // zero-width joiner
                || (0xFE00..=0xFE0F).contains(&u); // variation selectors
            !pictographic
        })
        .collect();
    stripped.trim().to_string()
}

pub fn bytes(value: Option<i64>, locale: AppLocale) -> String {
    let Some(value) = value else {
        return "—".to_string();
    };
    let units = units(locale);

    let mut amount = value as f64;
    let mut index = 0;
    while amount.abs() >= 1024.0 && index < units.len() - 1 {
        amount /= 1024.0;
        index += 1;
    }
    // Bytes and kilobytes have no meaningful fraction, and neither does a
    // three-figure amount — "150 GB" has to stay on one line beside "of".
    let digits = usize::from(index > 1 && amount.abs() < 100.0);
    format!("{}{NBSP}{}", decimal(amount, digits, locale), units[index])
}

/// A transfer rate. The design's connect screen updates this every second, so
/// it stays on one line at any magnitude.
pub fn rate(bytes_per_second: Option<i64>, locale: AppLocale) -> String {
    let Some(value) = bytes_per_second else {
        return "—".to_string();
    };
    let suffix = match locale {
        AppLocale::Ru => "/с",
        AppLocale::En => "/s",
    };
    format!("{}{suffix}", bytes(Some(value), locale))
}

/// `HH:MM:SS`, the connect dial's timer. Hours are not capped at 24 — a tunnel
/// up for two days reads `48:12:07`, not `00:12:07`.
pub fn duration(seconds: i64) -> String {
    let seconds = seconds.max(0);
    format!(
        "{:02}:{:02}:{:02}",
        seconds / 3600,
        (seconds / 60) % 60,
        seconds % 60
    )
}

/// Russian's three-way plural. `one` is 1, 21, 31…; `few` is 2–4, 22–24…;
/// `many` is everything else, including the whole 11–14 band, which is the case
/// a naive `n % 10` gets wrong.
fn plural_ru<'a>(count: i64, one: &'a str, few: &'a str, many: &'a str) -> &'a str {
    let mod100 = count.rem_euclid(100);
    let mod10 = count.rem_euclid(10);
    if (11..=14).contains(&mod100) {
        many
    } else if mod10 == 1 {
        one
    } else if (2..=4).contains(&mod10) {
        few
    } else {
        many
    }
}

/// "12 дней" / "12 days".
pub fn days(count: Option<i64>, locale: AppLocale) -> String {
    let Some(count) = count else {
        return match locale {
            AppLocale::Ru => "без срока".to_string(),
            AppLocale::En => "no expiry".to_string(),
        };
    };
    match locale {
        AppLocale::En => format!("{count} day{}", if count == 1 { "" } else { "s" }),
        AppLocale::Ru => format!("{count} {}", plural_ru(count, "день", "дня", "дней")),
    }
}

/// Russian's three-way plural again, for hours.
pub fn hours(count: i64, locale: AppLocale) -> String {
    match locale {
        AppLocale::En => format!("{count} hour{}", if count == 1 { "" } else { "s" }),
        AppLocale::Ru => format!("{count} {}", plural_ru(count, "час", "часа", "часов")),
    }
}

/// "3 слота" / "3 slots" — the free device slots line.
pub fn slots(count: i64, locale: AppLocale) -> String {
    match locale {
        AppLocale::En => format!("{count} slot{}", if count == 1 { "" } else { "s" }),
        AppLocale::Ru => format!("{count} {}", plural_ru(count, "слот", "слота", "слотов")),
    }
}

/// Days left, or hours once it is under a day — "12 дней", "7 часов".
///
/// A plan with nine hours on it reading "1 день" is the kind of rounding that
/// loses someone a day of service.
pub fn time_left(expire: Option<i64>, locale: AppLocale) -> String {
    let Some(expire) = expire else {
        return days(None, locale);
    };
    let seconds = expire - OffsetDateTime::now_utc().unix_timestamp();
    if seconds <= 0 {
        return days(Some(0), locale);
    }
    if seconds >= 86_400 {
        return days(Some((seconds as f64 / 86_400.0).ceil() as i64), locale);
    }
    hours(((seconds as f64 / 3600.0).ceil() as i64).max(1), locale)
}

/// "24,8 из 100 ГБ" / "24.8 of 100 GB". An unlimited plan says so rather than
/// showing a denominator it does not have.
pub fn quota(used: Option<i64>, total: Option<i64>, locale: AppLocale) -> String {
    let Some(total) = total.filter(|t| *t > 0) else {
        let used_text = bytes(used, locale);
        return match locale {
            AppLocale::Ru => format!("{used_text} · без лимита"),
            AppLocale::En => format!("{used_text} · unlimited"),
        };
    };
    // The unit is taken from the total so both halves read in the same one.
    let total_text = bytes(Some(total), locale);
    let unit = total_text.rsplit(NBSP).next().unwrap_or("");
    let scale = unit_scale(unit, locale);
    let used_value = decimal(used.unwrap_or(0) as f64 / scale, 1, locale);
    match locale {
        AppLocale::Ru => format!("{used_value} из {total_text}"),
        AppLocale::En => format!("{used_value} of {total_text}"),
    }
}

/// How long ago something started: "4 с", "2 мин", "1 ч".
pub fn age(seconds_ago: i64, locale: AppLocale) -> String {
    let seconds = seconds_ago.max(0);
    if seconds < 60 {
        return format!(
            "{seconds} {}",
            if locale == AppLocale::Ru { "с" } else { "s" }
        );
    }
    if seconds < 3600 {
        return format!(
            "{} {}",
            seconds / 60,
            if locale == AppLocale::Ru {
                "мин"
            } else {
                "m"
            }
        );
    }
    format!(
        "{} {}",
        seconds / 3600,
        if locale == AppLocale::Ru { "ч" } else { "h" }
    )
}

/// `n/a` rather than a dash for a node that has not answered: a dash reads as
/// "not measured yet", and the two are worth telling apart when one of them
/// means the node is down.
/// A node's latency.
///
/// `probed` separates the two things a missing number can mean. Before any probe
/// has run the answer is simply unknown, and a dash says so; `n/a` there claims
/// the server failed to answer a question it was never asked. Once a probe has
/// finished and come back with nothing, `n/a` is the truth.
pub fn latency(ms: Option<u32>, probed: bool) -> String {
    match ms {
        Some(ms) => format!("{ms} ms"),
        None if probed => "n/a".to_string(),
        None => "—".to_string(),
    }
}

/// "1 сентября" — the reset/expiry date line on the subscription card.
pub fn date(timestamp: Option<i64>, locale: AppLocale) -> String {
    let Some(date) = timestamp.and_then(|t| OffsetDateTime::from_unix_timestamp(t).ok()) else {
        return "—".to_string();
    };
    match locale {
        AppLocale::Ru => format!("{} {}", date.day(), month_ru_genitive(date.month())),
        AppLocale::En => format!("{} {}", date.day(), month_en(date.month())),
    }
}

/// Russian dates in this position take the genitive — "1 сентября", not
/// "1 сентябрь".
fn month_ru_genitive(month: Month) -> &'static str {
    match month {
        Month::January => "января",
        Month::February => "февраля",
        Month::March => "марта",
        Month::April => "апреля",
        Month::May => "мая",
        Month::June => "июня",
        Month::July => "июля",
        Month::August => "августа",
        Month::September => "сентября",
        Month::October => "октября",
        Month::November => "ноября",
        Month::December => "декабря",
    }
}

fn month_en(month: Month) -> &'static str {
    match month {
        Month::January => "January",
        Month::February => "February",
        Month::March => "March",
        Month::April => "April",
        Month::May => "May",
        Month::June => "June",
        Month::July => "July",
        Month::August => "August",
        Month::September => "September",
        Month::October => "October",
        Month::November => "November",
        Month::December => "December",
    }
}

fn unit_scale(unit: &str, locale: AppLocale) -> f64 {
    let units = units(locale);
    let index = units.iter().position(|u| *u == unit).unwrap_or(0);
    1024_f64.powi(index as i32)
}

fn decimal(value: f64, digits: usize, locale: AppLocale) -> String {
    let text = format!("{value:.digits$}");
    match locale {
        AppLocale::Ru => text.replace('.', ","),
        AppLocale::En => text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GB: i64 = 1024 * 1024 * 1024;

    #[test]
    fn sizes_use_binary_units() {
        assert_eq!(bytes(Some(1024), AppLocale::En), format!("1{NBSP}KB"));
        assert_eq!(bytes(Some(GB), AppLocale::En), format!("1.0{NBSP}GB"));
    }

    #[test]
    fn russian_sizes_use_a_decimal_comma() {
        assert_eq!(
            bytes(Some(GB * 5 / 2), AppLocale::Ru),
            format!("2,5{NBSP}ГБ")
        );
    }

    #[test]
    fn bytes_and_kilobytes_carry_no_fraction() {
        assert_eq!(bytes(Some(512), AppLocale::En), format!("512{NBSP}B"));
        assert_eq!(bytes(Some(2048), AppLocale::En), format!("2{NBSP}KB"));
    }

    #[test]
    fn three_figures_drop_the_fraction_to_stay_on_one_line() {
        assert_eq!(bytes(Some(GB * 150), AppLocale::En), format!("150{NBSP}GB"));
    }

    #[test]
    fn an_unknown_size_is_a_dash_not_a_zero() {
        assert_eq!(bytes(None, AppLocale::Ru), "—");
        assert_eq!(rate(None, AppLocale::Ru), "—");
    }

    #[test]
    fn duration_does_not_wrap_at_a_day() {
        assert_eq!(duration(0), "00:00:00");
        assert_eq!(duration(3661), "01:01:01");
        assert_eq!(duration(48 * 3600 + 12 * 60 + 7), "48:12:07");
    }

    #[test]
    fn negative_durations_floor_at_zero() {
        assert_eq!(duration(-5), "00:00:00");
    }

    #[test]
    fn russian_plurals_cover_all_three_forms() {
        assert_eq!(days(Some(1), AppLocale::Ru), "1 день");
        assert_eq!(days(Some(2), AppLocale::Ru), "2 дня");
        assert_eq!(days(Some(5), AppLocale::Ru), "5 дней");
        assert_eq!(days(Some(21), AppLocale::Ru), "21 день");
        assert_eq!(days(Some(22), AppLocale::Ru), "22 дня");
    }

    #[test]
    fn the_eleven_to_fourteen_band_is_the_case_a_naive_rule_gets_wrong() {
        for n in 11..=14 {
            assert_eq!(days(Some(n), AppLocale::Ru), format!("{n} дней"));
        }
        assert_eq!(hours(11, AppLocale::Ru), "11 часов");
        assert_eq!(slots(12, AppLocale::Ru), "12 слотов");
    }

    #[test]
    fn english_plurals_are_the_simple_rule() {
        assert_eq!(days(Some(1), AppLocale::En), "1 day");
        assert_eq!(days(Some(2), AppLocale::En), "2 days");
        assert_eq!(hours(1, AppLocale::En), "1 hour");
        assert_eq!(slots(3, AppLocale::En), "3 slots");
    }

    #[test]
    fn no_expiry_says_so_rather_than_showing_a_count() {
        assert_eq!(days(None, AppLocale::Ru), "без срока");
        assert_eq!(days(None, AppLocale::En), "no expiry");
        assert_eq!(time_left(None, AppLocale::Ru), "без срока");
    }

    #[test]
    fn under_a_day_switches_to_hours() {
        let now = OffsetDateTime::now_utc().unix_timestamp();
        // Nine hours must not round up into "1 день".
        assert_eq!(time_left(Some(now + 9 * 3600), AppLocale::Ru), "9 часов");
        assert_eq!(time_left(Some(now + 2 * 86_400), AppLocale::Ru), "2 дня");
    }

    #[test]
    fn a_part_hour_still_reads_as_an_hour_not_as_zero() {
        let now = OffsetDateTime::now_utc().unix_timestamp();
        assert_eq!(time_left(Some(now + 60), AppLocale::Ru), "1 час");
    }

    #[test]
    fn an_expired_plan_reads_zero_days() {
        let now = OffsetDateTime::now_utc().unix_timestamp();
        assert_eq!(time_left(Some(now - 10), AppLocale::Ru), "0 дней");
    }

    #[test]
    fn quota_states_both_halves_in_the_same_unit() {
        assert_eq!(
            quota(Some(GB * 25), Some(GB * 100), AppLocale::Ru),
            format!("25,0 из 100{NBSP}ГБ")
        );
    }

    #[test]
    fn an_unlimited_quota_shows_use_without_a_denominator() {
        assert_eq!(
            quota(Some(GB * 25), None, AppLocale::Ru),
            format!("25,0{NBSP}ГБ · без лимита")
        );
        assert_eq!(
            quota(Some(GB * 25), Some(0), AppLocale::En),
            format!("25.0{NBSP}GB · unlimited")
        );
    }

    #[test]
    fn an_unmeasured_node_is_not_a_zero() {
        assert_eq!(latency(Some(37), true), "37 ms");
        assert_eq!(latency(Some(37), false), "37 ms");
    }

    #[test]
    fn silence_and_never_asked_read_differently() {
        // `n/a` is a claim that the server did not answer. Before any probe has
        // run, nothing has asked it anything, and saying so is a lie.
        assert_eq!(latency(None, false), "—");
        assert_eq!(latency(None, true), "n/a");
    }

    #[test]
    fn age_steps_through_its_three_units() {
        assert_eq!(age(4, AppLocale::Ru), "4 с");
        assert_eq!(age(120, AppLocale::Ru), "2 мин");
        assert_eq!(age(7200, AppLocale::Ru), "2 ч");
        assert_eq!(age(-1, AppLocale::Ru), "0 с");
    }

    #[test]
    fn russian_dates_take_the_genitive() {
        let timestamp = time::macros::datetime!(2026-09-01 0:00 UTC).unix_timestamp();
        assert_eq!(date(Some(timestamp), AppLocale::Ru), "1 сентября");
        assert_eq!(date(Some(timestamp), AppLocale::En), "1 September");
        assert_eq!(date(None, AppLocale::Ru), "—");
    }

    #[test]
    fn every_month_has_a_genitive_form() {
        // A missing month would surface as a compile error rather than a wrong
        // string, but the genitive endings are easy to typo — spot-check the
        // ones that do not simply take -я.
        let march = time::macros::datetime!(2026-03-08 0:00 UTC).unix_timestamp();
        let august = time::macros::datetime!(2026-08-16 0:00 UTC).unix_timestamp();
        assert_eq!(date(Some(march), AppLocale::Ru), "8 марта");
        assert_eq!(date(Some(august), AppLocale::Ru), "16 августа");
    }
}
