//! The privileged Windows service that runs the mihomo core in TUN mode.
//!
//! It exists for one reason: creating a Wintun adapter and installing the routes
//! `auto-route` wants both need Administrator, and asking for a UAC prompt on
//! every connect is how people end up leaving TUN off. The privilege is taken
//! once, at install, and this service holds it.
//!
//! Everything it will do is in [`moonlight_core::helper::Request`], which is
//! four operations and carries no path of any kind. See that module for why the
//! boundary is drawn exactly there.
//!
//! Run without arguments it expects to be started by the service control
//! manager. `--install` and `--uninstall` are the elevated entry points the app
//! invokes through UAC.

#[cfg(not(windows))]
fn main() {
    eprintln!(
        "moonlight-helper is a Windows service and has nothing to do on this platform.\n\
         It is built here only so the workspace type-checks away from Windows."
    );
    std::process::exit(1);
}

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mode = std::env::args().nth(1).unwrap_or_default();
    match mode.as_str() {
        "--install" => service::install(),
        "--uninstall" => service::uninstall(),
        _ => service::run_as_service(),
    }
}

#[cfg(windows)]
mod core_runner;
#[cfg(windows)]
mod pipe;
#[cfg(windows)]
mod service;
