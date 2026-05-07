use std::collections::HashMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::config::{ParamRule, SkillDef};

/// Клиент для Ollama API. Используется для извлечения параметров и intent recognition.
pub struct OllamaClient {
    url: String,
    model: String,
    client: reqwest::Client,
    available: bool,
}

/// Запрос к Ollama API.
#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    stream: bool,
    options: OllamaOptions,
}

#[derive(Serialize)]
struct OllamaOptions {
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<u32>,
}

/// Ответ Ollama API.
#[derive(Deserialize)]
struct OllamaResponse {
    response: String,
}

/// Результат intent recognition через Ollama.
#[derive(Debug, Deserialize)]
pub struct IntentMatch {
    /// Имя скилла (action).
    pub skill: String,
    /// Извлечённые параметры.
    #[serde(default)]
    pub params: HashMap<String, String>,
}

impl OllamaClient {
    /// Создаёт клиент с заданным URL и моделью.
    pub fn new(url: &str, model: &str) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_default();
        Self {
            url: url.to_string(),
            model: model.to_string(),
            client,
            available: true,
        }
    }

    /// Проверяет доступность Ollama.
    pub async fn check_available(&mut self) -> bool {
        let url = format!("{}/api/tags", self.url);
        match self.client.get(&url).send().await {
            Ok(resp) => {
                self.available = resp.status().is_success();
            }
            Err(_) => {
                self.available = false;
            }
        }
        self.available
    }

    /// Извлекает параметры из текста через Ollama.
    pub async fn extract_params(
        &self,
        text: &str,
        rules: &[ParamRule],
    ) -> Option<HashMap<String, String>> {
        if !self.available {
            eprintln!("  [llm] extract_params: Ollama недоступна");
            return None;
        }

        let fields_desc: Vec<String> = rules
            .iter()
            .map(|r| {
                let desc = r.description.as_deref().unwrap_or("значение параметра");
                let opt = if r.optional { " (необязательный)" } else { "" };
                format!("- {}: {}{}", r.name, desc, opt)
            })
            .collect();

        let field_names: Vec<&str> = rules.iter().map(|r| r.name.as_str()).collect();

        let prompt = format!(
            "Извлеки параметры из текста и верни ТОЛЬКО JSON без пояснений.\n\
             Текст: \"{text}\"\n\
             Параметры:\n{fields}\n\
             Формат ответа: {{{example}}}\n\
             Если параметр не найден — omit его из JSON.",
            text = text,
            fields = fields_desc.join("\n"),
            example = field_names
                .iter()
                .map(|n| format!("\"{n}\": \"...\""))
                .collect::<Vec<_>>()
                .join(", ")
        );

        let request = OllamaRequest {
            model: self.model.clone(),
            prompt,
            stream: false,
            options: OllamaOptions { temperature: 0.1, num_predict: Some(200) },
        };

        eprintln!("  [llm] extract_params: отправка запроса...");
        let t = std::time::Instant::now();

        let url = format!("{}/api/generate", self.url);
        let resp = tokio::time::timeout(
            Duration::from_secs(15),
            self.client.post(&url).json(&request).send(),
        )
        .await
        .map_err(|_| {
            eprintln!("  [llm] extract_params: таймаут 15с");
        })
        .ok()?
        .map_err(|e| {
            eprintln!("  [llm] extract_params: ошибка запроса: {e}");
        })
        .ok()?;

        eprintln!("  [llm] extract_params: HTTP ответ за {:.1}с, читаем body...", t.elapsed().as_secs_f64());

        let body: OllamaResponse = resp.json().await.map_err(|e| {
            eprintln!("  [llm] extract_params: ошибка парсинга body: {e}");
        }).ok()?;

        eprintln!("  [llm] extract_params: ответ за {:.1}с: {}", t.elapsed().as_secs_f64(), body.response.trim());

        let content = body.response.trim();
        let json_str = content
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        let result: HashMap<String, String> = serde_json::from_str(json_str).map_err(|e| {
            eprintln!("  [llm] extract_params: ошибка JSON: {e}, raw: {json_str}");
        }).ok()?;

        Some(result)
    }

    /// Определяет подходящий скилл и параметры через Ollama.
    pub async fn match_skill(
        &self,
        text: &str,
        skills: &[SkillDef],
    ) -> Option<IntentMatch> {
        if !self.available || skills.is_empty() {
            eprintln!("  [llm] match_skill: недоступна или нет скиллов");
            return None;
        }

        let skills_desc: Vec<String> = skills
            .iter()
            .filter(|s| !s.triggers.is_empty())
            .map(|s| {
                let params_desc: Vec<String> = s
                    .params
                    .iter()
                    .map(|p| {
                        let desc = p.description.as_deref().unwrap_or(&p.name);
                        let opt = if p.optional { " (опционально)" } else { "" };
                        format!("    - {}: {}{}", p.name, desc, opt)
                    })
                    .collect();
                let params_block = if params_desc.is_empty() {
                    String::new()
                } else {
                    format!("\n  параметры:\n{}", params_desc.join("\n"))
                };
                format!("  - {} (action: {}){}", s.name, s.action, params_block)
            })
            .collect();

        let prompt = format!(
            "Определи какой скилл подходит для текста и извлеки параметры.\n\
             Верни ТОЛЬКО JSON без пояснений.\n\
             Текст: \"{text}\"\n\
             Доступные скиллы:\n{skills}\n\
             Формат ответа: {{\"skill\": \"action_name\", \"params\": {{\"param\": \"value\"}}}}\n\
             Если ни один скилл не подходит — верни {{\"skill\": \"clipboard\"}}",
            text = text,
            skills = skills_desc.join("\n"),
        );

        let request = OllamaRequest {
            model: self.model.clone(),
            prompt,
            stream: false,
            options: OllamaOptions { temperature: 0.1, num_predict: Some(200) },
        };

        eprintln!("  [llm] match_skill: отправка запроса...");
        let t = std::time::Instant::now();

        let url = format!("{}/api/generate", self.url);
        let resp = tokio::time::timeout(
            Duration::from_secs(15),
            self.client.post(&url).json(&request).send(),
        )
        .await
        .map_err(|_| {
            eprintln!("  [llm] match_skill: таймаут 15с");
        })
        .ok()?
        .map_err(|e| {
            eprintln!("  [llm] match_skill: ошибка запроса: {e}");
        })
        .ok()?;

        eprintln!("  [llm] match_skill: HTTP ответ за {:.1}с, читаем body...", t.elapsed().as_secs_f64());

        let body: OllamaResponse = resp.json().await.map_err(|e| {
            eprintln!("  [llm] match_skill: ошибка парсинга body: {e}");
        }).ok()?;

        eprintln!("  [llm] match_skill: ответ за {:.1}с: {}", t.elapsed().as_secs_f64(), body.response.trim());

        let content = body.response.trim();
        let json_str = content
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        let mut intent: IntentMatch = serde_json::from_str(json_str).map_err(|e| {
            eprintln!("  [llm] match_skill: ошибка JSON: {e}, raw: {json_str}");
        }).ok()?;

        // Ollama может вернуть name или action — нормализуем к action
        if let Some(matched) = skills.iter().find(|s| s.name == intent.skill || s.action == intent.skill) {
            intent.skill = matched.action.clone();
            Some(intent)
        } else {
            eprintln!("  [llm] match_skill: неизвестный скилл '{}', фоллбэк в clipboard", intent.skill);
            Some(IntentMatch {
                skill: "clipboard".into(),
                params: HashMap::new(),
            })
        }
    }
}
