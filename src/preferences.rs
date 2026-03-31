use crate::i18n::Language;
use crate::paths;
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PreferencesError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML error: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("TOML serialize error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
}

pub type PreferencesResult<T> = Result<T, PreferencesError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UserPreferences {
    pub language: Language,
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            language: Language::Es,
        }
    }
}

impl UserPreferences {
    pub fn load() -> PreferencesResult<Self> {
        let path = paths::user_preferences_file();
        if path.exists() {
            let content = std::fs::read_to_string(path)?;
            Ok(toml::from_str(&content)?)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self) -> PreferencesResult<()> {
        let path = paths::user_preferences_file();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        self.save_to(&path)
    }

    pub fn save_to(&self, path: &Path) -> PreferencesResult<()> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}
