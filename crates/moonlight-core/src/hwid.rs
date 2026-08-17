//! The device identifier the panel counts subscriptions against.
//!
//! Remnawave enforces a device limit by `x-hwid`, so this value decides whether
//! reinstalling the app costs the user a slot. It used to be a fresh random v4
//! UUID minted whenever `preferences.json` was absent — which is every reinstall,
//! every wiped profile and every fresh copy of the portable zip. Each of those
//! looked like a brand new device to the panel, and a five-device plan could be
//! exhausted without ever touching a second machine.
//!
//! So it is derived instead, from a value Windows keeps for the life of the
//! installation. Same machine, same identifier, no matter how many times the app
//! is removed and put back.
//!
//! It is **derived, not copied**. `MachineGuid` is a stable cross-application
//! identifier for the Windows install, and sending it verbatim to a panel would
//! hand over something that correlates this user across every other program that
//! reads it. A UUIDv5 under this app's own namespace is stable in exactly the
//! same way while being meaningless anywhere else.

use uuid::Uuid;

/// The namespace the machine identifier is hashed under. Any fixed UUID does;
/// this one is arbitrary and must never change, because changing it re-registers
/// every existing install as a new device.
const NAMESPACE: Uuid = Uuid::from_u128(0x8f31_c2a7_4b6e_4e2b_9e5d_2c1a_7f0b_6d34);

/// A stable identifier for this machine, or a random one where no stable source
/// is available.
///
/// The fallback is deliberately random rather than a constant: a shared constant
/// would make every machine that hit the fallback look like the *same* device to
/// the panel, which is a worse failure than looking like a new one.
pub fn stable() -> String {
    match machine_identifier() {
        Some(raw) => Uuid::new_v5(&NAMESPACE, raw.as_bytes()).to_string(),
        None => Uuid::new_v4().to_string(),
    }
}

#[cfg(windows)]
fn machine_identifier() -> Option<String> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::ERROR_SUCCESS;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_LOCAL_MACHINE,
        KEY_QUERY_VALUE, KEY_WOW64_64KEY, REG_VALUE_TYPE,
    };

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    unsafe {
        let mut key = HKEY::default();
        let path = wide(r"SOFTWARE\Microsoft\Cryptography");
        // KEY_WOW64_64KEY: without it a 32-bit build would be redirected to the
        // WOW6432Node view, where this value does not live.
        let opened = RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(path.as_ptr()),
            Some(0),
            KEY_QUERY_VALUE | KEY_WOW64_64KEY,
            &mut key,
        );
        if opened != ERROR_SUCCESS {
            return None;
        }

        let name = wide("MachineGuid");
        let mut kind = REG_VALUE_TYPE::default();
        let mut size = 0u32;
        let sized = RegQueryValueExW(
            key,
            PCWSTR(name.as_ptr()),
            None,
            Some(&mut kind),
            None,
            Some(&mut size),
        );
        if sized != ERROR_SUCCESS || size == 0 {
            let _ = RegCloseKey(key);
            return None;
        }

        let mut buffer = vec![0u8; size as usize];
        let read = RegQueryValueExW(
            key,
            PCWSTR(name.as_ptr()),
            None,
            Some(&mut kind),
            Some(buffer.as_mut_ptr()),
            Some(&mut size),
        );
        let _ = RegCloseKey(key);
        if read != ERROR_SUCCESS {
            return None;
        }

        let wide_chars: Vec<u16> = buffer
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .take_while(|c| *c != 0)
            .collect();
        let value = String::from_utf16(&wide_chars).ok()?;
        let value = value.trim().to_string();
        (!value.is_empty()).then_some(value)
    }
}

#[cfg(not(windows))]
fn machine_identifier() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_machine_derives_the_same_identifier() {
        // The whole point: a reinstall must not look like a new device.
        let first = Uuid::new_v5(&NAMESPACE, b"machine-guid");
        let second = Uuid::new_v5(&NAMESPACE, b"machine-guid");
        assert_eq!(first, second);
    }

    #[test]
    fn different_machines_derive_different_identifiers() {
        let a = Uuid::new_v5(&NAMESPACE, b"machine-a");
        let b = Uuid::new_v5(&NAMESPACE, b"machine-b");
        assert_ne!(a, b);
    }

    #[test]
    fn the_identifier_is_not_the_machine_guid_itself() {
        // Sending the raw value would correlate this user with every other
        // program on the machine that reads the same key.
        let raw = "c298bf22-61c6-481e-bac6-a2bdca3747e1";
        assert_ne!(Uuid::new_v5(&NAMESPACE, raw.as_bytes()).to_string(), raw);
    }

    #[test]
    #[cfg(windows)]
    fn a_real_windows_machine_derives_a_stable_identifier() {
        // Twice in a row, through the actual registry read.
        assert_eq!(stable(), stable());
        assert!(Uuid::parse_str(&stable()).is_ok());
    }
}
