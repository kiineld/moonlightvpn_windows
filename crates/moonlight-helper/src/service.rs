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
        // AutoStart, not OnDemand: the app is not elevated, so it cannot start
        // a stopped service. If this were on-demand, TUN would work exactly
        // once per reboot — until the first stop — and then silently stop
        // working with no way for the app to recover without another prompt.
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: std::env::current_exe()?,
        launch_arguments: vec![],
        dependencies: vec![],
        // LocalSystem: creating a Wintun adapter and installing routes are what
        // this exists for, and neither is available to a lesser account.
        account_name: None,
        account_password: None,
    };

    let service =
        manager.create_service(&info, ServiceAccess::CHANGE_CONFIG | ServiceAccess::START)?;
    service.set_description(
        "Runs the Moonlight VPN core in TUN mode. Removing this service disables \
         TUN mode; system-proxy mode keeps working without it.",
    )?;
    service.start::<&str>(&[])?;
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
