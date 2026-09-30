//! Links that open the app from a website or a chat.
//!
//! `moonlight://install-config?url=<subscription link>` adds a subscription —
//! the form Clash clients use, so a service's "add to app" button needs nothing
//! written for this app. `moonlight://import?url=…` and `moonlight:///import`
//! mean the same, as they do in Flowvy. The nested link should be
//! percent-encoded whole; one that was not, with `&` in it, is still read to
//! the end rather than cut at the first `&`.
//!
//! A link never adds anything by itself: any page can open one, and a
//! subscription added unseen would route the machine through whoever wrote the
//! page. The app asks first — see the prompt in the UI.

pub const SCHEME: &str = "moonlight";
const ACTIONS: [&str; 3] = ["install-config", "import", "add"];

/// The command Windows runs for a link: this executable, with the link as its
/// one argument, both quoted.
pub fn open_command(executable: &std::path::Path) -> String {
    format!("\"{}\" \"%1\"", executable.display())
}

/// Makes `moonlight:` links open this executable, for this user.
///
/// `HKCU\Software\Classes`, written at every launch like the sign-in entry,
/// so the links follow the app wherever it was installed or moved to and need
/// no elevation. Returns whether all of it was written.
#[cfg(windows)]
pub fn register() -> bool {
    use windows::core::HSTRING;
    use windows::Win32::Foundation::ERROR_SUCCESS;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_WRITE,
        REG_OPTION_NON_VOLATILE, REG_SZ,
    };

    let Ok(executable) = std::env::current_exe() else {
        return false;
    };
    let root = format!(r"Software\Classes\{SCHEME}");
    let entries = [
        (root.clone(), "", "URL:moonlight".to_string()),
        (root.clone(), "URL Protocol", String::new()),
        (
            format!(r"{root}\DefaultIcon"),
            "",
            format!("\"{}\",0", executable.display()),
        ),
        (
            format!(r"{root}\shell\open\command"),
            "",
            open_command(&executable),
        ),
    ];
    entries.iter().all(|(path, name, value)| unsafe {
        let mut key = HKEY::default();
        if RegCreateKeyExW(
            HKEY_CURRENT_USER,
            &HSTRING::from(path.as_str()),
            None,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut key,
            None,
        ) != ERROR_SUCCESS
        {
            return false;
        }
        let data: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
        let bytes = std::slice::from_raw_parts(data.as_ptr() as *const u8, data.len() * 2);
        let written = RegSetValueExW(key, &HSTRING::from(*name), None, REG_SZ, Some(bytes));
        let _ = RegCloseKey(key);
        written == ERROR_SUCCESS
    })
}

#[cfg(not(windows))]
pub fn register() -> bool {
    false
}

/// Whether an argument or a forwarded request is one of these links at all,
/// valid or not — so a bad one can be answered rather than ignored.
pub fn is_link(text: &str) -> bool {
    text.get(..SCHEME.len() + 1)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("moonlight:"))
}

/// The subscription link `link` asks to add, or `None` when it asks for
/// nothing this app does.
pub fn subscription_to_add(link: &str) -> Option<String> {
    let url = url::Url::parse(link.trim()).ok()?;
    if !url.scheme().eq_ignore_ascii_case(SCHEME) {
        return None;
    }
    // `moonlight://import?…` has the action as its host; `moonlight:///import?…`
    // as its path.
    let host = url.host_str().unwrap_or_default().to_lowercase();
    let action = if host.is_empty() {
        url.path()
            .split('/')
            .find(|s| !s.is_empty())
            .unwrap_or_default()
            .to_lowercase()
    } else {
        host
    };
    if !ACTIONS.contains(&action.as_str()) {
        return None;
    }
    let nested = nested_link(&url)?;
    is_web_address(&nested).then_some(nested)
}

/// The `url` parameter, decoded. Read from the raw query when it leads, so an
/// unencoded link keeps its own `&` and `#` parts.
fn nested_link(url: &url::Url) -> Option<String> {
    let decode = |raw: &str| {
        percent_encoding::percent_decode_str(raw)
            .decode_utf8_lossy()
            .trim()
            .to_string()
    };
    if let Some(raw) = url.query().and_then(|q| q.strip_prefix("url=")) {
        let mut value = decode(raw);
        if let Some(fragment) = url.fragment() {
            value = format!("{value}#{}", decode(fragment));
        }
        if !value.is_empty() {
            return Some(value);
        }
    }
    let values: Vec<String> = url
        .query_pairs()
        .filter(|(name, _)| name == "url")
        .map(|(_, value)| value.trim().to_string())
        .collect();
    match values.as_slice() {
        [only] if !only.is_empty() => Some(only.clone()),
        _ => None,
    }
}

/// Only a web address: a link from outside is not trusted with a file path,
/// or with any other scheme a subscription client might accept.
fn is_web_address(link: &str) -> bool {
    url::Url::parse(link).is_ok_and(|url| {
        matches!(url.scheme(), "http" | "https") && url.host_str().is_some_and(|h| !h.is_empty())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SUB: &str = "https://example.com/sub/token";

    #[test]
    fn every_form_adds_the_subscription_it_carries() {
        for link in [
            // The Clash form, encoded as it should be.
            "moonlight://install-config?url=https%3A%2F%2Fexample.com%2Fsub%2Ftoken",
            // Flowvy's `import`.
            "moonlight://import?url=https%3A%2F%2Fexample.com%2Fsub%2Ftoken",
            // The action as a path.
            "moonlight:///import?url=https%3A%2F%2Fexample.com%2Fsub%2Ftoken",
            // Scheme and action in any case.
            "MOONLIGHT://Install-Config?url=https%3A%2F%2Fexample.com%2Fsub%2Ftoken",
        ] {
            assert_eq!(subscription_to_add(link).as_deref(), Some(SUB), "{link}");
        }
    }

    #[test]
    fn an_unencoded_link_keeps_its_own_query_ampersand_and_all() {
        assert_eq!(
            subscription_to_add(
                "moonlight://install-config?url=https://example.com/sub/token?format=mihomo&device=win"
            )
            .as_deref(),
            Some("https://example.com/sub/token?format=mihomo&device=win")
        );
    }

    #[test]
    fn non_ascii_is_decoded() {
        assert_eq!(
            subscription_to_add(
                "moonlight://install-config?url=https%3A%2F%2Fexample.com%2Fsub%2F%D1%82%D0%BE%D0%BA%D0%B5%D0%BD"
            )
            .as_deref(),
            Some("https://example.com/sub/токен")
        );
    }

    #[test]
    fn anything_else_is_refused() {
        for link in [
            "moonlight://install-config",
            "moonlight://install-config?url=",
            // A file, not a web address.
            "moonlight://install-config?url=file%3A%2F%2F%2Fetc%2Fpasswd",
            // A single server, not a subscription.
            "moonlight://install-config?url=vless%3A%2F%2Fid%40host%3A443",
            // An action this app does not have.
            "moonlight://delete-everything?url=https%3A%2F%2Fexample.com",
            // Another app's scheme.
            "clash://install-config?url=https%3A%2F%2Fexample.com",
        ] {
            assert_eq!(subscription_to_add(link), None, "{link}");
        }
    }

    #[test]
    fn windows_hands_the_link_over_as_one_quoted_argument() {
        let command = open_command(std::path::Path::new(
            r"C:\Program Files\moonlight\moonlight.exe",
        ));
        assert_eq!(
            command,
            r#""C:\Program Files\moonlight\moonlight.exe" "%1""#
        );
    }

    #[test]
    fn a_link_is_recognised_before_it_is_understood() {
        assert!(is_link("moonlight://whatever"));
        assert!(is_link("MOONLIGHT:///import"));
        assert!(!is_link("show"));
        assert!(!is_link("https://example.com"));
    }
}
