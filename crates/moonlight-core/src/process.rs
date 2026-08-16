//! Supervises the bundled mihomo core as a child process.
//!
//! In system-proxy mode the core runs as the user, which needs no privileges at
//! all. TUN mode needs Administrator to create the Wintun adapter and install
//! routes, so there the core is started by the privileged service instead — see
//! [`crate::helper`]. Both paths produce the same running core reachable on the
//! same loopback controller, which is why everything above this type is
//! indifferent to which one started it.
//!
//! ## Two Windows specifics
//!
//! **No console window.** mihomo is a console application, so spawning it
//! normally pops a black `conhost` window in front of the user and leaves it in
//! the taskbar for the life of the tunnel. `CREATE_NO_WINDOW` is what makes the
//! core invisible.
//!
//! **A job object, so the core cannot outlive the app.** On macOS a child dies
//! with its parent's process group. Windows has no such relationship: if the
//! app crashes or is killed from Task Manager, the core keeps running, keeps
//! holding the controller port, and keeps carrying traffic with no UI attached.
//! The next launch then finds a core it did not start. Putting the child in a
//! job with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` makes the kernel terminate it
//! when the last handle to the job closes, which happens when this process
//! exits for any reason — including a crash.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};

use thiserror::Error;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

/// The tail of the core's own log, kept so a failed start can say why.
/// Bounded, because a core in a crash loop would otherwise grow it without
/// limit for the lifetime of the app.
const LOG_LIMIT: usize = 200;

#[derive(Debug, Error)]
pub enum Failure {
    #[error("The mihomo core is missing from the installation")]
    BinaryMissing,
    #[error("Core exited with status {code}{}", if .output.is_empty() { String::new() } else { format!("\n{}", .output) })]
    Exited { code: i32, output: String },
    #[error("Could not start the core: {0}")]
    Spawn(String),
}

pub struct MihomoProcess {
    binary: PathBuf,
    data_directory: PathBuf,
    child: Option<Child>,
    log: Arc<Mutex<Vec<String>>>,
}

impl MihomoProcess {
    pub fn new(binary: impl Into<PathBuf>, data_directory: impl Into<PathBuf>) -> Self {
        MihomoProcess {
            binary: binary.into(),
            data_directory: data_directory.into(),
            child: None,
            log: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn is_running(&mut self) -> bool {
        match &mut self.child {
            None => false,
            Some(child) => matches!(child.try_wait(), Ok(None)),
        }
    }

    /// Everything the core has written so far, newest last.
    pub fn log(&self) -> String {
        self.log.lock().expect("log mutex").join("\n")
    }

    /// Starts the core against `config_path` and streams its output into the
    /// bounded log. `lines` receives every line as it arrives, for the logs
    /// screen.
    pub async fn start(
        &mut self,
        config_path: &Path,
        lines: Option<mpsc::UnboundedSender<String>>,
    ) -> Result<(), Failure> {
        if !self.binary.is_file() {
            return Err(Failure::BinaryMissing);
        }
        self.log.lock().expect("log mutex").clear();

        let mut command = Command::new(&self.binary);
        command
            .arg("-d")
            .arg(&self.data_directory)
            .arg("-f")
            .arg(config_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        #[cfg(windows)]
        {
            // Without this the user gets a console window in front of whatever
            // they were doing, and an entry in the taskbar for the life of the
            // tunnel.
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = command.spawn().map_err(|e| Failure::Spawn(e.to_string()))?;

        #[cfg(windows)]
        job::assign_current(&child);

        for stream in [
            child.stdout.take().map(Streams::Out),
            child.stderr.take().map(Streams::Err),
        ]
        .into_iter()
        .flatten()
        {
            let log = Arc::clone(&self.log);
            let lines = lines.clone();
            tokio::spawn(async move {
                let mut reader = match stream {
                    Streams::Out(out) => BufReader::new(Box::pin(out) as Pinned).lines(),
                    Streams::Err(err) => BufReader::new(Box::pin(err) as Pinned).lines(),
                };
                while let Ok(Some(line)) = reader.next_line().await {
                    {
                        let mut log = log.lock().expect("log mutex");
                        log.push(line.clone());
                        if log.len() > LOG_LIMIT {
                            let excess = log.len() - LOG_LIMIT;
                            log.drain(..excess);
                        }
                    }
                    if let Some(lines) = &lines {
                        let _ = lines.send(line);
                    }
                }
            });
        }

        self.child = Some(child);
        Ok(())
    }

    pub async fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
    }

    /// The reason TUN failed to come up, if it did.
    ///
    /// This has to be asked for explicitly, because the failure is **not** a
    /// crash: the core keeps running and keeps answering its API with the
    /// interface never established, so every other signal says "connected"
    /// while no traffic moves.
    pub fn tun_failure(log: &str) -> Option<String> {
        let line = log
            .lines()
            .rfind(|l| l.contains("Start TUN listening error"))?;

        // `add route: …: file exists` means another VPN client already owns the
        // routes auto-route wants. That is the common case by far, and the raw
        // message sends people looking for a bug in this app.
        if line.contains("file exists") || line.contains("add route") {
            return Some(
                "Another VPN client already holds the system routes. \
                 Disconnect it and try again."
                    .to_string(),
            );
        }
        // The Windows-specific one: no adapter to drive.
        if line.contains("wintun") || line.contains("Wintun") {
            return Some(
                "The Wintun adapter could not be created. \
                 Check that the app is running as Administrator."
                    .to_string(),
            );
        }
        Some(line.trim().to_string())
    }
}

type Pinned = std::pin::Pin<Box<dyn tokio::io::AsyncRead + Send>>;

enum Streams {
    Out(tokio::process::ChildStdout),
    Err(tokio::process::ChildStderr),
}

#[cfg(windows)]
mod job {
    //! A job object that kills the core when this process goes away, however it
    //! goes away.

    use std::sync::OnceLock;

    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    /// Created once and deliberately never closed: the handle staying open for
    /// the life of the process is precisely what keeps the job alive, and the
    /// kernel closes it when the process ends — including on a crash, which is
    /// the case this exists for.
    static JOB: OnceLock<usize> = OnceLock::new();

    fn handle() -> Option<HANDLE> {
        let raw = JOB.get_or_init(|| unsafe {
            let Ok(job) = CreateJobObjectW(None, None) else {
                return 0;
            };
            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let ok = SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const core::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
            .is_ok();
            if ok {
                job.0 as usize
            } else {
                0
            }
        });
        (*raw != 0).then(|| HANDLE(*raw as *mut core::ffi::c_void))
    }

    pub fn assign_current(child: &tokio::process::Child) {
        let Some(job) = handle() else { return };
        let Some(raw) = child.raw_handle() else {
            return;
        };
        unsafe {
            // A failure here is not fatal: the core still runs, it just outlives
            // a crashed app, which the orphan sweep at launch then cleans up.
            let _ = AssignProcessToJobObject(job, HANDLE(raw as *mut core::ffi::c_void));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clean_log_reports_no_tun_failure() {
        assert_eq!(MihomoProcess::tun_failure(""), None);
        assert_eq!(
            MihomoProcess::tun_failure("INFO Start initial provider\nINFO RESTful API listening"),
            None
        );
    }

    #[test]
    fn a_route_collision_names_the_cause_rather_than_quoting_the_core() {
        let log = "ERRO Start TUN listening error: configure tun interface: \
                   add route: 1.0.0.0/8: file exists";
        let failure = MihomoProcess::tun_failure(log).expect("detected");
        assert!(failure.contains("Another VPN client"));
        // The raw message sends people looking for a bug in this app.
        assert!(!failure.contains("1.0.0.0/8"));
    }

    #[test]
    fn a_missing_adapter_points_at_the_privilege_it_needs() {
        let log = "ERRO Start TUN listening error: wintun: failed to create adapter";
        let failure = MihomoProcess::tun_failure(log).expect("detected");
        assert!(failure.contains("Administrator"));
    }

    #[test]
    fn an_unrecognised_tun_error_is_still_surfaced_verbatim() {
        // Better an unfamiliar message than a tunnel that silently routes
        // nothing while reporting success.
        let log = "ERRO Start TUN listening error: something entirely new";
        let failure = MihomoProcess::tun_failure(log).expect("detected");
        assert!(failure.contains("something entirely new"));
    }

    #[test]
    fn the_last_tun_error_wins() {
        // A reconnect appends; the current attempt is the one that matters.
        let log = "ERRO Start TUN listening error: add route: file exists\n\
                   INFO retrying\n\
                   ERRO Start TUN listening error: wintun: no adapter";
        let failure = MihomoProcess::tun_failure(log).expect("detected");
        assert!(failure.contains("Administrator"));
    }

    #[tokio::test]
    async fn a_missing_binary_is_refused_before_spawning() {
        let mut process = MihomoProcess::new("/nonexistent/mihomo.exe", "/tmp");
        let error = process
            .start(Path::new("/tmp/config.yaml"), None)
            .await
            .expect_err("must not start");
        assert!(matches!(error, Failure::BinaryMissing));
        assert!(!process.is_running());
    }

    #[test]
    fn the_log_is_bounded_so_a_crash_loop_cannot_grow_it_without_limit() {
        let process = MihomoProcess::new("/x", "/y");
        {
            let mut log = process.log.lock().unwrap();
            for i in 0..(LOG_LIMIT * 3) {
                log.push(format!("line {i}"));
                if log.len() > LOG_LIMIT {
                    let excess = log.len() - LOG_LIMIT;
                    log.drain(..excess);
                }
            }
        }
        assert_eq!(process.log.lock().unwrap().len(), LOG_LIMIT);
        // And it keeps the newest lines, which are the ones that say why.
        assert!(process
            .log()
            .ends_with(&format!("line {}", LOG_LIMIT * 3 - 1)));
    }
}
