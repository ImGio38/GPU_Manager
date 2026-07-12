use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use log::{info, warn, debug};
use hw_control_core::{
    read_temp, write_pwm_speed, enable_manual_control,
    interpolate_speed, Config, DiscoveredControls, GpuMode
};

pub struct DaemonState {
    pub config: Config,
    pub controls: DiscoveredControls,
    pub current_gpu_mode: GpuMode,
    pub cpu_fan_speed: Option<u8>,
    pub gpu_fan_speed: Option<u8>,
    pub cpu_temp: Option<f32>,
    pub gpu_temp: Option<f32>,
}

pub fn start_scheduler(state: Arc<Mutex<DaemonState>>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        info!("Starting fan control scheduler loop.");

        // Try to enable manual control on startup
        {
            let lock = state.lock().unwrap();
            if let Some(ref fan) = lock.controls.cpu_fan {
                if let Err(e) = enable_manual_control(fan) {
                    warn!("Failed to enable manual control for CPU fan: {}", e);
                }
            }
            if let Some(ref fan) = lock.controls.gpu_fan {
                if let Err(e) = enable_manual_control(fan) {
                    warn!("Failed to enable manual control for GPU fan: {}", e);
                }
            }
        }

        loop {
            // Read poll interval, curves, and controls from state
            let (poll_interval, cpu_curve, gpu_curve, controls) = {
                let lock = state.lock().unwrap();
                let poll_interval = lock.config.fan.poll_interval_secs;
                let mut cpu_curve = Vec::new();
                let mut gpu_curve = Vec::new();

                for curve in &lock.config.fan.curves {
                    if curve.name == "cpu" {
                        cpu_curve = curve.points.clone();
                    } else if curve.name == "gpu" {
                        gpu_curve = curve.points.clone();
                    }
                }

                (poll_interval, cpu_curve, gpu_curve, lock.controls.clone())
            };

            let mut current_cpu_temp = None;
            let mut current_gpu_temp = None;
            let mut current_cpu_speed = None;
            let mut current_gpu_speed = None;

            // 1. Process CPU Fan Control
            if let Some(ref sensor) = controls.cpu_temp {
                match read_temp(sensor) {
                    Ok(temp) => {
                        current_cpu_temp = Some(temp);
                        if let Some(ref fan) = controls.cpu_fan {
                            let speed = interpolate_speed(temp, &cpu_curve);
                            current_cpu_speed = Some(speed);
                            if let Err(e) = write_pwm_speed(fan, speed) {
                                warn!("Failed to write CPU fan speed: {}", e);
                            } else {
                                debug!("CPU Fan: Temp={:.1}C, TargetSpeed={}", temp, speed);
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to read CPU temperature: {}", e);
                    }
                }
            }

            // 2. Process GPU Fan Control
            if let Some(ref sensor) = controls.gpu_temp {
                match read_temp(sensor) {
                    Ok(temp) => {
                        current_gpu_temp = Some(temp);
                        if let Some(ref fan) = controls.gpu_fan {
                            let speed = interpolate_speed(temp, &gpu_curve);
                            current_gpu_speed = Some(speed);
                            if let Err(e) = write_pwm_speed(fan, speed) {
                                warn!("Failed to write GPU fan speed: {}", e);
                            } else {
                                debug!("GPU Fan: Temp={:.1}C, TargetSpeed={}", temp, speed);
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to read GPU temperature: {}", e);
                    }
                }
            }

            // Update telemetry in state
            {
                let mut lock = state.lock().unwrap();
                lock.cpu_temp = current_cpu_temp;
                lock.gpu_temp = current_gpu_temp;
                lock.cpu_fan_speed = current_cpu_speed;
                lock.gpu_fan_speed = current_gpu_speed;
            }

            thread::sleep(Duration::from_secs(poll_interval));
        }
    })
}
