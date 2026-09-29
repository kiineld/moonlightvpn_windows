//! Runs the core, as LocalSystem, from a path the client cannot influence.
//!
//! Both halves of the trust boundary live here:
//!
//! - [`CORE_BINARY`] is a constant. The install copies mihomo into
//!   `%ProgramData%\Moonlight`, whose ACL only Administrators and SYSTEM can
//!   write, and this is the only path the service will ever execute.
//! - [`write_config`] writes the *text* the client sent into that same
//!   directory, and it removes any existing entry at that name first. Without
//!   the removal, a junction or a symlink planted at `core.yaml` would redirect
//!   a LocalSystem write anywhere on the disk — the classic way a privileged
//!   service is turned into an arbitrary-file-write primitive.

use std::path::PathBuf;
use std::process::{Child, Command};

use moonlight_core::helper::INSTALL_ROOT;

/// Compiled in. There is no code path that executes anything else.
pub fn core_binary() -> PathBuf {
    PathBuf::from(INSTALL_ROOT).join("mihomo.exe")
}

/// The service's own config file. The client never names this.
///
/// Inside the data directory, not beside it: mihomo rejects a `-f` outside the
/// `-d` it was handed, with "not in SAFE_PATHS".
pub fn config_path() -> PathBuf {
    data_directory().join("core.yaml")
}

pub fn data_directory() -> PathBuf {
    PathBuf::from(INSTALL_ROOT).join("core")
}

/// Writes the client's config text into the service's own directory.
///
/// The unlink before the write is the load-bearing part: `File::create` on an
/// existing symlink or junction follows it, so an attacker who can create a
/// name in the target directory could otherwise redirect a LocalSystem write to
/// any path on the machine. Removing the entry first means the create always
/// makes a fresh file in the directory the service controls.
pub fn write_config(config: &str) -> std::io::Result<PathBuf> {
    let directory = PathBuf::from(INSTALL_ROOT);
    std::fs::create_dir_all(&directory)?;
    std::fs::create_dir_all(data_directory())?;

    let path = config_path();
    match std::fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.is_dir() => {
            // A directory here is either a junction or someone being awkward.
            std::fs::remove_dir_all(&path)?;
        }
        Ok(_) => std::fs::remove_file(&path)?,
        Err(_) => {}
    }

    std::fs::write(&path, config)?;
    Ok(path)
}

pub struct Core {
    child: Option<Child>,
}

impl Core {
    pub fn new() -> Self {
        Core { child: None }
    }

    pub fn is_running(&mut self) -> bool {
        match &mut self.child {
            None => false,
            Some(child) => matches!(child.try_wait(), Ok(None)),
        }
    }

    pub fn start(&mut self, config: &str) -> Result<(), String> {
        self.stop();
        refresh_core();

        let binary = core_binary();
        if !binary.is_file() {
            return Err(format!(
                "The core is missing from {}. Reinstall the helper.",
                binary.display()
            ));
        }
        let path = write_config(config).map_err(|e| format!("Could not write the config: {e}"))?;

        let mut command = Command::new(&binary);
        command.arg("-d").arg(data_directory()).arg("-f").arg(&path);

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }

        let child = command
            .spawn()
            .map_err(|e| format!("Could not start the core: {e}"))?;
        self.child = Some(child);
        Ok(())
    }

    pub fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Brings the staged core level with the one shipped beside this helper.
///
/// Only `--install` used to copy it, and only the installer runs that. An update
/// from the zip replaced the app, the helper and the core beside them, and left
/// TUN running the old core from here — on 1.19.29, every probe to the service's
/// LTE balancers fails. Copying from the helper's own directory adds no trust:
/// whoever can write there can already replace the service binary itself.
///
/// Called with the core stopped, so neither file is held open.
fn refresh_core() {
    let Some(source) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(PathBuf::from))
    else {
        return;
    };
    for name in ["mihomo.exe", "wintun.dll"] {
        // A development build has no core beside it; nothing to compare with.
        let Ok(shipped) = std::fs::read(source.join(name)) else {
            continue;
        };
        let staged = PathBuf::from(INSTALL_ROOT).join(name);
        if std::fs::read(&staged).ok().as_deref() != Some(shipped.as_slice()) {
            let _ = std::fs::write(&staged, &shipped);
        }
    }
}

impl Drop for Core {
    fn drop(&mut self) {
        // The service stopping must not leave a privileged core behind holding
        // the routes and the controller port.
        self.stop();
    }
}
