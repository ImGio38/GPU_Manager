use std::sync::{Arc, Mutex};
use log::{info, warn, error};
use tokio::signal::unix::{signal, SignalKind};

use hw_control_core::{
    Config, scan_hwmon_devices, discover_controls,
    disable_manual_control
};

mod scheduler;
mod socket_server;

use scheduler::{DaemonState, start_scheduler};
use socket_server::{start_socket_server, cleanup_socket};

extern "C" {
    fn getuid() -> u32;
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    info!("Starting Hardware Control Daemon...");

    // 2. Check for root privileges
    let uid = unsafe { getuid() };
    if uid != 0 {
        error!("This daemon must be run as root (current UID is {}). Exiting.", uid);
        std::process::exit(1);
    }

    // 3. Load configuration
    let config = Config::load();
    let current_gpu_mode = config.gpu.default_mode;

    // 4. Discover cooling hardware
    let hwmon_devices = scan_hwmon_devices();
    let controls = discover_controls(&hwmon_devices);
    
    if controls.cpu_temp.is_none() {
        warn!("No CPU temperature sensor detected. Temperature-based controls will be disabled.");
    }
    if controls.cpu_fan.is_none() {
        warn!("No CPU fan PWM controller detected.");
    }

    // 5. Initialize shared state
    let state = Arc::new(Mutex::new(DaemonState {
        config,
        controls: controls.clone(),
        current_gpu_mode,
        cpu_fan_speed: None,
        gpu_fan_speed: None,
        cpu_temp: None,
        gpu_temp: None,
    }));

    // 6. Start the scheduler thread
    let _scheduler_handle = start_scheduler(Arc::clone(&state));

    // 7. Setup clean shutdown handler
    let state_cleanup = Arc::clone(&state);
    tokio::spawn(async move {
        let mut sigterm = match signal(SignalKind::terminate()) {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to register SIGTERM handler: {}", e);
                return;
            }
        };

        tokio::select! {
            _ = tokio::signal::ctrl_c() => info!("Shutdown signal (Ctrl+C) received."),
            _ = sigterm.recv() => info!("Shutdown signal (SIGTERM) received."),
        }
        
        info!("Performing daemon cleanup...");
        cleanup_socket();

        // Restore automatic BIOS fan control
        let lock = state_cleanup.lock().unwrap();
        if let Some(ref fan) = lock.controls.cpu_fan {
            if let Err(e) = disable_manual_control(fan) {
                warn!("Failed to restore CPU fan auto control: {}", e);
            } else {
                info!("Restored CPU fan to automatic BIOS control.");
            }
        }
        if let Some(ref fan) = lock.controls.gpu_fan {
            if let Err(e) = disable_manual_control(fan) {
                warn!("Failed to restore GPU fan auto control: {}", e);
            } else {
                info!("Restored GPU fan to automatic BIOS control.");
            }
        }
        
        info!("Daemon shutdown complete.");
        std::process::exit(0);
    });

    // 8. Start socket server (runs until shutdown)
    if let Err(e) = start_socket_server(Arc::clone(&state)).await {
        error!("Fatal error in socket server: {}", e);
        cleanup_socket();
        return Err(e);
    }

    Ok(())
}
