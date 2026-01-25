#![cfg(windows)]

use std::{ffi::OsString, sync::mpsc, time::Duration};
use std::process::Command;

use tracing::trace;
use windows_service::{
    define_windows_service,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode,
        ServiceState, ServiceStatus, ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
};

use crate::init_server;

define_windows_service!(ffi_service_main, service_main);

pub fn run() -> anyhow::Result<()> {
    windows_service::service_dispatcher::start(
        "WebPushService",
        ffi_service_main,
    )?;
    Ok(())
}

fn service_main(_args: Vec<OsString>) {
    if let Err(e) = service_main_inner() {
        eprintln!("Service error: {:?}", e);
    }
}

fn service_main_inner() -> anyhow::Result<()> {
    let (router, addr) = init_server();

    trace!("Service main started");
    let (stop_tx, stop_rx_worker) = mpsc::channel();
    let (stop_tx_main, stop_rx_main) = mpsc::channel();
    let (ready_tx, ready_rx) = mpsc::channel();

    let handler = move |control| -> ServiceControlHandlerResult {
        trace!("Service control received: {:?}", control);
        match control {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                let _ = stop_tx.send(());
                let _ = stop_tx_main.send(());
                ServiceControlHandlerResult::NoError
            }
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = service_control_handler::register(
        "WebPushService",
        handler,
    )?;

    trace!("Service registered with SCM");
    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::StartPending,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 1,
        wait_hint: Duration::from_secs(10),
        process_id: None,
    })?;


    trace!("Starting Tokio runtime in separate thread");
    std::thread::spawn(move || {
        let _ = ready_tx.send(()); //avisar que ya se esta ejecutando tokio
        trace!("Tokio runtime started");
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            tokio::select! {
                res = crate::run_server(router, addr) => {
                    if let Err(e) = res {
                        eprintln!("Server exited: {:?}", e);
                    }
                }
                _ = tokio::task::spawn_blocking(move || stop_rx_worker.recv()) => {
                    trace!("Stop signal received in Tokio runtime");
                    // graceful stop
                }
            }
        });
    });

    // Esperara a que inicie tokio
    trace!("Waiting for Tokio runtime to be ready");
    let _ = ready_rx.recv();

    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::ZERO,
        process_id: None,
    })?;

    // BLOCK service_main thread until stop is requested
    trace!("Service is running. Waiting for stop signal.");
    let _ = stop_rx_main.recv();

    //Report STOPPED before exiting
    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::ZERO,
        process_id: None,
    })?;

    trace!("Service stopped.");
    Ok(())
}




const SERVICE_NAME: &str = "WebPushService";
const DISPLAY_NAME: &str = "Web Push Backend Service";

pub fn install() -> anyhow::Result<()> {
    let exe = std::env::current_exe()?;

    let status = std::process::Command::new("sc.exe")
        .args([
            "create"      , SERVICE_NAME,
            "binPath="    , &format!("{}", exe.display()),
            "start="      , "auto",
            "DisplayName=", DISPLAY_NAME,
        ])
        .status()?;

    anyhow::ensure!(status.success(), "service installation failed");

    Ok(())
}


pub fn uninstall() -> anyhow::Result<()> {
    let status = Command::new("sc.exe")
        .args(["delete", SERVICE_NAME])
        .status()?;

    anyhow::ensure!(status.success(), "service removal failed");

    Ok(())
}
