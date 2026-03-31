use crate::paths;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML error: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("TOML serialize error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
}

pub type ConfigResult<T> = Result<T, ConfigError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub schedule: ScheduleConfig,
    pub domains: DomainsConfig,
    pub hosts: HostsConfig,
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleConfig {
    pub block_start: u8,
    pub block_end: u8,
    pub block_weekends: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainsConfig {
    pub list: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostsConfig {
    pub file: PathBuf,
    pub marker: String,
    pub redirect_ips: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub file: PathBuf,
    pub level: String,
}

impl Default for Config {
    fn default() -> Self {
        toml::from_str(DEFAULT_CONFIG).expect("Failed to parse default config")
    }
}

impl Config {
    pub fn load() -> ConfigResult<Self> {
        let config_path = PathBuf::from(paths::CONFIG_FILE);

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            Ok(toml::from_str(&content)?)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self, path: &Path) -> ConfigResult<()> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    pub fn update_value(&mut self, key: &str, value: &str) -> Result<(), String> {
        match key {
            "schedule.block_start" => {
                self.schedule.block_start = value.parse().map_err(|_| "Invalid hour (0-23)")?;
            }
            "schedule.block_end" => {
                self.schedule.block_end = value.parse().map_err(|_| "Invalid hour (0-23)")?;
            }
            "schedule.block_weekends" => {
                self.schedule.block_weekends =
                    value.parse().map_err(|_| "Invalid boolean (true/false)")?;
            }
            "logging.level" => {
                self.logging.level = value.to_string();
            }
            _ => return Err(format!("Unknown configuration key: {}", key)),
        }
        Ok(())
    }
}

const DEFAULT_CONFIG: &str = include_str!("../config/default.toml");
