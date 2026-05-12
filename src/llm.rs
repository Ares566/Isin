use std::collections::HashMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::config::{ParamRule, SkillDef};

const MAX_RETRIES: u32 = 3;
const BASE_DELAY: Duration = Duration::from_secs(1);

/// Клиент для Ollama API.
pub struct OllamaClient {
    url: String,
    model: String,
    client: reqwest::Client,
    available: bool,
    auto_restart: bool,
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

/// Ответ Ollama API (поле response).
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
    /// Создаёт клиент с заданным URL, моделью и флагом авторестарта.
    pub fn new(url: &str, model: &str, auto_restart: bool) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_default();
        Self {
            url: url.to_string(),
            model: model.to_string(),
            client,
            available: true,
            auto_restart,
        }
    }

    /// Проверяет доступность Ollama: ping + тестовый запрос к модели.
    /// Если модель не отвечает и auto_restart=true — перезапускает Ollama.
    pub async fn ensure_available(&mut self) -> bool {
        // 1. Ping: доступен ли сервер?
        let ping_ok = self.ping().await;
        if !ping_ok {
            if self.auto_restart {
                println!("  [llm] Ollama не отвечает, перезапуск...");
                if self.restart_ollama().await {
                    self.available = self.health_check().await;
                } else {
                    self.available = false;
                }
            } else {
                self.available = false;
            }
            return self.available;
        }

        // 2. Deep check: может ли модель генерировать?
        self.available = self.health_check().await;
        if !self.available && self.auto_restart {
            println!("  [llm] Ollama модель не работает, перезапуск...");
            if self.restart_ollama().await {
                self.available = self.health_check().await;
            }
        }

        self.available
    }

    /// Проверяет что сервер отвечает на /api/tags.
    async fn ping(&self) -> bool {
        let url = format!("{}/api/tags", self.url);
        match self.client.get(&url).send().await {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }

    /// Тестовый запрос к модели: генерация 1 токена.
    async fn health_check(&self) -> bool {
        let request = OllamaRequest {
            model: self.model.clone(),
            prompt: "ok".into(),
            stream: false,
            options: OllamaOptions {
                temperature: 0.1,
                num_predict: Some(1),
            },
        };

        let url = format!("{}/api/generate", self.url);
        match tokio::time::timeout(
            Duration::from_secs(30),
            self.client.post(&url).json(&request).send(),
        )
        .await
        {
            Ok(Ok(resp)) => {
                if let Ok(raw) = resp.text().await {
                    // Проверяем что нет ошибки
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw) {
                        if val.get("error").is_some() {
                            eprintln!("  [llm] health_check: модель вернула ошибку: {}", &raw[..raw.len().min(200)]);
                            return false;
                        }
                    }
                    // Есть поле response — значит модель работает
                    raw.contains("\"response\"")
                } else {
                    false
                }
            }
            Ok(Err(e)) => {
                eprintln!("  [llm] health_check: HTTP ошибка: {e}");
                false
            }
            Err(_) => {
                eprintln!("  [llm] health_check: таймаут 30с");
                false
            }
        }
    }

    /// Перезапускает Ollama: kill → wait → serve.
    async fn restart_ollama(&self) -> bool {
        // Убиваем существующий процесс
        let _ = std::process::Command::new("pkill")
            .arg("ollama")
            .output();
        tokio::time::sleep(Duration::from_secs(3)).await;

        // Запускаем в фоне
        match std::process::Command::new("ollama")
            .arg("serve")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(_) => {
                println!("  [llm] Ollama запускается...");
                tokio::time::sleep(Duration::from_secs(5)).await;
                // Проверяем что поднялась
                self.ping().await
            }
            Err(e) => {
                eprintln!("  [llm] не удалось запустить Ollama: {e}");
                false
            }
        }
    }

    /// Отправляет prompt к /api/generate с ретраями (3 попытки, экспоненциальная задержка).
    /// Возвращает очищенный от markdown-обёрток текст ответа.
    async fn generate(&self, prompt: &str, num_predict: u32) -> Option<String> {
        let request = OllamaRequest {
            model: self.model.clone(),
            prompt: prompt.to_string(),
            stream: false,
            options: OllamaOptions {
                temperature: 0.1,
                num_predict: Some(num_predict),
            },
        };

        for attempt in 0..MAX_RETRIES {
            if attempt > 0 {
                let delay = BASE_DELAY * 2u32.pow(attempt - 1);
                eprintln!(
                    "  [llm] ретрай {}/{} через {:.1}с",
                    attempt + 1,
                    MAX_RETRIES,
                    delay.as_secs_f64()
                );
                tokio::time::sleep(delay).await;
            }

            let url = format!("{}/api/generate", self.url);
            let t = std::time::Instant::now();

            let resp = match tokio::time::timeout(
                Duration::from_secs(15),
                self.client.post(&url).json(&request).send(),
            )
            .await
            {
                Ok(Ok(r)) => r,
                Ok(Err(e)) => {
                    eprintln!("  [llm] ошибка HTTP: {e}");
                    continue;
                }
                Err(_) => {
                    eprintln!("  [llm] таймаут 15с");
                    continue;
                }
            };

            eprintln!("  [llm] HTTP за {:.1}с", t.elapsed().as_secs_f64());

            // Читаем body как текст для диагностики
            let raw = match resp.text().await {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("  [llm] ошибка чтения body: {e}");
                    continue;
                }
            };

            eprintln!("  [llm] raw: {}", &raw[..raw.len().min(500)]);

            // Проверяем ошибку Ollama {"error": "..."}
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw) {
                if let Some(err) = val.get("error").and_then(|v| v.as_str()) {
                    eprintln!("  [llm] Ollama error: {err}");
                    continue;
                }
            }

            let body: OllamaResponse = match serde_json::from_str(&raw) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("  [llm] ошибка JSON: {e}");
                    continue;
                }
            };

            let content = body.response.trim();
            let cleaned = content
                .trim_start_matches("```json")
                .trim_start_matches("```")
                .trim_end_matches("```")
                .trim();

            eprintln!(
                "  [llm] ответ за {:.1}с: {content}",
                t.elapsed().as_secs_f64()
            );

            return Some(cleaned.to_string());
        }

        eprintln!("  [llm] все {} попытки исчерпаны", MAX_RETRIES);
        None
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

        let json_str = self.generate(&prompt, 200).await?;

        let result: HashMap<String, String> =
            serde_json::from_str(&json_str)
                .map_err(|e| {
                    eprintln!("  [llm] extract_params: ошибка JSON: {e}, raw: {json_str}");
                })
                .ok()?;

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

        let json_str = self.generate(&prompt, 200).await?;

        let mut intent: IntentMatch =
            serde_json::from_str(&json_str)
                .map_err(|e| {
                    eprintln!("  [llm] match_skill: ошибка JSON: {e}, raw: {json_str}");
                })
                .ok()?;

        // Ollama может вернуть name или action — нормализуем к action
        if let Some(matched) = skills
            .iter()
            .find(|s| s.name == intent.skill || s.action == intent.skill)
        {
            intent.skill = matched.action.clone();
            Some(intent)
        } else {
            eprintln!(
                "  [llm] match_skill: неизвестный скилл '{}', фоллбэк в clipboard",
                intent.skill
            );
            Some(IntentMatch {
                skill: "clipboard".into(),
                params: HashMap::new(),
            })
        }
    }
}
