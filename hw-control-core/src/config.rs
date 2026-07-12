use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use log::{warn, info};

pub const DEFAULT_CONFIG_PATH: &str = "/etc/hw-control.toml";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Config {
    pub gpu: GpuConfig,
    pub fan: FanConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GpuConfig {
    pub default_mode: GpuMode,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum GpuMode {
    Integrated,
    Hybrid,
    Dedicated,
}

impl std::fmt::Display for GpuMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GpuMode::Integrated => write!(f, "Integrated"),
            GpuMode::Hybrid => write!(f, "Hybrid"),
            GpuMode::Dedicated => write!(f, "Dedicated"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FanConfig {
    pub poll_interval_secs: u64,
    pub curves: Vec<FanCurve>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FanCurve {
    pub name: String,
    pub points: Vec<CurvePoint>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct CurvePoint {
    pub temp: f32,   // Temperature in Celsius
    pub speed: u8,   // Fan speed duty cycle (0-255)
}

impl Default for Config {
    fn default() -> Self {
        Self {
            gpu: GpuConfig {
                default_mode: GpuMode::Hybrid,
            },
            fan: FanConfig {
                poll_interval_secs: 2,
                curves: vec![
                    FanCurve {
                        name: "cpu".to_string(),
                        points: vec![
                            CurvePoint { temp: 30.0, speed: 50 },
                            CurvePoint { temp: 50.0, speed: 100 },
                            CurvePoint { temp: 70.0, speed: 180 },
                            CurvePoint { temp: 85.0, speed: 255 },
                        ],
                    },
                    FanCurve {
                        name: "gpu".to_string(),
                        points: vec![
                            CurvePoint { temp: 35.0, speed: 0 },
                            CurvePoint { temp: 55.0, speed: 90 },
                            CurvePoint { temp: 75.0, speed: 170 },
                            CurvePoint { temp: 85.0, speed: 255 },
                        ],
                    },
                ],
            },
        }
    }
}

impl Config {
    /// Load config from the default path `/etc/hw-control.toml`.
    /// Falls back to Default if path doesn't exist or is invalid.
    pub fn load() -> Self {
        Self::load_from_path(DEFAULT_CONFIG_PATH)
    }

    /// Load config from a specific path.
    pub fn load_from_path<P: AsRef<Path>>(path: P) -> Self {
        let path = path.as_ref();
        if !path.exists() {
            info!("Config file at {:?} does not exist. Using default configuration.", path);
            return Config::default();
        }

        match fs::read_to_string(path) {
            Ok(content) => match toml::from_str::<Config>(&content) {
                Ok(mut config) => {
                    info!("Successfully loaded configuration from {:?}", path);
                    // Ensure points are sorted by temperature for linear interpolation
                    for curve in &mut config.fan.curves {
                        curve.points.sort_by(|a, b| a.temp.partial_cmp(&b.temp).unwrap_or(std::cmp::Ordering::Equal));
                    }
                    config
                }
                Err(err) => {
                    warn!("Failed to parse config file at {:?}: {}. Using default configuration.", path, err);
                    Config::default()
                }
            },
            Err(err) => {
                warn!("Failed to read config file at {:?}: {}. Using default configuration.", path, err);
                Config::default()
            }
        }
    }

    /// Save config to a specific path.
    pub fn save_to_path<P: AsRef<Path>>(&self, path: P) -> Result<(), std::io::Error> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        fs::write(path, content)
    }
}
