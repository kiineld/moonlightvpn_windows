//! The geo databases the panel's rules are written against.
//!
//! Every Remnawave config ships `GEOSITE,…` and `GEOIP,…` rules, and mihomo
//! refuses to parse a config whose rules it cannot resolve — the failure is
//! `level=fatal` and the process exits before it binds its API, which the app
//! then reports as "the core did not answer".
//!
//! mihomo can fetch these itself, and that is what it tried to do. It cannot,
//! reliably: the download happens *during* config parsing, before any tunnel
//! exists, using mihomo's own resolver — which at that point is a `fake-ip`
//! resolver pointed at nameservers it has not finished setting up. On this
//! machine that produced `dns resolve failed: ip version error` and a dead core
//! every single connect.
//!
//! So the app fetches them instead, over the OS resolver and the same HTTP stack
//! that already talks to the panel, before the core is ever started. They are
//! cached in the core's data directory, so this costs one download per install
//! rather than one per connect.

use std::path::{Path, PathBuf};
use std::time::Duration;

/// Where the files come from. These are the same URLs mihomo itself uses, so a
/// cache populated here is byte-identical to one it would have built.
///
/// GEOIP is the **MMDB**, not `geoip.dat`. mihomo only reads the `.dat` form
/// when a config sets `geodata-mode: true`; left at its default it wants
/// `geoip.metadb`, and seeding the wrong one gets you a core that loads every
/// GeoSite rule, then dies on the first `GEOIP,` rule instead — which looks like
/// the fix having done nothing.
const GEOSITE_URL: &str =
    "https://github.com/MetaCubeX/meta-rules-dat/releases/download/latest/geosite.dat";
const GEOIP_URL: &str =
    "https://github.com/MetaCubeX/meta-rules-dat/releases/download/latest/geoip.metadb";

/// The names mihomo looks for in its working directory.
const GEOSITE_FILE: &str = "GeoSite.dat";
const GEOIP_FILE: &str = "geoip.metadb";

/// A database is only trusted if it is at least this big. A truncated or
/// error-page download would otherwise be cached forever and fail every connect
/// with a parse error instead of a download one.
const MINIMUM: u64 = 100 * 1024;

/// Whether both databases are already cached.
pub fn present(directory: &Path) -> bool {
    [GEOSITE_FILE, GEOIP_FILE]
        .iter()
        .all(|name| is_usable(&directory.join(name)))
}

fn is_usable(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.len() >= MINIMUM)
}

/// Downloads whichever databases are missing.
///
/// Returns `Ok(false)` when nothing needed doing, so the caller can narrate the
/// wait only on the connect that actually pays for it.
pub async fn ensure(directory: &Path) -> Result<bool, String> {
    if present(directory) {
        return Ok(false);
    }
    std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;

    // The build ships a copy beside the executable. Copying it costs nothing and
    // spares the first connect a 13 MB download on a link that may well be the
    // reason the user wants a VPN.
    seed_from_install(directory);
    if present(directory) {
        return Ok(false);
    }

    // Generous: these are 4 MB and 17 MB, and the connect they are blocking has
    // no chance of working without them.
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|e| e.to_string())?;

    let mut downloaded = false;
    for (url, name) in [(GEOSITE_URL, GEOSITE_FILE), (GEOIP_URL, GEOIP_FILE)] {
        let target = directory.join(name);
        if is_usable(&target) {
            continue;
        }
        fetch(&http, url, &target).await?;
        downloaded = true;
    }
    Ok(downloaded)
}

/// Copies the databases shipped with the build into the core's data directory.
///
/// Best effort: a portable copy assembled by hand may not have them, and the
/// download is still there for that.
fn seed_from_install(directory: &Path) {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let Some(shipped) = exe.parent().map(|d| d.join("geodata")) else {
        return;
    };
    for name in [GEOSITE_FILE, GEOIP_FILE] {
        let from = shipped.join(name);
        let to = directory.join(name);
        if from.is_file() && !is_usable(&to) {
            let _ = std::fs::copy(&from, &to);
        }
    }
}

/// Fetches one file, writing through a temporary so an interrupted download
/// cannot leave a half-file that looks cached.
async fn fetch(http: &reqwest::Client, url: &str, target: &Path) -> Result<(), String> {
    let response = http
        .get(url)
        .send()
        .await
        .map_err(|e| format!("{}: {e}", display_name(target)))?;
    if !response.status().is_success() {
        return Err(format!(
            "{}: the server answered {}",
            display_name(target),
            response.status()
        ));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("{}: {e}", display_name(target)))?;

    if (bytes.len() as u64) < MINIMUM {
        return Err(format!(
            "{}: only {} bytes arrived",
            display_name(target),
            bytes.len()
        ));
    }

    let temporary = temporary_beside(target);
    std::fs::write(&temporary, &bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&temporary, target).map_err(|e| e.to_string())?;
    Ok(())
}

fn temporary_beside(target: &Path) -> PathBuf {
    let mut name = target.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    target.with_file_name(name)
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "geodata".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_directory_with_neither_file_is_not_present() {
        let dir = std::env::temp_dir().join(format!("ml-geo-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(!present(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_truncated_database_does_not_count_as_cached() {
        // The failure this guards against is a cached error page: small, present,
        // and fatal to every connect until somebody deletes it by hand.
        let dir = std::env::temp_dir().join(format!("ml-geo-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(GEOSITE_FILE), b"404: Not Found").unwrap();
        std::fs::write(dir.join(GEOIP_FILE), b"404: Not Found").unwrap();
        assert!(!present(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_full_sized_pair_counts_as_cached() {
        let dir = std::env::temp_dir().join(format!("ml-geo-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let block = vec![0u8; (MINIMUM + 1) as usize];
        std::fs::write(dir.join(GEOSITE_FILE), &block).unwrap();
        std::fs::write(dir.join(GEOIP_FILE), &block).unwrap();
        assert!(present(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_temporary_sits_beside_its_target() {
        // Same directory, so the rename is on one volume and therefore atomic.
        let target = Path::new(r"C:\data\GeoSite.dat");
        let temporary = temporary_beside(target);
        assert_eq!(temporary.parent(), target.parent());
        assert_ne!(temporary, target);
    }
}
