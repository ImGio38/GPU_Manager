pub mod config;
pub mod sysfs_gpu;
pub mod sysfs_fan;
pub mod ipc;

// Re-exports for convenience
pub use config::{Config, GpuMode, FanCurve, CurvePoint, DEFAULT_CONFIG_PATH};
pub use sysfs_gpu::{
    switch_gpu_mode,
    find_dgpu_pci_address,
    find_wmi_file,
    GpuError,
};
pub use sysfs_fan::{
    scan_hwmon_devices,
    discover_controls,
    read_temp,
    write_pwm_speed,
    enable_manual_control,
    disable_manual_control,
    interpolate_speed,
    HwmonDevice,
    PwmControl,
    TempSensor,
    DiscoveredControls,
};
pub use ipc::{IpcRequest, IpcResponse, SystemStatus};

