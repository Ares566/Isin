use std::path::Path;

use serde::Deserialize;

/// Корневая структура конфигурации.
#[derive(Debug, Deserialize, Default)]
pub struct Config {
    /// Настройки whisper: словарь терминов для улучшения распознавания.
    pub whisper: Option<WhisperConfig>,
    /// Настройки Ollama для извлечения параметров.
    pub ollama: Option<OllamaConfig>,
    /// Список скиллов.
    #[serde(default)]
    pub skill: Vec<SkillDef>,
}

/// Настройки whisper.
#[derive(Debug, Deserialize)]
pub struct WhisperConfig {
    /// Термины для initial_prompt (YouTrack, JetBrains, и т.д.).
    #[serde(default)]
    pub vocabulary: Vec<String>,
}

/// Настройки Ollama.
#[derive(Debug, Deserialize, Clone)]
pub struct OllamaConfig {
    /// URL Ollama API.
    #[serde(default = "default_ollama_url")]
    pub url: String,
    /// Имя модели для извлечения параметров.
    #[serde(default = "default_ollama_model")]
    pub model: String,
}

fn default_ollama_url() -> String {
    "http://localhost:11434".into()
}

fn default_ollama_model() -> String {
    "qwen2.5:1.5b".into()
}

/// Описание одного скилла из конфига.
#[derive(Debug, Deserialize, Clone)]
pub struct SkillDef {
    /// Имя скилла (идентификатор).
    pub name: String,
    /// Триггерные фразы для матчинга. Пусто = фоллбэк.
    #[serde(default)]
    pub triggers: Vec<String>,
    /// Правила извлечения параметров из текста.
    #[serde(default)]
    pub params: Vec<ParamRule>,
    /// Тип действия: clipboard, file_append, youtrack, shell, open.
    pub action: String,
    /// Цель действия (путь к файлу/директории, URL API, и т.д.).
    pub target: Option<String>,
    /// Шаблон имени файла для file_append ({{date}}, {{title}}, {{slug}}, {{time}}).
    pub filename_template: Option<String>,
    /// Требовать подтверждение перед выполнением.
    #[serde(default)]
    pub confirm: bool,
}

/// Правило извлечения одного параметра из текста.
#[derive(Debug, Deserialize, Clone)]
pub struct ParamRule {
    /// Имя параметра.
    pub name: String,
    /// Описание параметра для LLM (приоритетно).
    pub description: Option<String>,
    /// Regex-паттерн — fallback если Ollama недоступна.
    #[serde(default)]
    pub pattern: Option<String>,
    /// Параметр опциональный.
    #[serde(default)]
    pub optional: bool,
}

impl Config {
    /// Загружает конфиг из TOML-файла.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::Read(e.to_string()))?;
        let config: Config = toml::from_str(&content)
            .map_err(|e| ConfigError::Parse(e.to_string()))?;
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
