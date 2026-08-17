//! The protocol the app speaks to the privileged helper, and the client half of
//! it.
//!
//! TUN mode needs Administrator: creating the Wintun adapter and installing the
//! routes `auto-route` wants are both privileged operations, and no amount of
//! manifest work changes that. Asking for a UAC prompt on every connect is how
//! people end up leaving TUN off, so the privilege is taken once — a Windows
//! service installed with a single elevation — and the app talks to it over a
//! named pipe for the rest of the install's life.
//!
//! ## The helper's trust boundary
//!
//! A LocalSystem service taking instructions over a pipe is a privilege
//! escalation waiting to happen, so it is deliberately narrow. These three rules
//! are the whole of it, and they are the same three the macOS client's
//! LaunchDaemon follows:
//!
//! - **It never runs a path the client supplies.** The core binary is a copy
//!   made into the service's own directory at install time, and that path is
//!   compiled into the service. There is no field in this protocol for naming
//!   another one — see [`Request`], which carries no path at all.
//! - **It never opens a config path the client supplies.** The client sends
//!   config *text*; the service writes it into its own directory under
//!   `%ProgramData%`. Otherwise a symlink or a junction planted in that
//!   directory would let any local user have LocalSystem read a file they
//!   cannot.
//! - **The pipe's DACL grants only Administrators and SYSTEM.** Callers are
//!   exactly the accounts that can already elevate. This spares the user a
//!   prompt per connect; it is not a boundary against an administrator, and it
//!   is not meant to be.
//!
//! The reason the second rule matters more than it looks: the service runs as
//! LocalSystem, which can read every file on the machine. A protocol that took
//! a path would turn "start the tunnel" into "read this file as SYSTEM" for any
//! user in the Administrators group — which is a real escalation on a machine
//! where an admin account is not meant to be able to read another user's
//! profile.

use serde::{Deserialize, Serialize};

/// The service's registered name, and the pipe it listens on.
pub const SERVICE_NAME: &str = "MoonlightHelper";
pub const SERVICE_DISPLAY_NAME: &str = "Moonlight VPN Helper";
pub const PIPE_NAME: &str = r"\\.\pipe\moonlight-helper";

/// Where the service keeps the things only it may write: its own copy of the
/// core, and the config text the client sends it.
pub const INSTALL_ROOT: &str = r"C:\ProgramData\Moonlight";

/// What the app can ask the helper to do.
///
/// Note what is **not** here: no binary path, no config path, no working
/// directory, no arguments. The only thing that crosses the boundary is config
/// text and a port number, and both are validated on the far side.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum Request {
    /// Is the service alive, and what version is it?
    Ping,
    /// Start the core in TUN mode with this config.
    ///
    /// `config` is the document itself, never a path to one.
    Start { config: String },
    /// Stop the core the service started.
    Stop,
    /// Is a privileged core running right now?
    Status,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "camelCase")]
pub enum Response {
    Pong { version: String },
    Started,
    Stopped,
    Status { running: bool },
    Error { message: String },
}

/// The largest config the helper will accept, in bytes.
///
/// A panel config with a thousand rules is comfortably under 1 MB. The cap
/// exists so an unauthenticated write to the pipe cannot make a LocalSystem
/// service allocate without bound.
pub const MAX_REQUEST_BYTES: usize = 4 * 1024 * 1024;

/// Rejects a request the service must not act on, before it acts on it.
///
/// Run on the **service** side. The client is not the security boundary — a
/// caller that can open the pipe can send whatever it likes — so validation
/// that matters happens where the privilege is.
pub fn validate(request: &Request) -> Result<(), String> {
    match request {
        Request::Start { config } => {
            if config.trim().is_empty() {
                return Err("Config is empty".to_string());
            }
            if config.len() > MAX_REQUEST_BYTES {
                return Err(format!(
                    "Config is {} bytes, over the {MAX_REQUEST_BYTES} limit",
                    config.len()
                ));
            }
            // It has to be a YAML mapping with proxies in it. A service that
            // will write any string to disk and hand it to a privileged process
            // is a more useful primitive to an attacker than one that will only
            // write a config.
            let parsed: serde_yaml::Value = serde_yaml::from_str(config)
                .map_err(|e| format!("Config is not valid YAML: {e}"))?;
            let Some(mapping) = parsed.as_mapping() else {
                return Err("Config is not a YAML mapping".to_string());
            };
            if !mapping.contains_key(serde_yaml::Value::from("proxies")) {
                return Err("Config carries no proxies".to_string());
            }
            Ok(())
        }
        Request::Ping | Request::Stop | Request::Status => Ok(()),
    }
}

/// One request and one response, newline-delimited JSON.
///
/// A line per message rather than a length prefix: the messages are small, the
/// transport is a local pipe, and a framing bug in a privileged service is
/// worth avoiding more than a few bytes are worth saving.
pub fn encode(request: &Request) -> String {
    let mut line = serde_json::to_string(request).expect("a Request always serialises");
    line.push('\n');
    line
}

pub fn decode_response(line: &str) -> Option<Response> {
    serde_json::from_str(line.trim()).ok()
}

pub fn decode_request(line: &str) -> Option<Request> {
    serde_json::from_str(line.trim()).ok()
}

pub fn encode_response(response: &Response) -> String {
    let mut line = serde_json::to_string(response).expect("a Response always serialises");
    line.push('\n');
    line
}

#[cfg(windows)]
pub use client::*;

#[cfg(windows)]
mod client {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::time::Duration;

    /// Whether the service is installed on this machine.
    pub fn is_installed() -> bool {
        status().is_some()
    }

    /// Whether the service is not merely registered but actually **running**.
    ///
    /// TUN depends on the pipe, and the pipe only exists while the service is
    /// up. A registered-but-stopped service passed `is_installed`, so the app
    /// offered TUN and then failed at connect with a bare `os error 2` from the
    /// pipe open — which names a missing file and explains nothing.
    pub fn is_running() -> bool {
        use windows::Win32::System::Services::SERVICE_RUNNING;
        status() == Some(SERVICE_RUNNING.0)
    }

    /// Starts the service and waits for it to be running.
    ///
    /// Unelevated: `--install` grants the interactive user start and stop rights
    /// on the service precisely so this needs no UAC prompt. Returns whether the
    /// service is running when it gives up waiting, so an already-running
    /// service is a success rather than an error.
    pub fn start() -> bool {
        use windows::core::HSTRING;
        use windows::Win32::System::Services::{
            CloseServiceHandle, OpenSCManagerW, OpenServiceW, StartServiceW, SC_MANAGER_CONNECT,
            SERVICE_QUERY_STATUS, SERVICE_START,
        };

        if is_running() {
            return true;
        }
        unsafe {
            let Ok(manager) = OpenSCManagerW(None, None, SC_MANAGER_CONNECT) else {
                return false;
            };
            let name = HSTRING::from(SERVICE_NAME);
            let opened = OpenServiceW(manager, &name, SERVICE_START | SERVICE_QUERY_STATUS);
            if let Ok(service) = opened {
                let _ = StartServiceW(service, None);
                let _ = CloseServiceHandle(service);
            }
            let _ = CloseServiceHandle(manager);
        }
        // Starting is asynchronous — the call returns once the SCM accepts it.
        for _ in 0..40 {
            if is_running() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        is_running()
    }

    /// Stops the service, so it is not left holding a privileged core after the
    /// app that asked for it has gone.
    pub fn stop() {
        use windows::core::HSTRING;
        use windows::Win32::System::Services::{
            CloseServiceHandle, ControlService, OpenSCManagerW, OpenServiceW, SERVICE_CONTROL_STOP,
            SERVICE_STATUS, SC_MANAGER_CONNECT, SERVICE_STOP,
        };

        unsafe {
            let Ok(manager) = OpenSCManagerW(None, None, SC_MANAGER_CONNECT) else {
                return;
            };
            let name = HSTRING::from(SERVICE_NAME);
            if let Ok(service) = OpenServiceW(manager, &name, SERVICE_STOP) {
                let mut status = SERVICE_STATUS::default();
                let _ = ControlService(service, SERVICE_CONTROL_STOP, &mut status);
                let _ = CloseServiceHandle(service);
            }
            let _ = CloseServiceHandle(manager);
        }
    }

    /// The service's current state, or `None` when it is not registered at all.
    fn status() -> Option<u32> {
        use windows::core::HSTRING;
        use windows::Win32::System::Services::{
            CloseServiceHandle, OpenSCManagerW, OpenServiceW, QueryServiceStatus, SERVICE_STATUS,
            SC_MANAGER_CONNECT, SERVICE_QUERY_STATUS,
        };

        unsafe {
            let manager = OpenSCManagerW(None, None, SC_MANAGER_CONNECT).ok()?;
            let name = HSTRING::from(SERVICE_NAME);
            let service = OpenServiceW(manager, &name, SERVICE_QUERY_STATUS);
            let state = match service {
                Ok(service) => {
                    let mut status = SERVICE_STATUS::default();
                    let state = QueryServiceStatus(service, &mut status)
                        .is_ok()
                        .then_some(status.dwCurrentState.0);
                    let _ = CloseServiceHandle(service);
                    state
                }
                Err(_) => None,
            };
            let _ = CloseServiceHandle(manager);
            state
        }
    }

    /// Sends one request and waits for the answer.
    ///
    /// Synchronous and short-lived: the pipe is opened per call rather than held
    /// open, so a service restart (or an install that happened after this
    /// process started) is picked up without the app having to notice.
    pub fn send(request: &Request, timeout: Duration) -> Result<Response, String> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            match std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(PIPE_NAME)
            {
                Ok(mut pipe) => {
                    pipe.write_all(encode(request).as_bytes())
                        .map_err(|e| format!("Could not send to the helper: {e}"))?;
                    pipe.flush().ok();

                    let mut line = String::new();
                    BufReader::new(&pipe)
                        .read_line(&mut line)
                        .map_err(|e| format!("The helper did not answer: {e}"))?;
                    return decode_response(&line)
                        .ok_or_else(|| "The helper sent a reply this app cannot read".to_string());
                }
                Err(error) => {
                    // ERROR_PIPE_BUSY means the service is up but serving
                    // another caller; anything else means it is not listening.
                    if std::time::Instant::now() >= deadline {
                        return Err(format!("The Moonlight helper is not running: {error}"));
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        }
    }
}

#[cfg(not(windows))]
mod client {
    use super::*;
    use std::time::Duration;

    pub fn is_installed() -> bool {
        false
    }

    pub fn is_running() -> bool {
        false
    }

    pub fn start() -> bool {
        false
    }

    pub fn stop() {}

    pub fn send(_request: &Request, _timeout: Duration) -> Result<Response, String> {
        Err("The privileged helper exists only on Windows".to_string())
    }
}

#[cfg(not(windows))]
pub use client::{is_installed, send};

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = "proxies:\n  - name: A\n    type: vless\n";

    #[test]
    fn the_protocol_carries_no_path_for_the_helper_to_open() {
        // This is the trust boundary written as a type. If a path ever appears
        // in Request, a caller who can open the pipe can make a LocalSystem
        // service read any file on the machine.
        let encoded = encode(&Request::Start {
            config: GOOD.to_string(),
        });
        assert!(encoded.contains("\"config\""));
        assert!(!encoded.contains("path"));
        assert!(!encoded.contains("binary"));
    }

    #[test]
    fn requests_and_responses_round_trip() {
        for request in [
            Request::Ping,
            Request::Stop,
            Request::Status,
            Request::Start {
                config: GOOD.to_string(),
            },
        ] {
            let line = encode(&request);
            assert!(line.ends_with('\n'), "messages are newline-framed");
            assert_eq!(decode_request(&line), Some(request));
        }

        for response in [
            Response::Pong {
                version: "1.0.0".into(),
            },
            Response::Started,
            Response::Stopped,
            Response::Status { running: true },
            Response::Error {
                message: "no".into(),
            },
        ] {
            let line = encode_response(&response);
            assert_eq!(decode_response(&line), Some(response));
        }
    }

    #[test]
    fn a_message_with_no_newline_still_decodes() {
        // A short read that lost the terminator must not be treated as a
        // different message.
        let line = encode(&Request::Ping);
        assert_eq!(decode_request(line.trim()), Some(Request::Ping));
    }

    #[test]
    fn junk_on_the_pipe_decodes_to_nothing_rather_than_a_default() {
        assert_eq!(decode_request("not json"), None);
        assert_eq!(decode_request(""), None);
        assert_eq!(decode_response("{}"), None);
        // An unknown op must not fall through to a known one.
        assert_eq!(decode_request(r#"{"op":"formatDisk"}"#), None);
    }

    #[test]
    fn a_valid_config_is_accepted() {
        assert_eq!(
            validate(&Request::Start {
                config: GOOD.to_string()
            }),
            Ok(())
        );
    }

    #[test]
    fn an_empty_config_is_refused() {
        assert!(validate(&Request::Start {
            config: "   \n ".to_string()
        })
        .is_err());
    }

    #[test]
    fn a_config_that_is_not_yaml_is_refused() {
        // A service that will write any string to disk and hand it to a
        // privileged process is a more useful primitive than one that will only
        // write a config.
        assert!(validate(&Request::Start {
            config: "\t\x00not yaml: [".to_string()
        })
        .is_err());
    }

    #[test]
    fn a_yaml_document_that_is_not_a_config_is_refused() {
        assert!(validate(&Request::Start {
            config: "- a\n- b\n".to_string()
        })
        .is_err());
        assert!(validate(&Request::Start {
            config: "log-level: debug\n".to_string()
        })
        .is_err());
    }

    #[test]
    fn an_oversized_config_is_refused_before_it_is_parsed() {
        let huge = format!("proxies:\n{}", "  - name: A\n".repeat(500_000));
        assert!(huge.len() > MAX_REQUEST_BYTES);
        let error = validate(&Request::Start { config: huge }).expect_err("refused");
        assert!(error.contains("limit"));
    }

    #[test]
    fn the_other_operations_need_no_validation() {
        for request in [Request::Ping, Request::Stop, Request::Status] {
            assert_eq!(validate(&request), Ok(()));
        }
    }

    #[test]
    fn the_install_root_is_under_program_data_not_the_user_profile() {
        // It must be a directory no unprivileged account can write, or the
        // compiled-in core path stops meaning anything.
        assert!(INSTALL_ROOT.starts_with(r"C:\ProgramData"));
    }
}
