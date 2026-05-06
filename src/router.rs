use std::collections::HashMap;

use regex::Regex;

use crate::config::SkillDef;

/// Результат матчинга текста против скилла.
pub struct Match {
    /// Скилл, который matched.
    pub skill: SkillDef,
    /// Извлечённые параметры.
    pub params: HashMap<String, String>,
}

/// Пытается найти скилл, соответствующий тексту.
///
/// Перебирает скиллы по порядку, проверяет триггеры (regex).
/// Первый matched скилл побеждает. Если ни один не matched —
/// возвращается фоллбэк (скилл с пустыми triggers).
pub fn match_skill<'a>(
    text: &str,
    skills: &'a [SkillDef],
    fallback: Option<&'a SkillDef>,
) -> Option<Match> {
    let text_lower = text.to_lowercase();

    for skill in skills {
        // Фоллбэк-скилл пропускаем — он возвращается если никто не matched
        if skill.triggers.is_empty() {
            continue;
        }

        for trigger in &skill.triggers {
            let Ok(re) = Regex::new(&format!("(?i){trigger}")) else {
                continue;
            };
            if re.is_match(&text_lower) {
                let params = extract_params(text, &skill.params);
                return Some(Match {
                    skill: skill.clone(),
                    params,
                });
            }
        }
    }

    // Никто не matched — фоллбэк
    fallback.map(|skill| Match {
        skill: skill.clone(),
        params: HashMap::new(),
    })
}

/// Извлекает параметры из текста по правилам из конфига.
fn extract_params(text: &str, rules: &[crate::config::ParamRule]) -> HashMap<String, String> {
    let mut params = HashMap::new();
    for rule in rules {
        let Ok(re) = Regex::new(&rule.pattern) else {
            continue;
        };
        if let Some(caps) = re.captures(text) {
            // Берём первую capture group, либо весь матч
            let value = caps
                .get(1)
                .map(|m| m.as_str().trim().to_string())
                .unwrap_or_else(|| caps.get(0).unwrap().as_str().trim().to_string());
            params.insert(rule.name.clone(), value);
        }
    }
    params
}
