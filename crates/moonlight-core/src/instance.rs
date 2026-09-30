//! One copy of the app at a time, and a way for a second one to hand over.
//!
//! A second copy used to start its own controller, find the first one's core on
//! the controller port, treat it as a leftover and stop it — taking down a
//! tunnel the user was using. Now the first copy holds a named mutex for its
//! lifetime; a second one sees it, passes its request (bring the window back,
//! or a `moonlight://` link) down a pipe to the first, and exits before it has
//! touched anything.
//!
//! The pipe is created by the first copy with the default security, which lets
//! only this user, administrators and SYSTEM write to it.

/// What a second launch asks of the first: its first argument, or `show`.
pub fn request_from_args(mut args: impl Iterator<Item = String>) -> String {
    args.nth(1)
        .filter(|arg| !arg.starts_with("--"))
        .unwrap_or_else(|| "show".to_string())
}

#[cfg(windows)]
pub use imp::{claim, forward, serve};

#[cfg(not(windows))]
pub fn claim() -> bool {
    true
}
#[cfg(not(windows))]
pub fn forward(_request: &str) -> bool {
    false
}
#[cfg(not(windows))]
pub async fn serve(_sink: tokio::sync::mpsc::UnboundedSender<String>) {}

#[cfg(windows)]
mod imp {
    use std::io::Write;
    use tokio::io::AsyncBufReadExt;
    use tokio::net::windows::named_pipe::ServerOptions;
    use tokio::sync::mpsc::UnboundedSender;
    use windows::core::HSTRING;
    use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows::Win32::System::Threading::CreateMutexW;

    /// Per user, so two people signed in to one machine each get their own.
    fn pipe_name() -> String {
        let user: String = std::env::var("USERNAME")
            .unwrap_or_default()
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .collect();
        format!(r"\\.\pipe\moonlight-app-{user}")
    }

    /// Whether this is the only copy running in the session. The mutex is
    /// held until the process exits: its handle is never closed.
    pub fn claim() -> bool {
        let name = HSTRING::from(r"Local\moonlight-vpn-app");
        match unsafe { CreateMutexW(None, false, &name) } {
            Ok(_handle) => (unsafe { GetLastError() }) != ERROR_ALREADY_EXISTS,
            // Could not even ask: better two copies than none.
            Err(_) => true,
        }
    }

    /// Hands `request` to the copy already running. Retries briefly, since
    /// that copy may be between two pipe instances or still starting.
    pub fn forward(request: &str) -> bool {
        for _ in 0..20 {
            if let Ok(mut pipe) = std::fs::OpenOptions::new().write(true).open(pipe_name()) {
                return writeln!(pipe, "{request}").is_ok();
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        false
    }

    /// Reads requests from later copies, one line per connection, forever.
    pub async fn serve(sink: UnboundedSender<String>) {
        let name = pipe_name();
        // `first_pipe_instance` refuses to share the name with a pipe someone
        // else made first, so another process cannot sit in front of this one.
        let Ok(mut server) = ServerOptions::new().first_pipe_instance(true).create(&name) else {
            return;
        };
        loop {
            if server.connect().await.is_err() {
                continue;
            }
            let connected = server;
            server = match ServerOptions::new().create(&name) {
                Ok(next) => next,
                Err(_) => return,
            };
            let sink = sink.clone();
            tokio::spawn(async move {
                let mut line = String::new();
                let mut reader = tokio::io::BufReader::new(connected);
                // A request is one short line; anything longer is not one.
                if reader.read_line(&mut line).await.is_ok() && line.len() < 8192 {
                    let _ = sink.send(line.trim().to_string());
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::request_from_args;

    fn args(list: &[&str]) -> impl Iterator<Item = String> {
        list.iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .into_iter()
    }

    #[test]
    fn a_bare_launch_asks_for_the_window() {
        assert_eq!(request_from_args(args(&["moonlight.exe"])), "show");
        assert_eq!(
            request_from_args(args(&["moonlight.exe", "--autostart"])),
            "show"
        );
    }

    #[test]
    fn a_link_is_passed_on_as_it_came() {
        assert_eq!(
            request_from_args(args(&["moonlight.exe", "moonlight://import?url=x"])),
            "moonlight://import?url=x"
        );
    }
}
