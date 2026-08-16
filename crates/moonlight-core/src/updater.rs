//! Checks GitHub for a newer release, and swaps the installation for it.
//!
//! There is no App Store and no MSIX, so *Проверить обновления* does what a user
//! would otherwise do by hand: ask GitHub for the latest release, download the
//! zip, and replace the folder.
//!
//! ## Why the swap runs in a detached script
//!
//! A running `.exe` cannot be replaced on Windows — the loader holds the file
//! and `MoveFile` fails with `ERROR_SHARING_VIOLATION`. This is stricter than
//! macOS, where a bundle can be replaced under a running app and only *then*
//! behaves unpredictably. So the update is written as a `.cmd`, started
//! detached, and the app exits: the script waits for the process to disappear,
//! moves the old folder aside, unpacks the new one, **puts the old one back if
//! the unpack fails** rather than leaving no application at all, and relaunches.
//!
//! ## Why versions are compared numerically
//!
//! `"1.0.10" < "1.0.9"` as strings, so a string comparison stops offering
//! updates at the tenth patch and never says why.

use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Failure {
    #[error("Could not reach GitHub: {0}")]
    Transport(String),
    #[error("GitHub returned HTTP {0}")]
    Http(u16),
    #[error("The latest release has no Windows download")]
    NoAsset,
    #[error("Could not write the update script: {0}")]
    Script(String),
}

/// What the check found.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    UpToDate { current: String },
    Available(Release),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Release {
    pub version: String,
    pub notes: String,
    pub download_url: String,
    pub size: u64,
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<GithubAsset>,
}

#[derive(Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
}

/// Splits a version into its numeric components, ignoring a leading `v` and
/// anything after the numbers (`1.2.3-beta.1` → `[1, 2, 3]`).
///
/// A component that is not a number ends the parse rather than counting as
/// zero: `1.2.x` must not compare equal to `1.2.0`.
pub fn version_parts(version: &str) -> Vec<u64> {
    version
        .trim()
        .trim_start_matches(['v', 'V'])
        .split(['.', '-', '+'])
        .map(str::trim)
        .take_while(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
        .filter_map(|part| part.parse().ok())
        .collect()
}

/// Whether `candidate` is a later version than `current`.
///
/// Numeric, component by component, with a missing component read as zero so
/// `1.1` and `1.1.0` are the same version rather than different ones.
pub fn is_newer(candidate: &str, current: &str) -> bool {
    let a = version_parts(candidate);
    let b = version_parts(current);
    // A version with no numbers at all is not an upgrade over anything.
    if a.is_empty() {
        return false;
    }
    for i in 0..a.len().max(b.len()) {
        let left = a.get(i).copied().unwrap_or(0);
        let right = b.get(i).copied().unwrap_or(0);
        if left != right {
            return left > right;
        }
    }
    false
}

/// Picks the asset to download from a release's attachments.
///
/// The zip, not the bare `.exe`: an update has to replace the core and
/// `wintun.dll` alongside the app, and a release that only swapped
/// `moonlight.exe` would leave a new client driving whatever core the previous
/// install happened to have.
pub fn pick_asset<'a>(names: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    let mut candidates: Vec<&str> = names
        .filter(|n| {
            let lower = n.to_lowercase();
            lower.ends_with(".zip") && (lower.contains("x86_64") || lower.contains("x64"))
        })
        .collect();
    candidates.sort_unstable();
    candidates.first().copied()
}

/// Reads the GitHub releases JSON and decides whether it is worth offering.
///
/// Split from the HTTP call so the decision is testable without a network.
pub fn evaluate(body: &str, current_version: &str) -> Result<Outcome, Failure> {
    let releases: Vec<GithubRelease> = match serde_json::from_str::<Vec<GithubRelease>>(body) {
        Ok(list) => list,
        // `/releases/latest` returns one object rather than a list.
        Err(_) => match serde_json::from_str::<GithubRelease>(body) {
            Ok(one) => vec![one],
            Err(_) => return Err(Failure::NoAsset),
        },
    };

    let newest = releases
        .into_iter()
        // A draft is not published and a pre-release was not offered to
        // everyone; neither should push an update to a user who did not opt in.
        .filter(|r| !r.draft && !r.prerelease)
        .filter(|r| is_newer(&r.tag_name, current_version))
        .max_by(|a, b| version_parts(&a.tag_name).cmp(&version_parts(&b.tag_name)));

    let Some(release) = newest else {
        return Ok(Outcome::UpToDate {
            current: current_version.to_string(),
        });
    };

    let asset_name = pick_asset(release.assets.iter().map(|a| a.name.as_str()))
        .ok_or(Failure::NoAsset)?
        .to_string();
    let asset = release
        .assets
        .iter()
        .find(|a| a.name == asset_name)
        .ok_or(Failure::NoAsset)?;

    Ok(Outcome::Available(Release {
        version: release.tag_name.trim_start_matches(['v', 'V']).to_string(),
        notes: release.body.clone(),
        download_url: asset.browser_download_url.clone(),
        size: asset.size,
    }))
}

/// Asks GitHub what the latest release is.
pub async fn check(releases_api: &str, current_version: &str) -> Result<Outcome, Failure> {
    let client = reqwest::Client::builder()
        // The same reason the subscription client does it: while connected in
        // system-proxy mode this app has pointed the machine at its own core,
        // and an update check should not depend on the tunnel it is about to
        // replace the client for.
        .no_proxy()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| Failure::Transport(e.to_string()))?;

    let response = client
        .get(releases_api)
        .header("User-Agent", format!("moonlight/{current_version}"))
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| Failure::Transport(e.to_string()))?;

    let status = response.status();
    if !status.is_success() {
        return Err(Failure::Http(status.as_u16()));
    }
    let body = response
        .text()
        .await
        .map_err(|e| Failure::Transport(e.to_string()))?;
    evaluate(&body, current_version)
}

/// Downloads a release zip to `destination`.
pub async fn download(url: &str, destination: &std::path::Path) -> Result<(), Failure> {
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|e| Failure::Transport(e.to_string()))?;

    let response = client
        .get(url)
        .header("User-Agent", "moonlight")
        .send()
        .await
        .map_err(|e| Failure::Transport(e.to_string()))?;
    if !response.status().is_success() {
        return Err(Failure::Http(response.status().as_u16()));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|e| Failure::Transport(e.to_string()))?;
    std::fs::write(destination, &bytes).map_err(|e| Failure::Script(e.to_string()))
}

/// The batch script that performs the swap after this process exits.
///
/// Written out rather than built with a command builder so it can be read in
/// full here — it runs unattended, with the application already gone, and a
/// mistake in it leaves the user with no application at all.
///
/// The ordering is the whole point:
///
/// 1. Wait for the PID to disappear. Copying over a loaded `.exe` fails, and
///    failing *after* the old folder has been moved aside is the case that
///    loses the installation.
/// 2. Move the current folder aside rather than deleting it. If the unpack
///    fails there is still something to put back.
/// 3. Unpack, and on any failure restore the moved folder and relaunch the old
///    version — a working old client beats a half-written new one.
/// 4. Relaunch, then delete the backup and the script's own temporary files.
pub fn swap_script(pid: u32, zip: &str, install_dir: &str, executable: &str) -> String {
    let backup = format!("{install_dir}.old");
    format!(
        r#"@echo off
setlocal
set "PID={pid}"
set "ZIP={zip}"
set "DIR={install_dir}"
set "BACKUP={backup}"
set "EXE={executable}"

rem 1. Wait for the app to exit. A loaded .exe cannot be replaced, and the
rem    timeout is generous because a hung window is better than a lost install.
set /a TRIES=0
:wait
tasklist /FI "PID eq %PID%" 2>nul | find "%PID%" >nul
if errorlevel 1 goto gone
set /a TRIES+=1
if %TRIES% GEQ 60 goto giveup
timeout /t 1 /nobreak >nul
goto wait

:giveup
rem The app never exited. Do nothing at all rather than replace files under it.
del /q "%ZIP%" 2>nul
exit /b 1

:gone
rem 2. Move rather than delete, so there is something to restore.
if exist "%BACKUP%" rd /s /q "%BACKUP%" 2>nul
move "%DIR%" "%BACKUP%" >nul 2>&1
if errorlevel 1 goto restore

mkdir "%DIR%" 2>nul
rem 3. Unpack. Expand-Archive is in every supported PowerShell.
powershell -NoProfile -NonInteractive -Command ^
  "try {{ Expand-Archive -LiteralPath '%ZIP%' -DestinationPath '%DIR%' -Force; exit 0 }} catch {{ exit 1 }}"
if errorlevel 1 goto restore
if not exist "%DIR%\%EXE%" goto restore

rem 4. New version is in place. Relaunch, then clean up.
start "" "%DIR%\%EXE%"
rd /s /q "%BACKUP%" 2>nul
del /q "%ZIP%" 2>nul
(goto) 2>nul & del "%~f0"
exit /b 0

:restore
rem Put the old installation back and start it. A working old client beats a
rem half-written new one.
if exist "%DIR%" rd /s /q "%DIR%" 2>nul
if exist "%BACKUP%" move "%BACKUP%" "%DIR%" >nul 2>&1
if exist "%DIR%\%EXE%" start "" "%DIR%\%EXE%"
del /q "%ZIP%" 2>nul
(goto) 2>nul & del "%~f0"
exit /b 1
"#
    )
}

/// Writes the script and starts it detached, so it outlives this process.
#[cfg(windows)]
pub fn launch_swap(zip: &std::path::Path) -> Result<(), Failure> {
    use std::os::windows::process::CommandExt;

    let executable = std::env::current_exe().map_err(|e| Failure::Script(e.to_string()))?;
    let install_dir = executable
        .parent()
        .ok_or_else(|| Failure::Script("the executable has no parent directory".into()))?;
    let name = executable
        .file_name()
        .ok_or_else(|| Failure::Script("the executable has no file name".into()))?;

    let script_path = std::env::temp_dir().join("moonlight-update.cmd");
    let script = swap_script(
        std::process::id(),
        &zip.to_string_lossy(),
        &install_dir.to_string_lossy(),
        &name.to_string_lossy(),
    );
    std::fs::write(&script_path, script).map_err(|e| Failure::Script(e.to_string()))?;

    // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP, so the script is not killed
    // when this process exits — which it is about to.
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    std::process::Command::new("cmd")
        .arg("/c")
        .arg(&script_path)
        .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| Failure::Script(e.to_string()))?;
    Ok(())
}

#[cfg(not(windows))]
pub fn launch_swap(_zip: &std::path::Path) -> Result<(), Failure> {
    Err(Failure::Script(
        "In-place update is a Windows-only path".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_numerically_not_as_strings() {
        // The case a string comparison gets backwards, and the reason this
        // function exists at all.
        assert!(is_newer("1.0.10", "1.0.9"));
        assert!(!is_newer("1.0.9", "1.0.10"));
        assert!(is_newer("2.0.0", "1.99.99"));
    }

    #[test]
    fn a_leading_v_is_ignored_on_either_side() {
        assert!(is_newer("v1.1.0", "1.0.0"));
        assert!(is_newer("1.1.0", "v1.0.0"));
        assert!(!is_newer("v1.0.0", "v1.0.0"));
    }

    #[test]
    fn a_missing_component_reads_as_zero() {
        // 1.1 and 1.1.0 are the same release, not different ones.
        assert!(!is_newer("1.1", "1.1.0"));
        assert!(!is_newer("1.1.0", "1.1"));
        assert!(is_newer("1.1.1", "1.1"));
    }

    #[test]
    fn the_same_version_is_not_an_update() {
        assert!(!is_newer("1.0.0", "1.0.0"));
    }

    #[test]
    fn a_prerelease_suffix_is_ignored_for_ordering() {
        // The numbers are what order releases; the suffix only says which
        // channel, and a pre-release is filtered out before it gets here.
        assert_eq!(version_parts("1.2.3-beta.1"), vec![1, 2, 3]);
        assert_eq!(version_parts("v0.1.0"), vec![0, 1, 0]);
    }

    #[test]
    fn a_non_numeric_component_ends_the_parse_rather_than_counting_as_zero() {
        assert_eq!(version_parts("1.2.x"), vec![1, 2]);
        assert!(version_parts("nightly").is_empty());
    }

    #[test]
    fn a_version_with_no_numbers_is_never_an_upgrade() {
        assert!(!is_newer("nightly", "1.0.0"));
        assert!(!is_newer("", "1.0.0"));
    }

    #[test]
    fn the_zip_is_picked_over_the_bare_exe() {
        // An update has to replace the core and wintun.dll too; swapping only
        // moonlight.exe leaves a new client on an old core.
        let names = [
            "Moonlight.exe",
            "Moonlight-Helper.exe",
            "Moonlight-x86_64.zip",
            "SHA256SUMS.txt",
        ];
        assert_eq!(pick_asset(names.into_iter()), Some("Moonlight-x86_64.zip"));
    }

    #[test]
    fn a_release_with_no_zip_has_no_asset_to_offer() {
        assert_eq!(pick_asset(["Moonlight.exe", "notes.txt"].into_iter()), None);
        // A zip for another architecture is not this one.
        assert_eq!(pick_asset(["Moonlight-arm64.zip"].into_iter()), None);
    }

    fn release_json(tag: &str, draft: bool, prerelease: bool) -> String {
        format!(
            r#"{{"tag_name":"{tag}","body":"notes","draft":{draft},"prerelease":{prerelease},
                "assets":[{{"name":"Moonlight-x86_64.zip",
                            "browser_download_url":"https://example/{tag}.zip","size":1234}}]}}"#
        )
    }

    #[test]
    fn a_newer_release_is_offered() {
        let body = format!("[{}]", release_json("v0.2.0", false, false));
        match evaluate(&body, "0.1.0").expect("evaluates") {
            Outcome::Available(release) => {
                assert_eq!(release.version, "0.2.0");
                assert_eq!(release.download_url, "https://example/v0.2.0.zip");
                assert_eq!(release.size, 1234);
            }
            other => panic!("expected an update, got {other:?}"),
        }
    }

    #[test]
    fn the_current_version_reports_up_to_date() {
        let body = format!("[{}]", release_json("v0.1.0", false, false));
        assert_eq!(
            evaluate(&body, "0.1.0").expect("evaluates"),
            Outcome::UpToDate {
                current: "0.1.0".into()
            }
        );
    }

    #[test]
    fn drafts_and_prereleases_are_never_pushed() {
        // Neither was offered to everyone, so neither should update a user who
        // did not opt in.
        let draft = format!("[{}]", release_json("v9.0.0", true, false));
        let pre = format!("[{}]", release_json("v9.0.0", false, true));
        assert!(matches!(
            evaluate(&draft, "0.1.0"),
            Ok(Outcome::UpToDate { .. })
        ));
        assert!(matches!(
            evaluate(&pre, "0.1.0"),
            Ok(Outcome::UpToDate { .. })
        ));
    }

    #[test]
    fn the_highest_version_wins_regardless_of_list_order() {
        // GitHub returns newest-first, but that is by date, and a patch to an
        // old branch can be published after a newer minor.
        let body = format!(
            "[{},{},{}]",
            release_json("v0.2.0", false, false),
            release_json("v0.10.0", false, false),
            release_json("v0.3.0", false, false)
        );
        match evaluate(&body, "0.1.0").expect("evaluates") {
            Outcome::Available(release) => assert_eq!(release.version, "0.10.0"),
            other => panic!("expected an update, got {other:?}"),
        }
    }

    #[test]
    fn a_single_object_from_releases_latest_also_parses() {
        let body = release_json("v0.2.0", false, false);
        assert!(matches!(
            evaluate(&body, "0.1.0"),
            Ok(Outcome::Available(_))
        ));
    }

    #[test]
    fn a_newer_release_with_no_windows_zip_is_an_error_not_an_offer() {
        let body = r#"[{"tag_name":"v9.0.0","body":"","draft":false,"prerelease":false,
                       "assets":[{"name":"Moonlight.dmg","browser_download_url":"x","size":1}]}]"#;
        assert!(matches!(evaluate(body, "0.1.0"), Err(Failure::NoAsset)));
    }

    #[test]
    fn junk_json_is_refused() {
        assert!(matches!(
            evaluate("not json", "0.1.0"),
            Err(Failure::NoAsset)
        ));
    }

    // The script

    fn script() -> String {
        swap_script(
            4242,
            r"C:\Temp\m.zip",
            r"C:\Apps\Moonlight",
            "moonlight.exe",
        )
    }

    /// The body of one `:label` branch, up to the next label.
    ///
    /// Splitting on the label alone returns the rest of the file, which made an
    /// earlier version of these tests assert against every branch at once.
    fn branch(script: &str, label: &str) -> String {
        let start = script
            .find(&format!("\n{label}\n"))
            .expect("the label exists");
        let rest = &script[start + label.len() + 2..];
        match rest.find("\n:") {
            Some(end) => rest[..end].to_string(),
            None => rest.to_string(),
        }
    }

    #[test]
    fn the_script_waits_for_the_process_before_touching_anything() {
        let s = script();
        let wait = s.find("tasklist").expect("waits for the pid");
        let touch = s.find("move \"%DIR%\"").expect("moves the folder");
        assert!(
            wait < touch,
            "the folder must not be moved before the app has exited"
        );
    }

    #[test]
    fn the_script_moves_the_old_install_aside_rather_than_deleting_it() {
        let s = script();
        assert!(s.contains(r#"move "%DIR%" "%BACKUP%""#));
        // And the backup exists before the unpack, so there is something to
        // restore from.
        let backup = s.find(r#"move "%DIR%" "%BACKUP%""#).unwrap();
        let unpack = s.find("Expand-Archive").unwrap();
        assert!(backup < unpack);
    }

    #[test]
    fn a_failed_unpack_restores_the_old_installation() {
        let s = script();
        assert!(s.contains(":restore"));
        assert!(s.contains(r#"move "%BACKUP%" "%DIR%""#));
        // And starts it, because a working old client beats no client.
        let restore = branch(&s, ":restore");
        assert!(restore.contains("start"));
    }

    #[test]
    fn the_unpack_is_checked_for_having_produced_the_executable() {
        // Expand-Archive can succeed on a truncated zip and leave no .exe,
        // which would otherwise relaunch nothing and delete the backup.
        let s = script();
        assert!(s.contains(r#"if not exist "%DIR%\%EXE%" goto restore"#));
    }

    #[test]
    fn the_script_gives_up_rather_than_replacing_files_under_a_live_app() {
        let s = script();
        assert!(s.contains(":giveup"));
        let giveup = branch(&s, ":giveup");
        assert!(
            !giveup.contains("move \"%DIR%\""),
            "giving up must not touch the installation"
        );
        assert!(giveup.contains("exit /b 1"), "and must not fall through");
    }

    #[test]
    fn the_script_carries_the_values_it_was_given() {
        let s = script();
        assert!(s.contains("set \"PID=4242\""));
        assert!(s.contains(r#"set "ZIP=C:\Temp\m.zip""#));
        assert!(s.contains(r#"set "DIR=C:\Apps\Moonlight""#));
        assert!(s.contains(r#"set "BACKUP=C:\Apps\Moonlight.old""#));
        assert!(s.contains("set \"EXE=moonlight.exe\""));
    }

    #[test]
    fn the_script_deletes_itself_on_both_paths() {
        // It lives in %TEMP%; leaving one behind per update is untidy, and a
        // stale one that ran halfway is worse.
        let s = script();
        assert_eq!(s.matches(r#"del "%~f0""#).count(), 2);
    }
}
