//! ISO 3166-1 alpha-2 → country name, in both of the app's languages.
//!
//! The macOS client gets this free from `Locale.localizedString(forRegionCode:)`.
//! Windows has an equivalent in `GetGeoInfoEx`, but it needs a GEOID rather than
//! an ISO code, it answers in the *system* locale rather than the app's, and it
//! would put a Win32 call on the path of every node row's second line. A table
//! is smaller than the code to avoid one.
//!
//! Only the countries VPN panels actually place nodes in are listed. An unknown
//! code returns `None`, and the row then shows just the transport — which is
//! the same thing it does for a node with no flag at all, so an unlisted
//! country degrades to an already-designed state rather than to a blank.

use crate::models::AppLocale;

/// `(alpha-2, Russian, English)`, sorted by code so the lookup can bisect.
const COUNTRIES: &[(&str, &str, &str)] = &[
    ("AE", "ОАЭ", "United Arab Emirates"),
    ("AL", "Албания", "Albania"),
    ("AM", "Армения", "Armenia"),
    ("AR", "Аргентина", "Argentina"),
    ("AT", "Австрия", "Austria"),
    ("AU", "Австралия", "Australia"),
    ("AZ", "Азербайджан", "Azerbaijan"),
    ("BE", "Бельгия", "Belgium"),
    ("BG", "Болгария", "Bulgaria"),
    ("BR", "Бразилия", "Brazil"),
    ("BY", "Беларусь", "Belarus"),
    ("CA", "Канада", "Canada"),
    ("CH", "Швейцария", "Switzerland"),
    ("CL", "Чили", "Chile"),
    ("CN", "Китай", "China"),
    ("CO", "Колумбия", "Colombia"),
    ("CY", "Кипр", "Cyprus"),
    ("CZ", "Чехия", "Czechia"),
    ("DE", "Германия", "Germany"),
    ("DK", "Дания", "Denmark"),
    ("EE", "Эстония", "Estonia"),
    ("EG", "Египет", "Egypt"),
    ("ES", "Испания", "Spain"),
    ("FI", "Финляндия", "Finland"),
    ("FR", "Франция", "France"),
    ("GB", "Великобритания", "United Kingdom"),
    ("GE", "Грузия", "Georgia"),
    ("GR", "Греция", "Greece"),
    ("HK", "Гонконг", "Hong Kong"),
    ("HR", "Хорватия", "Croatia"),
    ("HU", "Венгрия", "Hungary"),
    ("ID", "Индонезия", "Indonesia"),
    ("IE", "Ирландия", "Ireland"),
    ("IL", "Израиль", "Israel"),
    ("IN", "Индия", "India"),
    ("IR", "Иран", "Iran"),
    ("IS", "Исландия", "Iceland"),
    ("IT", "Италия", "Italy"),
    ("JP", "Япония", "Japan"),
    ("KG", "Киргизия", "Kyrgyzstan"),
    ("KR", "Южная Корея", "South Korea"),
    ("KZ", "Казахстан", "Kazakhstan"),
    ("LT", "Литва", "Lithuania"),
    ("LU", "Люксембург", "Luxembourg"),
    ("LV", "Латвия", "Latvia"),
    ("MD", "Молдова", "Moldova"),
    ("MX", "Мексика", "Mexico"),
    ("MY", "Малайзия", "Malaysia"),
    ("NG", "Нигерия", "Nigeria"),
    ("NL", "Нидерланды", "Netherlands"),
    ("NO", "Норвегия", "Norway"),
    ("NZ", "Новая Зеландия", "New Zealand"),
    ("PE", "Перу", "Peru"),
    ("PH", "Филиппины", "Philippines"),
    ("PL", "Польша", "Poland"),
    ("PT", "Португалия", "Portugal"),
    ("QA", "Катар", "Qatar"),
    ("RO", "Румыния", "Romania"),
    ("RS", "Сербия", "Serbia"),
    ("RU", "Россия", "Russia"),
    ("SA", "Саудовская Аравия", "Saudi Arabia"),
    ("SE", "Швеция", "Sweden"),
    ("SG", "Сингапур", "Singapore"),
    ("SI", "Словения", "Slovenia"),
    ("SK", "Словакия", "Slovakia"),
    ("TH", "Таиланд", "Thailand"),
    ("TR", "Турция", "Türkiye"),
    ("TW", "Тайвань", "Taiwan"),
    ("UA", "Украина", "Ukraine"),
    ("US", "США", "United States"),
    ("UZ", "Узбекистан", "Uzbekistan"),
    ("VN", "Вьетнам", "Vietnam"),
    ("ZA", "ЮАР", "South Africa"),
];

pub fn name(code: &str, locale: AppLocale) -> Option<&'static str> {
    let index = COUNTRIES
        .binary_search_by(|(c, _, _)| (*c).cmp(code))
        .ok()?;
    let (_, ru, en) = COUNTRIES[index];
    Some(match locale {
        AppLocale::Ru => ru,
        AppLocale::En => en,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_sorted_so_the_bisect_is_valid() {
        // binary_search silently returns the wrong answer on unsorted data, so
        // this is load-bearing rather than tidiness.
        for pair in COUNTRIES.windows(2) {
            assert!(
                pair[0].0 < pair[1].0,
                "{} is not before {}",
                pair[0].0,
                pair[1].0
            );
        }
    }

    #[test]
    fn lookups_answer_in_the_asked_language() {
        assert_eq!(name("SE", AppLocale::Ru), Some("Швеция"));
        assert_eq!(name("SE", AppLocale::En), Some("Sweden"));
        assert_eq!(name("US", AppLocale::Ru), Some("США"));
    }

    #[test]
    fn an_unlisted_code_is_unknown_rather_than_guessed() {
        assert_eq!(name("ZZ", AppLocale::Ru), None);
        assert_eq!(name("", AppLocale::Ru), None);
    }

    #[test]
    fn every_entry_is_a_two_letter_upper_case_code() {
        for (code, ru, en) in COUNTRIES {
            assert_eq!(code.len(), 2, "{code} is not alpha-2");
            assert!(code.chars().all(|c| c.is_ascii_uppercase()), "{code}");
            assert!(!ru.is_empty() && !en.is_empty(), "{code} has a blank name");
        }
    }
}
