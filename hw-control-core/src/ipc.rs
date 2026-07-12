use serde::{Deserialize, Serialize};
use crate::config::{GpuMode, CurvePoint};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum IpcRequest {
    GetStatus,
    SetGpuMode(GpuMode),
    SetFanCurve {
        name: String, // "cpu" or "gpu"
        points: Vec<CurvePoint>,
    },
    Uninstall,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum IpcResponse {
    Status(SystemStatus),
    Ok,
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SystemStatus {
    pub current_gpu_mode: GpuMode,
    pub cpu_temp: Option<f32>,
    pub gpu_temp: Option<f32>,
    pub cpu_fan_speed: Option<u8>, // Current PWM value (0-255)
    pub gpu_fan_speed: Option<u8>,
    pub cpu_curve: Vec<CurvePoint>,
    pub gpu_curve: Vec<CurvePoint>,
}
