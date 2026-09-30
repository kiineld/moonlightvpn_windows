//! Checks GitHub for a newer release, and installs it.
//!
//! There is no store to do this, so the app does what a user would otherwise
//! do by hand: ask GitHub for the latest release, download its installer, and
//! run it.
//!
//! ## How an update installs
//!
//! The release's `Moonlight-Setup.exe` is downloaded and checked against the
//! release's own `SHA256SUMS.txt` **before the app quits**, so a damaged
//! download leaves the working version running rather than no version at all.
//! The app then puts the machine back and quits, and Setup runs silently: it
//! replaces the installation — rolling its own changes back if anything fails,
//! which is Inno Setup's behaviour, not a script's — re-registers the TUN
//! service with the tasks chosen at first install, and starts the new version.
//!
//! The installer, not the zip. Updating used to mean unpacking the zip over the
//! install folder from a detached batch script, unattended, with the app
//! already gone — and that script had never run against a real release. Setup
//! does all of it already, and is the same file a new user installs with.
//!
//! ## Why versions are compared numerically
//!
//! `"1.0.10" < "1.0.9"` as strings, so a string comparison stops offering
//! updates at the tenth patch and never says why.

use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum Failure {
    #[error("Could not reach GitHub: {0}")]
    Transport(String),
    #[error("GitHub returned HTTP {0}")]
    Http(u16),
    #[error("The latest release has no Windows installer")]
    NoAsset,
    #[error("The release publishes no checksum for its installer")]
    NoChecksum,
    #[error("The download does not match the release's checksum")]
    Checksum,
    #[error("{0}")]
    Io(String),
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
    /// The installer's file name, as the checksum list names it.
    pub asset_name: String,
    pub size: u64,
    /// `SHA256SUMS.txt`, which every release since the installer publishes.
    pub checksums_url: Option<String>,
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

/// Picks the installer from a release's attachments. The bare executables are
/// for replacing one file by hand — downloading one and running it would
/// launch the app, not update anything.
pub fn pick_asset<'a>(names: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    let mut installers: Vec<&str> = names
        .filter(|n| {
            let lower = n.to_lowercase();
            lower.ends_with(".exe") && lower.contains("setup")
        })
        .collect();
    installers.sort_unstable();
    installers.first().copied()
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
    let checksums_url = release
        .assets
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case("SHA256SUMS.txt"))
        .map(|a| a.browser_download_url.clone());

    Ok(Outcome::Available(Release {
        version: release.tag_name.trim_start_matches(['v', 'V']).to_string(),
        notes: release.body.clone(),
        download_url: asset.browser_download_url.clone(),
        asset_name,
        size: asset.size,
        checksums_url,
    }))
}

/// A client that ignores the machine's proxy settings: while connected in
/// system-proxy mode the app has pointed the machine at its own core, and an
/// update should not depend on the tunnel it is about to replace the app for.
fn client(timeout_secs: u64) -> Result<reqwest::Client, Failure> {
    reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build()
        .map_err(|e| Failure::Transport(e.to_string()))
}

async fn get_text(url: &str, user_agent: &str) -> Result<String, Failure> {
    let response = client(30)?
        .get(url)
        .header("User-Agent", user_agent)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| Failure::Transport(e.without_url().to_string()))?;
    let status = response.status();
    if !status.is_success() {
        return Err(Failure::Http(status.as_u16()));
    }
    response
        .text()
        .await
        .map_err(|e| Failure::Transport(e.without_url().to_string()))
}

/// Asks GitHub what the latest release is.
pub async fn check(releases_api: &str, current_version: &str) -> Result<Outcome, Failure> {
    let body = get_text(releases_api, &format!("moonlight/{current_version}")).await?;
    evaluate(&body, current_version)
}

/// Downloads, reporting bytes received and the total as they arrive — a 25 MB
/// installer on a slow link is otherwise half a minute of a button that looks
/// like it did nothing. The total is `None` when the server does not say.
///
/// Written whole to a `.part` beside `destination` and moved into place, so an
/// interrupted download cannot leave a half-installer looking complete.
pub async fn download_with_progress(
    url: &str,
    destination: &std::path::Path,
    mut progress: impl FnMut(u64, Option<u64>),
) -> Result<(), Failure> {
    use futures_util::StreamExt;

    let response = client(900)?
        .get(url)
        .header("User-Agent", "moonlight")
        .send()
        .await
        .map_err(|e| Failure::Transport(e.without_url().to_string()))?;
    if !response.status().is_success() {
        return Err(Failure::Http(response.status().as_u16()));
    }

    let total = response.content_length();
    let mut received: u64 = 0;
    let mut buffer: Vec<u8> = Vec::with_capacity(total.unwrap_or(0) as usize);
    let mut stream = response.bytes_stream();
    progress(0, total);
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| Failure::Transport(e.without_url().to_string()))?;
        received += chunk.len() as u64;
        buffer.extend_from_slice(&chunk);
        progress(received, total);
    }

    let partial = destination.with_extension("part");
    std::fs::write(&partial, &buffer).map_err(|e| Failure::Io(e.to_string()))?;
    std::fs::rename(&partial, destination).map_err(|e| Failure::Io(e.to_string()))
}

/// The hash `SHA256SUMS.txt` gives for `name`: lines of `<hex>  <name>`, or
/// `<hex> *<name>` in binary mode, matched on the file name.
pub fn expected_hash(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let (hash, file) = line.trim().split_once(char::is_whitespace)?;
        let file = file.trim().trim_start_matches('*');
        (file.eq_ignore_ascii_case(name)
            && hash.len() == 64
            && hash.chars().all(|c| c.is_ascii_hexdigit()))
        .then(|| hash.to_ascii_lowercase())
    })
}

/// SHA-256 of a file, in lower-case hex, through Windows' own CNG — the
/// one-shot hash every Windows 10 has, rather than a dependency for it.
#[cfg(windows)]
pub fn sha256_hex(path: &std::path::Path) -> Result<String, Failure> {
    use windows::Win32::Security::Cryptography::{BCryptHash, BCRYPT_SHA256_ALG_HANDLE};
    let data = std::fs::read(path).map_err(|e| Failure::Io(e.to_string()))?;
    let mut digest = [0u8; 32];
    let status = unsafe { BCryptHash(BCRYPT_SHA256_ALG_HANDLE, None, &data, &mut digest) };
    if status.is_err() {
        return Err(Failure::Io(format!("hashing failed: {status:?}")));
    }
    Ok(digest.iter().map(|b| format!("{b:02x}")).collect())
}

#[cfg(not(windows))]
pub fn sha256_hex(_path: &std::path::Path) -> Result<String, Failure> {
    Err(Failure::Io("hashing is done with Windows' CNG".into()))
}

/// Checks the downloaded installer against the release's checksum list. Done
/// before the app quits, so a damaged download changes nothing.
pub async fn verify(release: &Release, file: &std::path::Path) -> Result<(), Failure> {
    let url = release
        .checksums_url
        .as_deref()
        .ok_or(Failure::NoChecksum)?;
    let sums = get_text(url, "moonlight").await?;
    let expected = expected_hash(&sums, &release.asset_name).ok_or(Failure::NoChecksum)?;
    let actual = sha256_hex(file)?;
    if actual == expected {
        Ok(())
    } else {
        Err(Failure::Checksum)
    }
}

/// How Setup is asked to run for an update: no questions and no wizard pages,
/// only its progress window; the previous choices of tasks (the TUN service, a
/// desktop icon) are Inno's default; no reboot; and the language the app is in.
pub fn installer_arguments(russian: bool) -> String {
    format!(
        "/SP- /SILENT /SUPPRESSMSGBOXES /NORESTART /CLOSEAPPLICATIONS /LANG={}",
        if russian { "ru" } else { "en" }
    )
}

/// Hands the installer to Windows to run, with its elevation prompt.
///
/// Through the shell rather than `CreateProcess`: Setup asks for administrator
/// rights, and `CreateProcess` refuses an executable that does
/// (`ERROR_ELEVATION_REQUIRED`) instead of showing the prompt.
#[cfg(windows)]
pub fn launch_installer(installer: &std::path::Path, arguments: &str) -> Result<(), Failure> {
    use windows::core::{w, HSTRING};
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            &HSTRING::from(installer.as_os_str()),
            &HSTRING::from(arguments),
            None,
            SW_SHOWNORMAL,
        )
    };
    // Anything above 32 is success; below is an error code.
    if result.0 as isize > 32 {
        Ok(())
    } else {
        Err(Failure::Io(format!(
            "the installer did not start ({})",
            result.0 as isize
        )))
    }
}

#[cfg(not(windows))]
pub fn launch_installer(_installer: &std::path::Path, _arguments: &str) -> Result<(), Failure> {
    Err(Failure::Io("Windows only".into()))
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
    fn the_installer_is_the_asset_and_nothing_else_is() {
        let names = [
            "Moonlight-Helper.exe",
            "Moonlight-Setup.exe",
            "Moonlight-x86_64.zip",
            "Moonlight.exe",
            "SHA256SUMS.txt",
        ];
        assert_eq!(pick_asset(names.into_iter()), Some("Moonlight-Setup.exe"));
        // The bare executables launch the app rather than install anything.
        assert_eq!(
            pick_asset(["Moonlight.exe", "Moonlight-Helper.exe"].into_iter()),
            None
        );
        assert_eq!(pick_asset(["Moonlight-x86_64.zip"].into_iter()), None);
    }

    fn release_json(tag: &str, draft: bool, prerelease: bool) -> String {
        format!(
            r#"{{"tag_name":"{tag}","body":"notes","draft":{draft},"prerelease":{prerelease},
                "assets":[{{"name":"Moonlight-Setup.exe","browser_download_url":"https://example/{tag}.exe","size":1234}},
                          {{"name":"SHA256SUMS.txt","browser_download_url":"https://example/{tag}.sums","size":10}}]}}"#
        )
    }

    #[test]
    fn a_newer_release_is_offered_with_its_checksums() {
        let body = format!("[{}]", release_json("v0.2.0", false, false));
        match evaluate(&body, "0.1.0").expect("evaluates") {
            Outcome::Available(release) => {
                assert_eq!(release.version, "0.2.0");
                assert_eq!(release.download_url, "https://example/v0.2.0.exe");
                assert_eq!(release.asset_name, "Moonlight-Setup.exe");
                assert_eq!(release.size, 1234);
                assert_eq!(
                    release.checksums_url.as_deref(),
                    Some("https://example/v0.2.0.sums")
                );
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
    fn a_newer_release_with_no_installer_is_an_error_not_an_offer() {
        let body = r#"[{"tag_name":"v9.0.0","body":"","draft":false,"prerelease":false,
                       "assets":[{"name":"Moonlight-x86_64.zip","browser_download_url":"x","size":1}]}]"#;
        assert_eq!(evaluate(body, "0.1.0"), Err(Failure::NoAsset));
        assert_eq!(evaluate("not json", "0.1.0"), Err(Failure::NoAsset));
    }

    #[test]
    fn the_checksum_list_is_read_the_way_sha256sum_writes_it() {
        let hash = "a".repeat(64);
        let other = "b".repeat(64);
        let sums = format!(
            "{other}  Moonlight-x86_64.zip\n{hash}  Moonlight-Setup.exe\n{other} *Moonlight.exe\n"
        );
        assert_eq!(expected_hash(&sums, "Moonlight-Setup.exe"), Some(hash));
        assert_eq!(
            expected_hash(&sums, "moonlight.exe"),
            Some(other),
            "binary mode, any case"
        );
        assert_eq!(expected_hash(&sums, "Nope.exe"), None);
        assert_eq!(
            expected_hash("xyz  Moonlight-Setup.exe", "Moonlight-Setup.exe"),
            None
        );
    }

    #[cfg(windows)]
    #[test]
    fn the_hash_is_sha256() {
        let path = std::env::temp_dir().join(format!("ml-hash-{}", uuid::Uuid::new_v4()));
        std::fs::write(&path, b"abc").unwrap();
        // The FIPS 180-2 test vector for "abc".
        assert_eq!(
            sha256_hex(&path).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn setup_is_asked_to_run_without_questions_in_the_apps_language() {
        let ru = installer_arguments(true);
        assert!(
            ru.contains("/SILENT") && ru.contains("/SUPPRESSMSGBOXES") && ru.ends_with("/LANG=ru")
        );
        assert!(installer_arguments(false).ends_with("/LANG=en"));
        assert!(
            !ru.contains("/VERYSILENT"),
            "its progress window is the only sign it is working"
        );
    }
}
