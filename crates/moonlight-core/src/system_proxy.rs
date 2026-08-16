//! Points Windows's own proxy settings at the running core, and puts them back.
//!
//! This is the default way traffic reaches the tunnel, because it needs no
//! privileges: the values live under `HKEY_CURRENT_USER`, which is the same
//! place the Settings app's *Manual proxy setup* writes. What it buys is also
//! what it costs — only applications that honour WinINET are captured. A game
//! with its own socket stack, or anything using QUIC, goes straight out the
//! physical interface. That is the reason TUN mode exists, and the reason the
//! split-tunnel screen is inert here: without an interface to route, there is
//! nothing to route per-process.
//!
//! ## Why the registry write is not the whole job
//!
//! macOS's `networksetup` takes effect the moment it returns. Windows does not:
//! WinINET caches the proxy configuration per process, and an application that
//! is already running keeps using the old settings until it is told to re-read
//! them. `InternetSetOptionW(INTERNET_OPTION_SETTINGS_CHANGED)` followed by
//! `INTERNET_OPTION_REFRESH` is the broadcast that does the telling. Without it
//! the registry says "proxied" while every browser already open is still going
//! direct — which is worse than failing, because the UI reports a tunnel that
//! is only carrying traffic from applications started afterwards.
//!
//! ## Why there is a snapshot
//!
//! A machine may already have had a proxy the user set by hand, or one left
//! behind by another client. Restoring "off" unconditionally would silently
//! delete it. So the previous values are recorded before the first write and
//! put back verbatim on disconnect.

use serde::{Deserialize, Serialize};

/// Loopback and local names must never go through the proxy, or the app's own
/// calls to the core would loop back through the tunnel it is managing.
///
/// `<local>` is WinINET's own token for "any hostname without a dot", which is
/// how Windows spells the `*.local` exclusion.
pub const BYPASS: &str = "localhost;127.*;10.*;172.16.*;172.17.*;172.18.*;172.19.*;172.20.*;\
172.21.*;172.22.*;172.23.*;172.24.*;172.25.*;172.26.*;172.27.*;172.28.*;172.29.*;172.30.*;\
172.31.*;192.168.*;169.254.*;<local>";

/// What the settings were before this client touched them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub enabled: bool,
    pub server: String,
    pub bypass: String,
    /// A PAC URL, if one was configured. Kept because a machine configured by
    /// group policy to use a PAC script has no `ProxyServer` at all, and
    /// restoring only the manual fields would leave it with no proxy config.
    pub auto_config_url: Option<String>,
}

/// The value written to `ProxyServer` for a core listening on `port`.
///
/// mihomo's mixed listener speaks HTTP and SOCKS on the one port, but WinINET
/// only routes HTTP and HTTPS through a proxy — its `socks=` scheme is used by
/// almost nothing. So both protocols are pointed at the mixed port explicitly
/// rather than relying on the bare `host:port` form, which some applications
/// read as HTTP-only.
pub fn proxy_server_value(port: u16) -> String {
    format!("http=127.0.0.1:{port};https=127.0.0.1:{port}")
}

#[cfg(windows)]
mod imp {
    use super::{Snapshot, BYPASS};
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::ERROR_SUCCESS;
    use windows::Win32::Networking::WinInet::{
        InternetSetOptionW, INTERNET_OPTION_REFRESH, INTERNET_OPTION_SETTINGS_CHANGED,
    };
    use windows::Win32::System::Registry::{
        RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
        HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_DWORD, REG_SZ, REG_VALUE_TYPE,
    };

    const SETTINGS_PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\Internet Settings";

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
            let path = wide(SETTINGS_PATH);
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

        fn string(&self, name: &str) -> Option<String> {
            let name = wide(name);
            let mut kind = REG_VALUE_TYPE::default();
            let mut size = 0u32;
            let status = unsafe {
                RegQueryValueExW(
                    self.0,
                    PCWSTR(name.as_ptr()),
                    None,
                    Some(&mut kind),
                    None,
                    Some(&mut size),
                )
            };
            if status != ERROR_SUCCESS || size == 0 {
                return None;
            }

            let mut buffer = vec![0u8; size as usize];
            let status = unsafe {
                RegQueryValueExW(
                    self.0,
                    PCWSTR(name.as_ptr()),
                    None,
                    Some(&mut kind),
                    Some(buffer.as_mut_ptr()),
                    Some(&mut size),
                )
            };
            if status != ERROR_SUCCESS {
                return None;
            }

            let units: Vec<u16> = buffer
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .take_while(|unit| *unit != 0)
                .collect();
            Some(String::from_utf16_lossy(&units))
        }

        fn dword(&self, name: &str) -> Option<u32> {
            let name = wide(name);
            let mut kind = REG_VALUE_TYPE::default();
            let mut value = 0u32;
            let mut size = std::mem::size_of::<u32>() as u32;
            let status = unsafe {
                RegQueryValueExW(
                    self.0,
                    PCWSTR(name.as_ptr()),
                    None,
                    Some(&mut kind),
                    Some(&mut value as *mut u32 as *mut u8),
                    Some(&mut size),
                )
            };
            (status == ERROR_SUCCESS).then_some(value)
        }

        fn set_string(&self, name: &str, value: &str) -> bool {
            let name = wide(name);
            let data = wide(value);
            let bytes =
                unsafe { std::slice::from_raw_parts(data.as_ptr() as *const u8, data.len() * 2) };
            let status =
                unsafe { RegSetValueExW(self.0, PCWSTR(name.as_ptr()), None, REG_SZ, Some(bytes)) };
            status == ERROR_SUCCESS
        }

        fn set_dword(&self, name: &str, value: u32) -> bool {
            let name = wide(name);
            let status = unsafe {
                RegSetValueExW(
                    self.0,
                    PCWSTR(name.as_ptr()),
                    None,
                    REG_DWORD,
                    Some(&value.to_le_bytes()),
                )
            };
            status == ERROR_SUCCESS
        }

        fn delete(&self, name: &str) {
            let name = wide(name);
            unsafe {
                let _ = RegDeleteValueW(self.0, PCWSTR(name.as_ptr()));
            }
        }
    }

    impl Drop for Key {
        fn drop(&mut self) {
            unsafe {
                let _ = RegCloseKey(self.0);
            }
        }
    }

    pub fn snapshot() -> Snapshot {
        let Some(key) = Key::open(false) else {
            return Snapshot::default();
        };
        Snapshot {
            enabled: key.dword("ProxyEnable").unwrap_or(0) != 0,
            server: key.string("ProxyServer").unwrap_or_default(),
            bypass: key.string("ProxyOverride").unwrap_or_default(),
            auto_config_url: key.string("AutoConfigURL"),
        }
    }

    pub fn enable(port: u16) -> bool {
        let Some(key) = Key::open(true) else {
            return false;
        };
        let written = key.set_string("ProxyServer", &super::proxy_server_value(port))
            && key.set_string("ProxyOverride", BYPASS)
            && key.set_dword("ProxyEnable", 1);
        // A PAC script takes precedence over the manual settings, so a machine
        // that has one would ignore everything just written. It is restored
        // verbatim by `restore`.
        key.delete("AutoConfigURL");
        drop(key);

        notify();
        written
    }

    pub fn restore(snapshot: &Snapshot) -> bool {
        let Some(key) = Key::open(true) else {
            return false;
        };
        let mut ok = key.set_dword("ProxyEnable", u32::from(snapshot.enabled));
        if snapshot.server.is_empty() {
            key.delete("ProxyServer");
        } else {
            ok &= key.set_string("ProxyServer", &snapshot.server);
        }
        if snapshot.bypass.is_empty() {
            key.delete("ProxyOverride");
        } else {
            ok &= key.set_string("ProxyOverride", &snapshot.bypass);
        }
        match &snapshot.auto_config_url {
            Some(url) => {
                ok &= key.set_string("AutoConfigURL", url);
            }
            None => key.delete("AutoConfigURL"),
        }
        drop(key);

        notify();
        ok
    }

    /// Tells every running WinINET process to re-read the settings.
    ///
    /// Without this the registry says "proxied" while every browser already
    /// open keeps going direct, which reads as a tunnel that only captures
    /// applications started after connecting.
    fn notify() {
        unsafe {
            let _ = InternetSetOptionW(None, INTERNET_OPTION_SETTINGS_CHANGED, None, 0);
            let _ = InternetSetOptionW(None, INTERNET_OPTION_REFRESH, None, 0);
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::Snapshot;

    // The UI and the whole platform-neutral core build and run on macOS and
    // Linux so the port can be developed and tested away from Windows. Only
    // this layer is stubbed: taking over the machine's proxy settings is the one
    // thing with no meaningful non-Windows behaviour, and silently succeeding
    // would let a test claim a tunnel it never established.

    pub fn snapshot() -> Snapshot {
        Snapshot::default()
    }

    pub fn enable(_port: u16) -> bool {
        false
    }

    pub fn restore(_snapshot: &Snapshot) -> bool {
        false
    }
}

/// Records the settings as they are now, so they can be put back exactly.
pub fn snapshot() -> Snapshot {
    imp::snapshot()
}

/// Points HTTP and HTTPS at the core's mixed port and refreshes WinINET.
pub fn enable(port: u16) -> bool {
    imp::enable(port)
}

/// Puts back what [`snapshot`] recorded.
pub fn restore(snapshot: &Snapshot) -> bool {
    imp::restore(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_protocols_are_named_explicitly() {
        // The bare `host:port` form is read as HTTP-only by some applications,
        // which leaves HTTPS going direct while the UI says connected.
        let value = proxy_server_value(7897);
        assert_eq!(value, "http=127.0.0.1:7897;https=127.0.0.1:7897");
        assert!(value.contains("http="));
        assert!(value.contains("https="));
    }

    #[test]
    fn the_bypass_list_keeps_the_apps_own_calls_off_the_proxy() {
        // The controller is on 127.0.0.1; routing it through the core would
        // loop the app's own API calls back through the tunnel.
        assert!(BYPASS.contains("127.*"));
        assert!(BYPASS.contains("localhost"));
    }

    #[test]
    fn the_bypass_list_covers_the_private_ranges_and_link_local() {
        for prefix in ["10.*", "192.168.*", "172.16.*", "172.31.*", "169.254.*"] {
            assert!(BYPASS.contains(prefix), "{prefix} is not bypassed");
        }
    }

    #[test]
    fn the_bypass_list_uses_wininets_own_local_token() {
        // `<local>` is how Windows spells "any hostname with no dot"; `*.local`
        // is the macOS spelling and matches nothing here.
        assert!(BYPASS.contains("<local>"));
        assert!(!BYPASS.contains("*.local"));
    }

    #[test]
    fn the_bypass_list_is_semicolon_separated_with_no_blank_entries() {
        // WinINET splits on ';' and a stray empty entry disables the whole list
        // on some Windows builds.
        for entry in BYPASS.split(';') {
            assert!(!entry.trim().is_empty(), "blank entry in the bypass list");
        }
    }

    #[test]
    fn a_default_snapshot_reads_as_no_proxy() {
        let snapshot = Snapshot::default();
        assert!(!snapshot.enabled);
        assert!(snapshot.server.is_empty());
        assert_eq!(snapshot.auto_config_url, None);
    }

    #[test]
    fn a_snapshot_round_trips_through_preferences() {
        // It is persisted, because the app has to be able to put the settings
        // back after a crash that skipped the disconnect path.
        let snapshot = Snapshot {
            enabled: true,
            server: "proxy.corp:8080".into(),
            bypass: "<local>".into(),
            auto_config_url: Some("http://wpad/wpad.dat".into()),
        };
        let json = serde_json::to_string(&snapshot).expect("serialises");
        let back: Snapshot = serde_json::from_str(&json).expect("deserialises");
        assert_eq!(snapshot, back);
    }

    #[cfg(not(windows))]
    #[test]
    fn the_non_windows_stub_reports_failure_rather_than_pretending() {
        // A stub that returned true would let a test claim a tunnel it never
        // established.
        assert!(!enable(7897));
        assert!(!restore(&Snapshot::default()));
    }
}
