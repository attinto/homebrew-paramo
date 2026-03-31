use crate::paths;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;
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

const DEFAULT_CONFIG_TEMPLATE: &str = include_str!("../config/default.toml");

#[derive(Debug, Deserialize)]
struct EmbeddedSystemConfig {
    schedule: ScheduleConfig,
    sites: SitesConfig,
    hosts: HostsConfig,
    logging: LoggingConfig,
    daemon: DaemonConfig,
}

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
#[derive(Default)]
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
        Self::embedded_default()
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
        match Self::config_source() {
            ConfigSource::Active => Self::load_from(&paths::system_config_file()),
            ConfigSource::Legacy => Self::load_from(Path::new(paths::LEGACY_SYSTEM_CONFIG_FILE)),
            ConfigSource::EmbeddedDefault => Ok(Self::default()),
        }
    }

    pub fn config_source() -> ConfigSource {
        if paths::system_config_file().exists() {
            ConfigSource::Active
        } else if Path::new(paths::LEGACY_SYSTEM_CONFIG_FILE).exists() {
            ConfigSource::Legacy
        } else {
            ConfigSource::EmbeddedDefault
        }
    }

    pub fn effective_config_path() -> Option<PathBuf> {
        match Self::config_source() {
            ConfigSource::Active => Some(paths::system_config_file()),
            ConfigSource::Legacy => Some(PathBuf::from(paths::LEGACY_SYSTEM_CONFIG_FILE)),
            ConfigSource::EmbeddedDefault => None,
        }
    }

    pub fn load_effective_contents() -> ConfigResult<String> {
        if let Some(path) = Self::effective_config_path() {
            Ok(std::fs::read_to_string(path)?)
        } else {
            Ok(DEFAULT_CONFIG_TEMPLATE.to_string())
        }
    }

    #[cfg(test)]
    pub fn default_template() -> &'static str {
        DEFAULT_CONFIG_TEMPLATE
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
        write_atomic(path, &content)?;
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
        if cloned.logging.file == Path::new("/var/log/undistracted.log") {
            cloned.logging.file = PathBuf::from(paths::LOG_FILE);
        }
        cloned
    }

    fn embedded_default() -> Self {
        let config: EmbeddedSystemConfig =
            toml::from_str(DEFAULT_CONFIG_TEMPLATE).expect("embedded default config must be valid");
        Self {
            schedule: config.schedule,
            sites: config.sites,
            hosts: config.hosts,
            logging: config.logging,
            daemon: config.daemon,
        }
        .normalized()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigSource {
    Active,
    Legacy,
    EmbeddedDefault,
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

fn write_atomic(path: &Path, content: &str) -> std::io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut temp = NamedTempFile::new_in(parent)?;
    temp.write_all(content.as_bytes())?;
    temp.flush()?;
    temp.persist(path).map_err(|error| error.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

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

    #[test]
    fn test_default_matches_embedded_template() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("default.toml");
        std::fs::write(&config_path, SystemConfig::default_template()).unwrap();

        let defaults = SystemConfig::default();
        let parsed = SystemConfig::load_from(&config_path).unwrap();

        assert_eq!(defaults.sites.list, parsed.sites.list);
        assert_eq!(defaults.schedule.start, parsed.schedule.start);
        assert_eq!(
            defaults.daemon.interval_seconds,
            parsed.daemon.interval_seconds
        );
    }

    #[test]
    fn test_effective_config_path_uses_active_file_when_present() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        let config = SystemConfig::default();
        config.save_to(&config_path).unwrap();

        assert_eq!(
            SystemConfig::load_from(&config_path).unwrap().sites.list,
            config.sites.list
        );
    }
}
