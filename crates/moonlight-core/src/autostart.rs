//! Starting the client at sign-in.
//!
//! A value under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, which is
//! the per-user Run key: no elevation, and it survives an in-place update
//! because the value is rewritten from the running executable's own path each
//! time it is enabled.
//!
//! Deliberately *not* a scheduled task or a service. Both can start the client
//! before the desktop session exists, and this app's whole job — writing the
//! user's proxy settings and drawing a window — needs one.
//!
//! The preference and the registry are two separate facts, and they can
//! disagree: a user can delete the value with `msconfig` while the app is shut.
//! [`is_enabled`] reads the registry rather than trusting the stored flag, so
//! the switch shows what Windows will actually do.

/// The value name under the Run key. Matches the product name rather than the
/// executable, because this is what the Startup tab in Task Manager shows.
pub const VALUE_NAME: &str = "Moonlight";

#[cfg(windows)]
pub use imp::{is_enabled, set_enabled};

/// The command the Run key stores: the executable's own path, quoted so a path
/// containing spaces survives the shell that expands it.
///
/// Split out from the registry write so it can be tested off Windows — the
/// quoting is the part that goes wrong, and `C:\Program Files\...` is the
/// default install location.
pub fn run_command(executable: &std::path::Path) -> String {
    format!("\"{}\"", executable.display())
}

#[cfg(windows)]
mod imp {
    use super::{run_command, VALUE_NAME};
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::ERROR_SUCCESS;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
        HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_SZ, REG_VALUE_TYPE,
    };

    const RUN_PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    struct Key(HKEY);

    impl Key {
        fn open(write: bool) -> Option<Key> {
            let access = if write {
                KEY_READ | KEY_WRITE
            } else {
                KEY_READ
            };
            let mut handle = HKEY::default();
            let path = wide(RUN_PATH);
            let status = unsafe {
                RegOpenKeyExW(
                    HKEY_CURRENT_USER,
                    PCWSTR(path.as_ptr()),
                    Some(0),
                    access,
                    &mut handle,
                )
            };
            (status == ERROR_SUCCESS).then_some(Key(handle))
        }
    }

    impl Drop for Key {
        fn drop(&mut self) {
            unsafe { let _ = RegCloseKey(self.0); }
        }
    }

    /// Whether Windows will start the client at sign-in.
    pub fn is_enabled() -> bool {
        let Some(key) = Key::open(false) else {
            return false;
        };
        let name = wide(VALUE_NAME);
        let mut kind = REG_VALUE_TYPE::default();
        let mut size = 0u32;
        let status = unsafe {
            RegQueryValueExW(
                key.0,
                PCWSTR(name.as_ptr()),
                None,
                Some(&mut kind),
                None,
                Some(&mut size),
            )
        };
        status == ERROR_SUCCESS && size > 0
    }

    /// Adds or removes the Run value. Returns whether the registry now agrees
    /// with `enabled`, so a caller can avoid persisting a preference the machine
    /// refused.
    pub fn set_enabled(enabled: bool) -> bool {
        let Some(key) = Key::open(true) else {
            return false;
        };
        let name = wide(VALUE_NAME);

        if !enabled {
            let status = unsafe { RegDeleteValueW(key.0, PCWSTR(name.as_ptr())) };
            // Already absent is the requested state, not a failure.
            return status == ERROR_SUCCESS || !is_enabled();
        }

        let Ok(executable) = std::env::current_exe() else {
            return false;
        };
        let command = wide(&run_command(&executable));
        let bytes = unsafe {
            std::slice::from_raw_parts(command.as_ptr() as *const u8, command.len() * 2)
        };
        let status =
            unsafe { RegSetValueExW(key.0, PCWSTR(name.as_ptr()), None, REG_SZ, Some(bytes)) };
        status == ERROR_SUCCESS
    }
}

#[cfg(not(windows))]
pub fn is_enabled() -> bool {
    false
}

#[cfg(not(windows))]
pub fn set_enabled(_enabled: bool) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn the_run_command_quotes_the_path() {
        // `C:\Program Files\Moonlight\moonlight.exe` is the default install
        // location, and an unquoted Run value there starts `C:\Program.exe`.
        let path = PathBuf::from(r"C:\Program Files\Moonlight\moonlight.exe");
        let command = run_command(&path);
        assert!(command.starts_with('"'));
        assert!(command.ends_with('"'));
        assert!(command.contains("Program Files"));
    }

    #[test]
    fn the_value_name_is_the_product_not_the_executable() {
        // It is what Task Manager's Startup tab lists.
        assert_eq!(VALUE_NAME, "Moonlight");
        assert!(!VALUE_NAME.ends_with(".exe"));
    }
}
