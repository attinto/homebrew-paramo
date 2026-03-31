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
    #[error("{0}")]
    Validation(String),
}

pub type ConfigResult<T> = Result<T, ConfigError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SystemConfig {
    pub schedule: ScheduleConfig,
    #[serde(alias = "domains")]
    pub sites: SitesConfig,
    pub hosts: HostsConfig,
    pub logging: LoggingConfig,
    pub daemon: DaemonConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScheduleConfig {
    #[serde(alias = "block_start")]
    pub start: u8,
    #[serde(alias = "block_end")]
    pub end: u8,
    pub block_weekends: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SitesConfig {
    pub list: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HostsConfig {
    pub file: PathBuf,
    pub marker: String,
    pub redirect_ips: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LoggingConfig {
    pub file: PathBuf,
    pub level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DaemonConfig {
    pub interval_seconds: u32,
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            schedule: ScheduleConfig::default(),
            sites: SitesConfig {
                list: vec![
                    "tiktok.com".to_string(),
                    "m.tiktok.com".to_string(),
                    "vm.tiktok.com".to_string(),
                    "api.tiktok.com".to_string(),
                    "api2.musical.ly".to_string(),
                    "musical.ly".to_string(),
                    "instagram.com".to_string(),
                    "i.instagram.com".to_string(),
                    "l.instagram.com".to_string(),
                    "graph.instagram.com".to_string(),
                    "cdninstagram.com".to_string(),
                    "pornhub.com".to_string(),
                    "es.pornhub.com".to_string(),
                    "xvideos.com".to_string(),
                    "xnxx.com".to_string(),
                    "xhamster.com".to_string(),
                    "redtube.com".to_string(),
                    "youporn.com".to_string(),
                    "tube8.com".to_string(),
                    "spankbang.com".to_string(),
                    "youtube.com".to_string(),
                ],
            },
            hosts: HostsConfig::default(),
            logging: LoggingConfig::default(),
            daemon: DaemonConfig::default(),
        }
    }
}

impl Default for ScheduleConfig {
    fn default() -> Self {
        Self {
            start: 9,
            end: 18,
            block_weekends: false,
        }
    }
}

impl Default for SitesConfig {
    fn default() -> Self {
        Self { list: Vec::new() }
    }
}

impl Default for HostsConfig {
    fn default() -> Self {
        Self {
            file: PathBuf::from("/etc/hosts"),
            marker: "# --- PARAMO BLOCK ---".to_string(),
            redirect_ips: vec!["127.0.0.1".to_string(), "::1".to_string()],
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            file: PathBuf::from(paths::LOG_FILE),
            level: "info".to_string(),
        }
    }
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            interval_seconds: 1200,
        }
    }
}

impl SystemConfig {
    pub fn load() -> ConfigResult<Self> {
        let path = paths::system_config_file();
        if path.exists() {
            Self::load_from(&path)
        } else {
            let legacy = PathBuf::from(paths::LEGACY_SYSTEM_CONFIG_FILE);
            if legacy.exists() {
                Self::load_from(&legacy)
            } else {
                Ok(Self::default())
            }
        }
    }

    pub fn load_from(path: &Path) -> ConfigResult<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        config.validate()?;
        Ok(config.normalized())
    }

    pub fn save_active(&self) -> ConfigResult<()> {
        let path = paths::system_config_file();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        self.save_to(&path)
    }

    pub fn save_to(&self, path: &Path) -> ConfigResult<()> {
        let normalized = self.normalized();
        normalized.validate()?;
        let content = toml::to_string_pretty(&normalized)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    pub fn add_site(&mut self, raw_site: &str) -> Result<SiteMutation, String> {
        let site = normalize_site_input(raw_site)?;

        if self
            .sites
            .list
            .iter()
            .filter_map(|existing| normalized_site_key(existing))
            .any(|existing| existing == site)
        {
            return Ok(SiteMutation::AlreadyPresent(site));
        }

        self.sites.list.push(site.clone());
        self.sites.list.sort();
        self.sites.list.dedup();

        Ok(SiteMutation::Added(site))
    }

    pub fn remove_site(&mut self, raw_site: &str) -> Result<SiteMutation, String> {
        let site = normalize_site_input(raw_site)?;
        if let Some(index) =
            self.sites.list.iter().position(|existing| {
                normalized_site_key(existing).as_deref() == Some(site.as_str())
            })
        {
            self.sites.list.remove(index);
            Ok(SiteMutation::Removed(site))
        } else {
            Ok(SiteMutation::NotFound(site))
        }
    }

    pub fn set_schedule(&mut self, start: u8, end: u8, block_weekends: bool) -> Result<(), String> {
        validate_hour(start)?;
        validate_hour(end)?;
        self.schedule.start = start;
        self.schedule.end = end;
        self.schedule.block_weekends = block_weekends;
        Ok(())
    }

    pub fn validate(&self) -> ConfigResult<()> {
        validate_hour(self.schedule.start).map_err(ConfigError::Validation)?;
        validate_hour(self.schedule.end).map_err(ConfigError::Validation)?;

        if self.sites.list.is_empty() {
            return Ok(());
        }

        for site in &self.sites.list {
            normalize_site_input(site).map_err(ConfigError::Validation)?;
        }

        Ok(())
    }

    fn normalized(&self) -> Self {
        let mut cloned = self.clone();
        cloned.sites.list = cloned
            .sites
            .list
            .iter()
            .filter_map(|site| normalize_site_input(site).ok())
            .collect();
        cloned.sites.list.sort();
        cloned.sites.list.dedup();
        if cloned.logging.file == PathBuf::from("/var/log/undistracted.log") {
            cloned.logging.file = PathBuf::from(paths::LOG_FILE);
        }
        cloned
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SiteMutation {
    Added(String),
    Removed(String),
    AlreadyPresent(String),
    NotFound(String),
}

pub fn normalize_site_input(raw_site: &str) -> Result<String, String> {
    let mut site = raw_site.trim().to_lowercase();
    if site.is_empty() {
        return Err("El dominio no puede estar vacío.".to_string());
    }

    for prefix in ["https://", "http://"] {
        if let Some(stripped) = site.strip_prefix(prefix) {
            site = stripped.to_string();
        }
    }

    let first_segment = site
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        .trim()
        .trim_end_matches('.')
        .to_string();
    let host = first_segment
        .split(':')
        .next()
        .unwrap_or_default()
        .trim()
        .to_string();

    if host.is_empty() {
        return Err("No se ha encontrado un dominio válido.".to_string());
    }

    let normalized = host.strip_prefix("www.").unwrap_or(&host).to_string();

    if normalized.len() < 3 || !normalized.contains('.') {
        return Err("El dominio debe tener al menos un punto.".to_string());
    }

    if !normalized
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '.')
    {
        return Err("El dominio contiene caracteres no válidos.".to_string());
    }

    Ok(normalized)
}

fn normalized_site_key(existing: &str) -> Option<String> {
    normalize_site_input(existing).ok()
}

fn validate_hour(value: u8) -> Result<(), String> {
    if value <= 23 {
        Ok(())
    } else {
        Err("La hora debe estar entre 0 y 23.".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_site_input() {
        assert_eq!(
            normalize_site_input("https://www.youtube.com/watch?v=abc").unwrap(),
            "youtube.com"
        );
        assert_eq!(
            normalize_site_input("Sub.Example.com").unwrap(),
            "sub.example.com"
        );
    }

    #[test]
    fn test_add_site_deduplicates_www() {
        let mut config = SystemConfig::default();
        config.sites.list.clear();
        assert!(matches!(
            config.add_site("youtube.com").unwrap(),
            SiteMutation::Added(_)
        ));
        assert!(matches!(
            config.add_site("www.youtube.com").unwrap(),
            SiteMutation::AlreadyPresent(_)
        ));
        assert_eq!(config.sites.list, vec!["youtube.com".to_string()]);
    }

    #[test]
    fn test_remove_site_handles_www_variant() {
        let mut config = SystemConfig::default();
        config.sites.list = vec!["youtube.com".to_string()];
        assert!(matches!(
            config.remove_site("www.youtube.com").unwrap(),
            SiteMutation::Removed(_)
        ));
        assert!(config.sites.list.is_empty());
    }
}
