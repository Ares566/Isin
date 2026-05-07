use std::collections::HashMap;

use regex::Regex;

use crate::config::SkillDef;
use crate::llm::OllamaClient;

/// Результат матчинга текста против скилла.
#[derive(Debug)]
pub struct Match {
    /// Скилл, который matched.
    pub skill: SkillDef,
}

/// Пытается найти скилл, соответствующий тексту по триггерам (regex).
///
/// Перебирает скиллы по порядку. Первый matched скилл побеждает.
/// Возвращает None если ни один триггер не сработал.
pub fn match_skill(text: &str, skills: &[SkillDef]) -> Option<Match> {
    let text_lower = text.to_lowercase();

    for skill in skills {
        if skill.triggers.is_empty() {
            continue;
        }

        for trigger in &skill.triggers {
            // TODO сделать Regex::new всех паттернов единожды на старте, для скорости.
            let Ok(re) = Regex::new(&format!("(?i){trigger}")) else {
                continue;
            };
            if re.is_match(&text_lower) {
                return Some(Match {
                    skill: skill.clone(),
                });
            }
        }
    }

    None
}

/// Извлекает параметры из текста. Сначала через Ollama, потом regex fallback.
pub async fn extract_params(
    text: &str,
    skill: &SkillDef,
    ollama: &Option<OllamaClient>,
) -> HashMap<String, String> {
    if skill.params.is_empty() {
        return HashMap::new();
    }

    // Ollama — приоритетный способ
    if let Some(ref client) = ollama {
        if let Some(params) = client.extract_params(text, &skill.params).await {
            println!("Ollama extracted params {:?}", params);
            return params;
        }
    }

    // Fallback: regex по pattern
    extract_params_regex(text, &skill.params)
}

/// Regex-извлечение параметров (fallback).
fn extract_params_regex(text: &str, rules: &[crate::config::ParamRule]) -> HashMap<String, String> {
    let mut params = HashMap::new();
    for rule in rules {
        let Some(ref pattern) = rule.pattern else {
            continue;
        };
        let Ok(re) = Regex::new(pattern) else {
            continue;
        };
        if let Some(caps) = re.captures(text) {
            let value = caps
                .get(1)
                .map(|m| m.as_str().trim().to_string())
                .unwrap_or_else(|| caps.get(0).unwrap().as_str().trim().to_string());
            params.insert(rule.name.clone(), value);
        }
    }
    params
}
