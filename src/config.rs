use std::path::Path;

use serde::Deserialize;

/// Корневая структура конфигурации.
#[derive(Debug, Deserialize, Default)]
pub struct Config {
    /// Настройки whisper: словарь терминов для улучшения распознавания.
    pub whisper: Option<WhisperConfig>,
}

/// Настройки whisper.
#[derive(Debug, Deserialize)]
pub struct WhisperConfig {
    /// Термины для initial_prompt (YouTrack, JetBrains, и т.д.).
    #[serde(default)]
    pub vocabulary: Vec<String>,
}

impl Config {
    /// Загружает конфиг из TOML-файла.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let content =
            std::fs::read_to_string(path).map_err(|e| ConfigError::Read(e.to_string()))?;
        let config: Config =
            toml::from_str(&content).map_err(|e| ConfigError::Parse(e.to_string()))?;
        Ok(config)
    }

    /// Склеивает vocabulary в строку для initial_prompt whisper.
    pub fn whisper_prompt(&self) -> Option<String> {
        self.whisper.as_ref().and_then(|w| {
            if w.vocabulary.is_empty() {
                None
            } else {
                Some(w.vocabulary.join(" "))
            }
        })
    }
}

#[derive(Debug)]
pub enum ConfigError {
    Read(String),
    Parse(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read(e) => write!(f, "ошибка чтения конфига: {e}"),
            Self::Parse(e) => write!(f, "ошибка парсинга конфига: {e}"),
        }
    }
}

impl std::error::Error for ConfigError {}
