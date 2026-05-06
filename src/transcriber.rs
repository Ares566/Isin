use std::path::Path;

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// Обёртка над whisper-rs для транскрибации аудио.
///
/// Инициализирует контекст модели один раз и переиспользует его
/// для повторных вызовов транскрибации.
pub struct Transcriber {
    ctx: WhisperContext,
    language: String,
    initial_prompt: Option<String>,
}

impl Transcriber {
    /// Создаёт новый транскрибатор из GGML-модели.
    ///
    /// `model_path` — путь к файлу ggml-*.bin
    /// `language` — код языка (например, "ru")
    /// `initial_prompt` — промпт для улучшения распознавания (имена, термины)
    pub fn new(
        model_path: &Path,
        language: &str,
        initial_prompt: Option<String>,
    ) -> Result<Self, TranscriberError> {
        let ctx = WhisperContext::new_with_params(
            model_path.to_str().ok_or(TranscriberError::InvalidPath)?,
            WhisperContextParameters::default(),
        )
        .map_err(|e| TranscriberError::Context(e.to_string()))?;

        Ok(Self {
            ctx,
            language: language.to_string(),
            initial_prompt,
        })
    }

    /// Транскрибирует аудио из массива f32-сэмплов (16kHz mono).
    ///
    /// Возвращает распознанный текст целиком.
    pub fn transcribe(&self, audio: &[f32]) -> Result<String, TranscriberError> {
        if audio.is_empty() {
            return Ok(String::new());
        }

        let mut state = self
            .ctx
            .create_state()
            .map_err(|e| TranscriberError::State(e.to_string()))?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some(&self.language));
        params.set_print_progress(false);
        params.set_print_timestamps(false);
        params.set_print_special(false);
        params.set_print_realtime(false);
        params.set_suppress_blank(true);
        params.set_suppress_nst(true);

        if let Some(ref prompt) = self.initial_prompt {
            params.set_initial_prompt(prompt);
        }

        state
            .full(params, audio)
            .map_err(|e| TranscriberError::Inference(e.to_string()))?;

        let num_segments = state.full_n_segments();

        let mut text = String::new();
        for i in 0..num_segments {
            let Some(segment) = state.get_segment(i) else {
                continue;
            };
            let trimmed = segment.to_string();
            let trimmed = trimmed.trim();
            if !trimmed.is_empty() {
                if !text.is_empty() {
                    text.push(' ');
                }
                text.push_str(trimmed);
            }
        }

        Ok(text)
    }
}

#[derive(Debug)]
pub enum TranscriberError {
    InvalidPath,
    Context(String),
    State(String),
    Inference(String),
}

impl std::fmt::Display for TranscriberError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPath => write!(f, "неверный путь к модели"),
            Self::Context(e) => write!(f, "ошибка загрузки модели: {e}"),
            Self::State(e) => write!(f, "ошибка создания состояния: {e}"),
            Self::Inference(e) => write!(f, "ошибка транскрибации: {e}"),
        }
    }
}

impl std::error::Error for TranscriberError {}
