//! Lists the applications a user might want to route, so the Apps screen can
//! offer them instead of asking for an executable name.
//!
//! macOS has one answer to "what is installed": `/Applications`, where every
//! entry is a bundle with a name and an executable. Windows has none. An
//! application can live in `Program Files`, in `%LOCALAPPDATA%`, in a Steam
//! library on another drive, or nowhere at all if it is portable. So this reads
//! the three sources that between them cover what people actually have:
//!
//! 1. **Start Menu shortcuts.** The closest thing Windows has to an installed
//!    applications list, and the only source that carries the *display* name a
//!    user would recognise — `Google Chrome` rather than `chrome.exe`. Both the
//!    all-users and per-user menus are read.
//! 2. **`Program Files` and `Program Files (x86)`,** one level of executables
//!    deep, for things that install without a shortcut.
//! 3. **Running processes**, which catch the portable and the
//!    installed-somewhere-odd. They are also what lets the screen mark an entry
//!    *Запущено*, which is the fastest way for someone to find the program they
//!    are looking at right now.
//!
//! The three are merged on the executable name, because that is what mihomo
//! matches: a `PROCESS-NAME` rule sees `chrome.exe`, so the executable is the
//! identity and everything else is decoration.

use std::collections::BTreeMap;

use crate::models::AppEntry;

/// Executables that are part of Windows rather than something a user installed.
///
/// Without this the list is mostly `svchost.exe`, and the handful of real
/// applications are lost in it. Matched case-insensitively on the file name.
const SYSTEM_EXECUTABLES: &[&str] = &[
    "applicationframehost.exe",
    "audiodg.exe",
    "backgroundtaskhost.exe",
    "conhost.exe",
    "csrss.exe",
    "ctfmon.exe",
    "dasHost.exe",
    "dllhost.exe",
    "dwm.exe",
    "explorer.exe",
    "fontdrvhost.exe",
    "lsass.exe",
    "memory compression",
    "microsoftedgeupdate.exe",
    "registry",
    "runtimebroker.exe",
    "searchapp.exe",
    "searchhost.exe",
    "searchindexer.exe",
    "服务",
    "services.exe",
    "shellexperiencehost.exe",
    "sihost.exe",
    "smartscreen.exe",
    "smss.exe",
    "spoolsv.exe",
    "startmenuexperiencehost.exe",
    "svchost.exe",
    "system",
    "system idle process",
    "taskhostw.exe",
    "textinputhost.exe",
    "wininit.exe",
    "winlogon.exe",
    "wudfhost.exe",
];

/// Directories whose executables are uninstallers, crash handlers and helpers
/// rather than applications.
const NOISE_SUBSTRINGS: &[&str] = &[
    "unins",
    "crashpad",
    "crashhandler",
    "setup",
    "installer",
    "updater",
    "update.exe",
    "vcredist",
    "dxsetup",
    "helper.exe",
];

pub fn is_system_executable(executable: &str) -> bool {
    let lowered = executable.to_lowercase();
    SYSTEM_EXECUTABLES.contains(&lowered.as_str())
}

/// Whether mihomo could ever match this with a `PROCESS-NAME` rule.
///
/// It reads the executable's file name out of the process table, so an entry
/// without one cannot be matched however the rule is written. The kernel's own
/// pseudo-processes are the ones that reach here: PID 0 reports as
/// `[System Process]`, PID 4 as `System`, and the memory compressor as
/// `Memory Compression` — all bracketed or bare, none of them files.
///
/// Found by the integration suite on a real Windows runner, where
/// `[System Process]` was being offered in the Apps list as something a user
/// could route.
pub fn is_matchable(executable: &str) -> bool {
    !executable.starts_with('[') && executable.to_lowercase().ends_with(".exe")
}

pub fn is_noise(executable: &str) -> bool {
    let lowered = executable.to_lowercase();
    NOISE_SUBSTRINGS.iter().any(|n| lowered.contains(n))
}

/// A display name from a file stem: `google chrome` → `Google Chrome`.
///
/// Only used for entries with no shortcut, since a shortcut already carries the
/// name its installer chose.
pub fn title_case(stem: &str) -> String {
    stem.split([' ', '_', '-'])
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                // Left alone if it already has capitals of its own — `PyCharm`
                // and `qBittorrent` must not become `Pycharm` and `Qbittorrent`.
                Some(first) if word.chars().skip(1).any(char::is_uppercase) => {
                    first.to_string() + chars.as_str()
                }
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// One candidate, before the three sources are merged.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub name: String,
    pub executable: String,
    pub path: String,
    pub running: bool,
    /// A name that came from a shortcut is the one an installer chose, so it
    /// beats a name derived from a file stem.
    pub from_shortcut: bool,
}

/// Merges candidates from every source, keyed on the executable name — which is
/// the identity, because it is what mihomo's `PROCESS-NAME` rules match.
///
/// Where two sources disagree on the display name, a shortcut wins; where they
/// disagree on whether it is running, running wins. That makes the merge
/// order-independent, which matters because the three scans finish in whatever
/// order the filesystem answers in.
pub fn merge(candidates: Vec<Candidate>) -> Vec<AppEntry> {
    let mut merged: BTreeMap<String, Candidate> = BTreeMap::new();

    for candidate in candidates {
        if candidate.executable.is_empty()
            || !is_matchable(&candidate.executable)
            || is_system_executable(&candidate.executable)
            || is_noise(&candidate.executable)
        {
            continue;
        }
        let key = candidate.executable.to_lowercase();
        match merged.get_mut(&key) {
            None => {
                merged.insert(key, candidate);
            }
            Some(existing) => {
                if candidate.from_shortcut && !existing.from_shortcut {
                    existing.name = candidate.name;
                    existing.from_shortcut = true;
                }
                if !existing.path.is_empty() || candidate.path.is_empty() {
                    // Keep the first real path; a running process gives an
                    // absolute one, which is the most useful for PROCESS-PATH.
                } else {
                    existing.path = candidate.path;
                }
                existing.running |= candidate.running;
            }
        }
    }

    let mut entries: Vec<AppEntry> = merged
        .into_values()
        .map(|c| AppEntry {
            name: c.name,
            executable: c.executable,
            path: c.path,
        })
        .collect();

    // Sorted by display name, case-insensitively, because the list is read
    // alphabetically and `Telegram` must not sort before `discord`.
    entries.sort_by_key(|e| e.name.to_lowercase());
    entries
}

/// Which of `entries` are running right now.
///
/// Kept separate from the scan so the Apps screen can refresh the *Запущено*
/// pills once a second without re-walking `Program Files` — the walk takes
/// hundreds of milliseconds, the process snapshot takes one.
pub fn running_executables() -> Vec<String> {
    imp::running_executables()
}

/// The full scan. Slow enough to belong off the UI thread.
pub fn scan() -> Vec<AppEntry> {
    imp::scan()
}

/// The file name of a path, as mihomo would report it.
///
/// Split on both separators explicitly rather than through `Path::file_name`:
/// these are Windows paths whichever machine is reading them, and off Windows
/// `Path` does not treat a backslash as a separator — so the whole path came
/// back as the "file name", which is a rule that is accepted and never matches.
pub fn executable_name(path: &str) -> String {
    path.rsplit(['\\', '/'])
        .next()
        .unwrap_or_default()
        .to_string()
}

#[cfg(windows)]
mod imp {
    use super::*;
    use std::ffi::OsString;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::path::{Path, PathBuf};

    use windows::core::{Interface, PCWSTR};
    use windows::Win32::Foundation::{CloseHandle, MAX_PATH};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, IPersistFile, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink, SLGP_RAWPATH};

    pub fn scan() -> Vec<AppEntry> {
        // The COM apartment is initialised once per thread. An already-
        // initialised thread returns RPC_E_CHANGED_MODE, which is not a failure
        // for our purposes: we only need *an* apartment, not ours.
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }

        let mut candidates = Vec::new();
        candidates.extend(from_start_menu());
        candidates.extend(from_program_files());
        candidates.extend(from_processes());
        merge(candidates)
    }

    fn from_start_menu() -> Vec<Candidate> {
        let mut roots = Vec::new();
        if let Some(all) = std::env::var_os("ProgramData") {
            roots.push(PathBuf::from(all).join(r"Microsoft\Windows\Start Menu\Programs"));
        }
        if let Some(user) = std::env::var_os("APPDATA") {
            roots.push(PathBuf::from(user).join(r"Microsoft\Windows\Start Menu\Programs"));
        }

        let mut out = Vec::new();
        for root in roots {
            walk(&root, 4, &mut |path| {
                if path.extension().and_then(|e| e.to_str()) != Some("lnk") {
                    return;
                }
                let Some(target) = resolve_shortcut(path) else {
                    return;
                };
                if target
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(str::to_lowercase)
                    != Some("exe".to_string())
                {
                    return;
                }
                let executable = super::executable_name(&target.to_string_lossy());
                // The shortcut's own file name is the label the installer chose
                // and the user reads in the Start Menu.
                let name = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| executable.clone());
                out.push(Candidate {
                    name,
                    executable,
                    path: target.to_string_lossy().to_string(),
                    running: false,
                    from_shortcut: true,
                });
            });
        }
        out
    }

    fn from_program_files() -> Vec<Candidate> {
        let mut out = Vec::new();
        for key in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
            let Some(root) = std::env::var_os(key) else {
                continue;
            };
            let root = PathBuf::from(root);
            // Two levels: `Program Files\Vendor\App\app.exe` is the common
            // shape, and going deeper turns a scan into a full-disk walk.
            walk(&root, 3, &mut |path| {
                if path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(str::to_lowercase)
                    != Some("exe".to_string())
                {
                    return;
                }
                let executable = super::executable_name(&path.to_string_lossy());
                let stem = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                out.push(Candidate {
                    name: super::title_case(&stem),
                    executable,
                    path: path.to_string_lossy().to_string(),
                    running: false,
                    from_shortcut: false,
                });
            });
        }
        out
    }

    fn from_processes() -> Vec<Candidate> {
        running_executables()
            .into_iter()
            .map(|executable| {
                let stem = executable.trim_end_matches(".exe").to_string();
                Candidate {
                    name: super::title_case(&stem),
                    executable,
                    path: String::new(),
                    running: true,
                    from_shortcut: false,
                }
            })
            .collect()
    }

    pub fn running_executables() -> Vec<String> {
        let mut out = Vec::new();
        unsafe {
            let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
                return out;
            };
            let mut entry = PROCESSENTRY32W {
                dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                ..Default::default()
            };
            if Process32FirstW(snapshot, &mut entry).is_ok() {
                loop {
                    let name = wide_to_string(&entry.szExeFile);
                    if !name.is_empty() {
                        out.push(name);
                    }
                    if Process32NextW(snapshot, &mut entry).is_err() {
                        break;
                    }
                }
            }
            let _ = CloseHandle(snapshot);
        }
        out.sort();
        out.dedup();
        out
    }

    /// Reads a `.lnk`'s target through the shell, rather than parsing the
    /// format by hand. `SLGP_RAWPATH` keeps the path as written instead of
    /// resolving it against the current machine, which is what stops a missing
    /// target from triggering the shell's "search for this file" dialog.
    fn resolve_shortcut(path: &Path) -> Option<PathBuf> {
        unsafe {
            let link: IShellLinkW =
                CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
            let file: IPersistFile = link.cast().ok()?;

            let wide: Vec<u16> = path
                .as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            file.Load(
                PCWSTR(wide.as_ptr()),
                windows::Win32::System::Com::STGM_READ,
            )
            .ok()?;

            let mut buffer = [0u16; MAX_PATH as usize];
            link.GetPath(&mut buffer, std::ptr::null_mut(), SLGP_RAWPATH.0 as u32)
                .ok()?;

            let target = wide_to_string(&buffer);
            (!target.is_empty()).then(|| PathBuf::from(target))
        }
    }

    fn wide_to_string(buffer: &[u16]) -> String {
        let end = buffer.iter().position(|c| *c == 0).unwrap_or(buffer.len());
        OsString::from_wide(&buffer[..end])
            .to_string_lossy()
            .to_string()
    }

    /// A bounded directory walk.
    ///
    /// `depth` is what keeps this from becoming a full-disk scan: `Program
    /// Files` contains vendor trees tens of levels deep, and an application's
    /// own executable is never more than two or three down.
    fn walk(root: &Path, depth: usize, visit: &mut impl FnMut(&Path)) {
        if depth == 0 {
            return;
        }
        let Ok(entries) = std::fs::read_dir(root) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            match entry.file_type() {
                Ok(kind) if kind.is_dir() => walk(&path, depth - 1, visit),
                Ok(kind) if kind.is_file() => visit(&path),
                // Symlinks and junctions are skipped rather than followed: a
                // loop in Program Files would otherwise hang the scan.
                _ => {}
            }
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;

    // There is nothing meaningful to enumerate off Windows, and inventing
    // entries would let the Apps screen look populated in a build that cannot
    // route any of them.
    pub fn scan() -> Vec<AppEntry> {
        Vec::new()
    }

    pub fn running_executables() -> Vec<String> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(name: &str, executable: &str, from_shortcut: bool, running: bool) -> Candidate {
        Candidate {
            name: name.into(),
            executable: executable.into(),
            path: String::new(),
            running,
            from_shortcut,
        }
    }

    #[test]
    fn the_executable_is_the_identity() {
        // Two sources describing the same program merge into one entry, because
        // that is what a PROCESS-NAME rule matches on.
        let merged = merge(vec![
            candidate("Google Chrome", "chrome.exe", true, false),
            candidate("Chrome", "chrome.exe", false, true),
        ]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].executable, "chrome.exe");
    }

    #[test]
    fn a_shortcut_name_beats_a_derived_one_in_either_order() {
        // The scans finish in whatever order the filesystem answers, so the
        // merge must not depend on it.
        let forwards = merge(vec![
            candidate("Chrome", "chrome.exe", false, false),
            candidate("Google Chrome", "chrome.exe", true, false),
        ]);
        let backwards = merge(vec![
            candidate("Google Chrome", "chrome.exe", true, false),
            candidate("Chrome", "chrome.exe", false, false),
        ]);
        assert_eq!(forwards[0].name, "Google Chrome");
        assert_eq!(backwards[0].name, "Google Chrome");
    }

    #[test]
    fn executables_are_matched_case_insensitively() {
        // The Start Menu says Chrome.exe, the process table says chrome.exe.
        let merged = merge(vec![
            candidate("Google Chrome", "Chrome.exe", true, false),
            candidate("Chrome", "chrome.exe", false, true),
        ]);
        assert_eq!(merged.len(), 1, "the same program listed twice");
    }

    #[test]
    fn a_process_that_is_not_a_file_is_never_offered() {
        // PID 0 and PID 4 are kernel pseudo-processes with no executable, so a
        // PROCESS-NAME rule for them could never match however it is written.
        // The Apps list was offering them until a real Windows runner said so.
        assert!(!is_matchable("[System Process]"));
        assert!(!is_matchable("System"));
        assert!(!is_matchable("Memory Compression"));
        assert!(!is_matchable("Registry"));
        assert!(is_matchable("chrome.exe"));
        assert!(is_matchable("Telegram.exe"));

        let merged = merge(vec![
            candidate("System Process", "[System Process]", false, true),
            candidate("System", "System", false, true),
            candidate("Telegram", "Telegram.exe", true, true),
        ]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].executable, "Telegram.exe");
    }

    #[test]
    fn windows_own_processes_are_left_out() {
        // Without this the list is mostly svchost and the real applications are
        // lost in it.
        let merged = merge(vec![
            candidate("Svchost", "svchost.exe", false, true),
            candidate("Explorer", "explorer.exe", false, true),
            candidate("Telegram", "Telegram.exe", true, true),
        ]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].executable, "Telegram.exe");
    }

    #[test]
    fn uninstallers_and_crash_handlers_are_left_out() {
        let merged = merge(vec![
            candidate("Unins000", "unins000.exe", false, false),
            candidate("Crashpad", "crashpad_handler.exe", false, false),
            candidate("Setup", "setup.exe", false, false),
            candidate("Discord", "Discord.exe", true, false),
        ]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].executable, "Discord.exe");
    }

    #[test]
    fn running_wins_regardless_of_which_source_saw_it() {
        let merged = merge(vec![
            candidate("Telegram", "telegram.exe", true, false),
            candidate("Telegram", "telegram.exe", false, true),
        ]);
        assert_eq!(merged.len(), 1);
        // The merged entry has to carry the running flag for the Запущено pill,
        // and the sources disagree by construction.
        assert_eq!(merged[0].executable, "telegram.exe");
    }

    #[test]
    fn the_list_is_sorted_case_insensitively() {
        let merged = merge(vec![
            candidate("Telegram", "telegram.exe", true, false),
            candidate("discord", "discord.exe", true, false),
            candidate("Arc", "arc.exe", true, false),
        ]);
        let names: Vec<&str> = merged.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["Arc", "discord", "Telegram"]);
    }

    #[test]
    fn an_entry_with_no_executable_is_dropped() {
        assert!(merge(vec![candidate("Ghost", "", true, false)]).is_empty());
    }

    #[test]
    fn title_case_capitalises_a_plain_stem() {
        assert_eq!(title_case("telegram"), "Telegram");
        assert_eq!(title_case("google chrome"), "Google Chrome");
        assert_eq!(title_case("visual_studio_code"), "Visual Studio Code");
    }

    #[test]
    fn title_case_leaves_a_name_that_capitalises_itself() {
        // PyCharm and qBittorrent must not become Pycharm and Qbittorrent.
        assert_eq!(title_case("PyCharm"), "PyCharm");
        assert_eq!(title_case("qBittorrent"), "qBittorrent");
        assert_eq!(title_case("VLC"), "VLC");
    }

    #[test]
    fn the_executable_name_comes_off_either_separator() {
        assert_eq!(
            executable_name(r"C:\Program Files\App\thing.exe"),
            "thing.exe"
        );
        assert_eq!(executable_name("thing.exe"), "thing.exe");
    }

    #[test]
    fn system_and_noise_checks_are_case_insensitive() {
        assert!(is_system_executable("SVCHOST.EXE"));
        assert!(is_system_executable("svchost.exe"));
        assert!(is_noise("Unins000.exe"));
        assert!(!is_noise("Discord.exe"));
    }

    #[cfg(not(windows))]
    #[test]
    fn the_non_windows_scan_is_empty_rather_than_invented() {
        // Inventing entries would let the Apps screen look populated in a build
        // that cannot route any of them.
        assert!(scan().is_empty());
        assert!(running_executables().is_empty());
    }
}
