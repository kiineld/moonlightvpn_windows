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
pub fn config_path() -> PathBuf {
    PathBuf::from(INSTALL_ROOT).join("core.yaml")
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

impl Drop for Core {
    fn drop(&mut self) {
        // The service stopping must not leave a privileged core behind holding
        // the routes and the controller port.
        self.stop();
    }
}
