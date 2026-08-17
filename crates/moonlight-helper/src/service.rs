//! Service registration, and the control-manager entry point.

use std::ffi::OsString;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use windows_service::service::{
    ServiceAccess, ServiceControl, ServiceControlAccept, ServiceErrorControl, ServiceExitCode,
    ServiceInfo, ServiceStartType, ServiceState, ServiceStatus, ServiceType,
};
use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

use moonlight_core::helper::{validate, Request, Response, SERVICE_DISPLAY_NAME, SERVICE_NAME};

use crate::core_runner::Core;
use crate::pipe;

const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

windows_service::define_windows_service!(ffi_service_main, service_main);

pub fn run_as_service() -> Result<(), Box<dyn std::error::Error>> {
    windows_service::service_dispatcher::start(SERVICE_NAME, ffi_service_main)?;
    Ok(())
}

fn service_main(_arguments: Vec<OsString>) {
    if let Err(error) = run() {
        // There is no console to print to; the event log is what an
        // administrator will look at.
        eprintln!("moonlight-helper: {error}");
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let stop = Arc::new(AtomicBool::new(false));
    let core = Arc::new(Mutex::new(Core::new()));

    let status_handle = {
        let stop = Arc::clone(&stop);
        service_control_handler::register(SERVICE_NAME, move |control| match control {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                stop.store(true, Ordering::SeqCst);
                // Setting the flag is not enough. The serve loop spends its life
                // blocked inside `ConnectNamedPipe` waiting for a client, and
                // only looks at the flag between connections — so an idle
                // service ignored every stop request until something happened to
                // connect. `sc stop` reported success and the service kept
                // running; Restart Manager gave up on it during upgrades.
                //
                // Connecting to our own pipe is what unblocks the wait.
                crate::pipe::wake();
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        })?
    };

    let report = |state: ServiceState, accept: ServiceControlAccept| ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: state,
        controls_accepted: accept,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    };

    status_handle.set_service_status(report(
        ServiceState::Running,
        ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
    ))?;

    {
        let core = Arc::clone(&core);
        let stop_flag = Arc::clone(&stop);
        pipe::serve(
            move |request| dispatch(request, &core),
            move || stop_flag.load(Ordering::SeqCst),
        );
    }

    // Stopping the service must not leave a privileged core behind holding the
    // routes and the controller port.
    core.lock().expect("core mutex").stop();

    status_handle
        .set_service_status(report(ServiceState::Stopped, ServiceControlAccept::empty()))?;
    Ok(())
}

fn dispatch(request: &Request, core: &Arc<Mutex<Core>>) -> Response {
    // Validation happens here, on the privileged side. The client is not the
    // security boundary — anything that can open the pipe can send anything.
    if let Err(message) = validate(request) {
        return Response::Error { message };
    }

    let mut core = match core.lock() {
        Ok(core) => core,
        Err(poisoned) => poisoned.into_inner(),
    };

    match request {
        Request::Ping => Response::Pong {
            version: env!("CARGO_PKG_VERSION").to_string(),
        },
        Request::Status => Response::Status {
            running: core.is_running(),
        },
        Request::Stop => {
            core.stop();
            Response::Stopped
        }
        Request::Start { config } => match core.start(config) {
            Ok(()) => Response::Started,
            Err(message) => Response::Error { message },
        },
    }
}

/// Registers the service. Invoked through UAC by the app, once.
pub fn install() -> Result<(), Box<dyn std::error::Error>> {
    let manager = ServiceManager::local_computer(
        None::<&str>,
        ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
    )?;

    let info = ServiceInfo {
        name: OsString::from(SERVICE_NAME),
        display_name: OsString::from(SERVICE_DISPLAY_NAME),
        service_type: SERVICE_TYPE,
        // OnDemand, with the interactive user granted start/stop rights below.
        //
        // It used to be AutoStart, on the reasoning that an unelevated app
        // cannot start a stopped service. That is only true of the *default*
        // service DACL, and it bought a service that ran from boot to shutdown
        // whether or not anybody used TUN — and, worse, one the app could not
        // recover if it ever stopped. Granting start and stop instead means the
        // service runs while the app does and no longer.
        start_type: ServiceStartType::OnDemand,
        error_control: ServiceErrorControl::Normal,
        executable_path: std::env::current_exe()?,
        launch_arguments: vec![],
        dependencies: vec![],
        // LocalSystem: creating a Wintun adapter and installing routes are what
        // this exists for, and neither is available to a lesser account.
        account_name: None,
        account_password: None,
    };

    // Idempotent, because `--install` is run again by every upgrade and by the
    // app's own button. Creating a service that already exists fails with
    // ERROR_SERVICE_EXISTS, and the old code gave up there — before the line
    // that starts it. A second install therefore left the service registered,
    // configured, and stopped, with TUN failing at connect against a pipe that
    // was never listening. Auto-start meant it would come back at the next
    // reboot, which is not an answer anyone should have to find.
    let access = ServiceAccess::CHANGE_CONFIG | ServiceAccess::START | ServiceAccess::QUERY_STATUS;
    let service = match manager.create_service(&info, access) {
        Ok(service) => service,
        Err(_) => {
            let service = manager.open_service(SERVICE_NAME, access)?;
            // The path changes when the app is installed somewhere new, so the
            // existing registration is pointed at this binary rather than left
            // aimed at wherever the last one lived.
            service.change_config(&info)?;
            service
        }
    };

    service.set_description(
        "Runs the Moonlight VPN core in TUN mode. Removing this service disables \
         TUN mode; system-proxy mode keeps working without it.",
    )?;
    grant_user_control()?;
    stage_core()?;

    // Already running is the state being asked for, not a failure.
    if service.query_status()?.current_state != ServiceState::Running {
        service.start::<&str>(&[])?;
    }

    // Registered-but-not-running is the failure this whole function exists to
    // avoid, so it is confirmed rather than assumed. Starting is asynchronous:
    // the call returns as soon as the SCM has accepted it.
    for _ in 0..40 {
        if service.query_status()?.current_state == ServiceState::Running {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    Err("the helper service was registered but did not start".into())
}

/// Copies the core into the directory the service will run it from.
///
/// `core_runner::core_binary()` is a compile-time constant under
/// `%ProgramData%\Moonlight` precisely so a client cannot influence what gets
/// executed as LocalSystem — and the module comment has always said the install
/// puts the binary there. Nothing did. TUN therefore failed at every connect
/// with "the core is missing", pointing at a path only this function can fill.
///
/// The ACL is the other half, and is set here rather than inherited: a directory
/// under `%ProgramData%` that authenticated users can write to would let any
/// local account replace `mihomo.exe` and have the service run it as
/// LocalSystem. Inheritance is broken and only SYSTEM and Administrators are
/// granted.
fn stage_core() -> Result<(), Box<dyn std::error::Error>> {
    let source = std::env::current_exe()?
        .parent()
        .ok_or("the helper has no directory")?
        .to_path_buf();
    let target = std::path::PathBuf::from(moonlight_core::helper::INSTALL_ROOT);
    std::fs::create_dir_all(&target)?;

    let locked = std::process::Command::new("icacls.exe")
        .args([
            target.to_string_lossy().as_ref(),
            "/inheritance:r",
            "/grant:r",
            "*S-1-5-18:(OI)(CI)F", // SYSTEM
            "/grant:r",
            "*S-1-5-32-544:(OI)(CI)F", // Administrators
        ])
        .status()?;
    if !locked.success() {
        return Err("could not lock down the core directory".into());
    }

    // wintun.dll travels with it: mihomo loads it from beside its own binary,
    // and without it TUN fails at adapter creation with a message naming the DLL.
    for name in ["mihomo.exe", "wintun.dll"] {
        let from = source.join(name);
        if !from.is_file() {
            return Err(format!("{name} is not next to the helper").into());
        }
        std::fs::copy(&from, target.join(name))?;
    }

    // The geo databases, into the directory the core is run with as its `-d`.
    // Without them mihomo tries to fetch them itself while parsing the config,
    // through a resolver that does not work yet, and dies before binding its
    // API — which the app can only report as "the privileged core did not
    // answer". Missing here is not fatal: system-proxy mode is unaffected, and
    // the app keeps its own copy for that.
    let core_directory = target.join("core");
    std::fs::create_dir_all(&core_directory)?;
    for name in ["GeoSite.dat", "geoip.metadb"] {
        let from = source.join("geodata").join(name);
        if from.is_file() {
            std::fs::copy(&from, core_directory.join(name))?;
        }
    }
    Ok(())
}

/// Lets the signed-in user start and stop this service without elevation.
///
/// The default service DACL grants start only to administrators, which is what
/// forced the service to run from boot whether or not anybody wanted TUN. This
/// replaces it with the default entries plus one for `AU` (authenticated users)
/// carrying `RP` and `WP` — start and stop, and nothing else. No configuration
/// rights, no delete, no ability to repoint the binary: those stay with
/// administrators, which is the part that matters, since the service runs as
/// LocalSystem and its pipe is still open only to SYSTEM and Administrators.
fn grant_user_control() -> Result<(), Box<dyn std::error::Error>> {
    // Set with sc.exe rather than by hand: SetServiceObjectSecurity wants a
    // parsed self-relative descriptor, and this runs once at install where a
    // subprocess costs nothing.
    const SDDL: &str = concat!(
        "D:",
        "(A;;CCLCSWRPWPDTLOCRRC;;;SY)",   // SYSTEM: full control
        "(A;;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;BA)", // Administrators: everything
        "(A;;CCLCSWLOCRRC;;;IU)",         // Interactive users: query
        "(A;;RPWP;;;AU)",                 // Authenticated users: start and stop
    );

    let status = std::process::Command::new("sc.exe")
        .args(["sdset", SERVICE_NAME, SDDL])
        .status()?;
    if !status.success() {
        return Err("could not grant the signed-in user control of the service".into());
    }
    Ok(())
}

pub fn uninstall() -> Result<(), Box<dyn std::error::Error>> {
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
    let service = manager.open_service(
        SERVICE_NAME,
        ServiceAccess::STOP | ServiceAccess::DELETE | ServiceAccess::QUERY_STATUS,
    )?;

    if service.query_status()?.current_state != ServiceState::Stopped {
        service.stop()?;
        // The delete does not take effect until every handle closes and the
        // service actually stops; giving it a moment avoids leaving a
        // marked-for-deletion service that blocks a reinstall until reboot.
        for _ in 0..20 {
            std::thread::sleep(Duration::from_millis(250));
            if service.query_status()?.current_state == ServiceState::Stopped {
                break;
            }
        }
    }
    service.delete()?;
    Ok(())
}
