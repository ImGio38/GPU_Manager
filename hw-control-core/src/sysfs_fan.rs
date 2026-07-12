use std::fs;
use std::path::{Path, PathBuf};
use log::{info, warn, debug};
use crate::config::CurvePoint;

#[derive(Debug, Clone)]
pub struct HwmonDevice {
    pub path: PathBuf,
    pub name: String,
    pub pwm_controls: Vec<PwmControl>,
    pub temp_sensors: Vec<TempSensor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PwmControl {
    pub pwm_file: PathBuf,
    pub enable_file: PathBuf,
    pub index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TempSensor {
    pub input_file: PathBuf,
    pub label: Option<String>,
    pub index: u32,
}

#[derive(Debug, Clone)]
pub struct DiscoveredControls {
    pub cpu_temp: Option<TempSensor>,
    pub cpu_fan: Option<PwmControl>,
    pub gpu_temp: Option<TempSensor>,
    pub gpu_fan: Option<PwmControl>,
}

/// Scan /sys/class/hwmon/ to find all hwmon devices, their temp sensors, and PWM controls.
pub fn scan_hwmon_devices() -> Vec<HwmonDevice> {
    let mut devices = Vec::new();
    let hwmon_dir = Path::new("/sys/class/hwmon");
    if !hwmon_dir.exists() {
        warn!("hwmon directory {:?} does not exist. (Ignore if running in mock/non-Linux environment)", hwmon_dir);
        return devices;
    }

    if let Ok(entries) = fs::read_dir(hwmon_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            // Resolve symlink to get the real directory (e.g. /sys/devices/...)
            let canonical_path = fs::canonicalize(&path).unwrap_or(path);
            
            // Read device name (often a file named 'name' under the hwmon directory)
            let name_file = canonical_path.join("name");
            if !name_file.exists() {
                continue;
            }
            let name = fs::read_to_string(&name_file)
                .unwrap_or_default()
                .trim()
                .to_string();

            let mut pwm_controls = Vec::new();
            let mut temp_sensors = Vec::new();

            // Read all files inside the hwmon directory
            if let Ok(sub_entries) = fs::read_dir(&canonical_path) {
                for sub_entry in sub_entries.flatten() {
                    let file_name = sub_entry.file_name();
                    let file_name_str = file_name.to_string_lossy();

                    // Match pwm[index] controls (e.g. pwm1, pwm2, etc., but not pwm1_enable)
                    if file_name_str.starts_with("pwm") && !file_name_str.contains('_') {
                        if let Ok(index) = file_name_str["pwm".len()..].parse::<u32>() {
                            let pwm_file = sub_entry.path();
                            let enable_file = canonical_path.join(format!("pwm{}_enable", index));
                            
                            // Log if enable file is missing, but still allow pwm file
                            if !enable_file.exists() {
                                debug!("PWM control {:?} found but enable file {:?} is missing.", pwm_file, enable_file);
                            }
                            
                            pwm_controls.push(PwmControl {
                                pwm_file,
                                enable_file,
                                index,
                            });
                        }
                    }

                    // Match temp[index]_input sensors
                    if file_name_str.starts_with("temp") && file_name_str.ends_with("_input") {
                        // Extract index from "temp[index]_input"
                        let index_str: String = file_name_str.chars()
                            .skip(4) // skip "temp"
                            .take_while(|c| c.is_ascii_digit())
                            .collect();
                        
                        if let Ok(index) = index_str.parse::<u32>() {
                            let input_file = sub_entry.path();
                            let label_file = canonical_path.join(format!("temp{}_label", index));
                            let label = if label_file.exists() {
                                fs::read_to_string(label_file).ok().map(|s| s.trim().to_string())
                            } else {
                                None
                            };
                            
                            temp_sensors.push(TempSensor {
                                input_file,
                                label,
                                index,
                            });
                        }
                    }
                }
            }

            // Sort lists by index for deterministic lookup
            pwm_controls.sort_by_key(|c| c.index);
            temp_sensors.sort_by_key(|s| s.index);

            debug!("Discovered hwmon device '{}' at {:?} with {} PWM and {} Temp nodes", 
                name, canonical_path, pwm_controls.len(), temp_sensors.len());

            devices.push(HwmonDevice {
                path: canonical_path,
                name,
                pwm_controls,
                temp_sensors,
            });
        }
    }

    devices
}

/// Dynamically search and identify CPU / GPU temperature sensors and PWM controls from system.
pub fn discover_controls(devices: &[HwmonDevice]) -> DiscoveredControls {
    let mut cpu_temp = None;
    let mut cpu_fan = None;
    let mut gpu_temp = None;
    let mut gpu_fan = None;

    // 1. Resolve CPU temperature:
    // Coretemp / k10temp / zenpower are the standard CPU temp sensors.
    for dev in devices {
        let name_lower = dev.name.to_lowercase();
        if name_lower.contains("coretemp") || name_lower.contains("k10temp") || name_lower.contains("zenpower") {
            // Prefer sensor with label "Package id 0" or just the first sensor
            if !dev.temp_sensors.is_empty() {
                let package_sensor = dev.temp_sensors.iter().find(|s| {
                    s.label.as_ref().map_or(false, |lbl| lbl.to_lowercase().contains("package"))
                });
                cpu_temp = package_sensor.or(dev.temp_sensors.first()).cloned();
                debug!("Matched CPU temp sensor from hwmon '{}' at {:?}", dev.name, cpu_temp.as_ref().unwrap().input_file);
                break;
            }
        }
    }

    // Fallback CPU temp search if coretemp is not found:
    if cpu_temp.is_none() {
        for dev in devices {
            let name_lower = dev.name.to_lowercase();
            if name_lower.contains("cpu") || name_lower.contains("acpitz") {
                if let Some(sensor) = dev.temp_sensors.first() {
                    cpu_temp = Some(sensor.clone());
                    debug!("Fallback CPU temp sensor from hwmon '{}' at {:?}", dev.name, sensor.input_file);
                    break;
                }
            }
        }
    }

    // 2. Resolve GPU temperature:
    // Nouveau / amdgpu / nvidia-smi / platform GPU temps.
    for dev in devices {
        let name_lower = dev.name.to_lowercase();
        if name_lower.contains("amdgpu") || name_lower.contains("nouveau") || name_lower.contains("nvidia") {
            if let Some(sensor) = dev.temp_sensors.first() {
                gpu_temp = Some(sensor.clone());
                debug!("Matched GPU temp sensor from hwmon '{}' at {:?}", dev.name, sensor.input_file);
                break;
            }
        }
    }

    // 3. Resolve Fan PWMs from platform driver (ASUS / Thinkpad / HP / Dell etc.):
    for dev in devices {
        let name_lower = dev.name.to_lowercase();
        let is_platform = name_lower.contains("asus")
            || name_lower.contains("thinkpad")
            || name_lower.contains("dell")
            || name_lower.contains("hp")
            || name_lower.contains("f71882fg")
            || name_lower.contains("nct6779")
            || name_lower.contains("it87")
            || name_lower.contains("applesmc");

        if is_platform {
            if dev.pwm_controls.len() >= 2 {
                // Commonly pwm1 is CPU, pwm2 is GPU on dual-fan laptops (like ASUS Rog/Tuf)
                cpu_fan = Some(dev.pwm_controls[0].clone());
                gpu_fan = Some(dev.pwm_controls[1].clone());
                debug!("Matched dual-fan platform device '{}': CPU={:?}, GPU={:?}", dev.name, cpu_fan, gpu_fan);
                break;
            } else if dev.pwm_controls.len() == 1 {
                cpu_fan = Some(dev.pwm_controls[0].clone());
                debug!("Matched single-fan platform device '{}': CPU={:?}", dev.name, cpu_fan);
                break;
            }
        }
    }

    // GPU fan fallback if not found in platform driver, check if amdgpu/nouveau has it:
    if gpu_fan.is_none() {
        for dev in devices {
            let name_lower = dev.name.to_lowercase();
            if (name_lower.contains("amdgpu") || name_lower.contains("nouveau")) && !dev.pwm_controls.is_empty() {
                gpu_fan = Some(dev.pwm_controls[0].clone());
                debug!("Matched GPU fallback fan from GPU driver '{}': GPU={:?}", dev.name, gpu_fan);
                break;
            }
        }
    }

    // Absolute fallback: assign whatever we found if still empty
    if cpu_temp.is_none() || cpu_fan.is_none() {
        for dev in devices {
            if cpu_temp.is_none() && !dev.temp_sensors.is_empty() {
                cpu_temp = dev.temp_sensors.first().cloned();
            }
            if cpu_fan.is_none() && !dev.pwm_controls.is_empty() {
                cpu_fan = dev.pwm_controls.first().cloned();
            }
        }
    }

    DiscoveredControls {
        cpu_temp,
        cpu_fan,
        gpu_temp,
        gpu_fan,
    }
}

/// Read temperature in Celsius from a TempSensor.
pub fn read_temp(sensor: &TempSensor) -> Result<f32, std::io::Error> {
    if !sensor.input_file.exists() {
        warn!("Temperature input file {:?} does not exist", sensor.input_file);
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Sensor file {:?} not found", sensor.input_file),
        ));
    }

    let content = fs::read_to_string(&sensor.input_file)?;
    let millidegrees = content.trim().parse::<i32>().map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Invalid temperature format in {:?}: {}", sensor.input_file, e),
        )
    })?;

    Ok(millidegrees as f32 / 1000.0)
}

/// Enable manual control mode on a PWM fan.
/// Writes '1' to pwm*_enable.
pub fn enable_manual_control(control: &PwmControl) -> Result<(), std::io::Error> {
    if !control.enable_file.exists() {
        warn!("PWM enable file {:?} does not exist. Skipping manual toggle.", control.enable_file);
        return Ok(());
    }

    info!("Enabling manual control on fan {:?}", control.enable_file);
    fs::write(&control.enable_file, "1")
}

/// Restore automatic BIOS/firmware control mode on a PWM fan.
/// Writes '2' to pwm*_enable.
pub fn disable_manual_control(control: &PwmControl) -> Result<(), std::io::Error> {
    if !control.enable_file.exists() {
        warn!("PWM enable file {:?} does not exist. Skipping auto toggle.", control.enable_file);
        return Ok(());
    }

    info!("Restoring BIOS automatic control on fan {:?}", control.enable_file);
    fs::write(&control.enable_file, "2")
}

/// Write PWM duty cycle value (0-255) to a PWM fan.
pub fn write_pwm_speed(control: &PwmControl, speed: u8) -> Result<(), std::io::Error> {
    if !control.pwm_file.exists() {
        warn!("PWM speed control file {:?} does not exist. Mocking write of value {}.", control.pwm_file, speed);
        return Ok(());
    }

    debug!("Writing PWM duty cycle {} to {:?}", speed, control.pwm_file);
    fs::write(&control.pwm_file, speed.to_string())
}

/// Perform linear interpolation to calculate speed (0-255) from temperature.
pub fn interpolate_speed(temp: f32, points: &[CurvePoint]) -> u8 {
    if points.is_empty() {
        return 0;
    }
    if points.len() == 1 {
        return points[0].speed;
    }

    // Temp is lower than the first point
    if temp <= points[0].temp {
        return points[0].speed;
    }

    // Temp is higher than the last point
    let last_idx = points.len() - 1;
    if temp >= points[last_idx].temp {
        return points[last_idx].speed;
    }

    // Interpolate between the boundaries
    for i in 0..last_idx {
        let p1 = &points[i];
        let p2 = &points[i + 1];
        if temp >= p1.temp && temp <= p2.temp {
            let t_diff = p2.temp - p1.temp;
            if t_diff.abs() < 1e-5 {
                return p1.speed;
            }
            let factor = (temp - p1.temp) / t_diff;
            let speed_diff = p2.speed as f32 - p1.speed as f32;
            let speed = p1.speed as f32 + factor * speed_diff;
            return speed.round().clamp(0.0, 255.0) as u8;
        }
    }

    points[last_idx].speed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpolation() {
        let points = vec![
            CurvePoint { temp: 30.0, speed: 50 },
            CurvePoint { temp: 50.0, speed: 100 },
            CurvePoint { temp: 70.0, speed: 200 },
        ];

        // Boundaries
        assert_eq!(interpolate_speed(25.0, &points), 50);
        assert_eq!(interpolate_speed(75.0, &points), 200);

        // Exact points
        assert_eq!(interpolate_speed(30.0, &points), 50);
        assert_eq!(interpolate_speed(50.0, &points), 100);
        assert_eq!(interpolate_speed(70.0, &points), 200);

        // Mid points
        assert_eq!(interpolate_speed(40.0, &points), 75); // Halfway between 30 and 50 is speed 75
        assert_eq!(interpolate_speed(60.0, &points), 150); // Halfway between 50 and 70 is speed 150
    }
}
