use std::os::unix::net::UnixStream;
use std::io::{Write, BufReader, BufRead};
use std::time::Duration;
use hw_control_core::{IpcRequest, IpcResponse};

/// Send an IPC request to the daemon socket and wait for a response.
pub fn send_request(request: &IpcRequest) -> Result<IpcResponse, String> {
    let socket_path = "/run/hw-control.sock";
    
    // Connect to the Unix Domain Socket
    let mut stream = UnixStream::connect(socket_path)
        .map_err(|e| format!("Could not connect to daemon socket at {}: {}. Is the daemon running?", socket_path, e))?;

    // Set 500ms read/write timeouts to ensure the GUI is always responsive
    stream.set_read_timeout(Some(Duration::from_millis(500)))
        .map_err(|e| format!("Failed to set read timeout: {}", e))?;
    stream.set_write_timeout(Some(Duration::from_millis(500)))
        .map_err(|e| format!("Failed to set write timeout: {}", e))?;

    // Serialize request (newline-delimited JSON)
    let mut payload = serde_json::to_string(request)
        .map_err(|e| format!("Failed to serialize request: {}", e))?;
    payload.push('\n');
    
    // Send request
    stream.write_all(payload.as_bytes())
        .map_err(|e| format!("Failed to write to socket: {}", e))?;

    // Read response line
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)
        .map_err(|e| format!("Failed to read response line: {}", e))?;

    if line.is_empty() {
        return Err("Daemon closed connection unexpectedly".to_string());
    }

    // Parse response
    let response: IpcResponse = serde_json::from_str(&line)
        .map_err(|e| format!("Failed to parse daemon response JSON: {}", e))?;

    Ok(response)
}
