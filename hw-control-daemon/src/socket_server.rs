use std::sync::{Arc, Mutex};
use std::path::Path;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use tokio::net::UnixListener;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use log::{info, warn, error, debug};
use hw_control_core::{
    IpcRequest, IpcResponse, SystemStatus,
    switch_gpu_mode, DEFAULT_CONFIG_PATH
};
use crate::scheduler::DaemonState;

pub const SOCKET_PATH: &str = "/run/hw-control.sock";

pub async fn start_socket_server(state: Arc<Mutex<DaemonState>>) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Remove existing socket if present
    cleanup_socket();

    // 2. Bind to UDS
    info!("Binding Unix Domain Socket to {}", SOCKET_PATH);
    let listener = UnixListener::bind(SOCKET_PATH)?;

    // 3. Set file permissions so standard users can write to the socket
    if let Ok(metadata) = fs::metadata(SOCKET_PATH) {
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o666);
        if let Err(e) = fs::set_permissions(SOCKET_PATH, permissions) {
            warn!("Failed to set socket permissions to 0666: {}", e);
        } else {
            debug!("Set socket permissions to 0666");
        }
    }

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let state_clone = Arc::clone(&state);
                tokio::spawn(async move {
                    if let Err(e) = handle_client(stream, state_clone).await {
                        debug!("Error handling client: {}", e);
                    }
                });
            }
            Err(e) => {
                error!("Failed to accept connection: {}", e);
            }
        }
    }
}

async fn handle_client(stream: tokio::net::UnixStream, state: Arc<Mutex<DaemonState>>) -> Result<(), Box<dyn std::error::Error>> {
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);
    let mut line = String::new();

    while buf_reader.read_line(&mut line).await? > 0 {
        // Parse request
        let request: IpcRequest = match serde_json::from_str(&line) {
            Ok(req) => req,
            Err(e) => {
                let response = IpcResponse::Error(format!("Invalid request JSON: {}", e));
                let resp_line = serde_json::to_string(&response)? + "\n";
                let _ = writer.write_all(resp_line.as_bytes()).await;
                line.clear();
                continue;
            }
        };

        debug!("Received IPC request: {:?}", request);

        // Process request
        let response = match request {
            IpcRequest::GetStatus => {
                let lock = state.lock().unwrap();
                let cpu_curve = lock.config.fan.curves.iter().find(|c| c.name == "cpu").map(|c| c.points.clone()).unwrap_or_default();
                let gpu_curve = lock.config.fan.curves.iter().find(|c| c.name == "gpu").map(|c| c.points.clone()).unwrap_or_default();
                
                IpcResponse::Status(SystemStatus {
                    current_gpu_mode: lock.current_gpu_mode,
                    cpu_temp: lock.cpu_temp,
                    gpu_temp: lock.gpu_temp,
                    cpu_fan_speed: lock.cpu_fan_speed,
                    gpu_fan_speed: lock.gpu_fan_speed,
                    cpu_curve,
                    gpu_curve,
                })
            }
            IpcRequest::SetGpuMode(mode) => {
                match switch_gpu_mode(mode) {
                    Ok(_) => {
                        let mut lock = state.lock().unwrap();
                        lock.current_gpu_mode = mode;
                        // Save new GPU mode as default in configuration file
                        lock.config.gpu.default_mode = mode;
                        if let Err(e) = lock.config.save_to_path(DEFAULT_CONFIG_PATH) {
                            warn!("Failed to save new GPU default mode to config file: {}", e);
                        }
                        IpcResponse::Ok
                    }
                    Err(e) => IpcResponse::Error(e.to_string()),
                }
            }
            IpcRequest::SetFanCurve { name, mut points } => {
                let mut lock = state.lock().unwrap();
                let mut found = false;

                // Ensure points are sorted by temp
                points.sort_by(|a, b| a.temp.partial_cmp(&b.temp).unwrap_or(std::cmp::Ordering::Equal));

                for curve in &mut lock.config.fan.curves {
                    if curve.name == name {
                        curve.points = points.clone();
                        found = true;
                        break;
                    }
                }

                if found {
                    match lock.config.save_to_path(DEFAULT_CONFIG_PATH) {
                        Ok(_) => IpcResponse::Ok,
                        Err(e) => IpcResponse::Error(format!("Failed to save configuration: {}", e)),
                    }
                } else {
                    IpcResponse::Error(format!("Fan curve name '{}' not found in configuration", name))
                }
            }
        };

        // Write response back
        let resp_line = serde_json::to_string(&response)? + "\n";
        let _ = writer.write_all(resp_line.as_bytes()).await;
        line.clear();
    }

    Ok(())
}

pub fn cleanup_socket() {
    if Path::new(SOCKET_PATH).exists() {
        info!("Cleaning up UDS socket at {}", SOCKET_PATH);
        let _ = fs::remove_file(SOCKET_PATH);
    }
}
