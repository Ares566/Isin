use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;

use crate::skills::{Skill, SkillError};

/// Скилл claude: отправляет вопрос локальному Claude CLI, сохраняет ответ в MD-файл.
pub struct ClaudeSkill;

impl Skill for ClaudeSkill {
    fn name(&self) -> &str {
        "claude"
    }

    fn execute(&self, text: &str, params: &HashMap<String, String>) -> Result<String, SkillError> {
        let query = params.get("query").map(|s| s.as_str()).unwrap_or(text);

        if query.trim().is_empty() {
            return Err(SkillError::Execution("пустой запрос к Claude".into()));
        }

        println!("  🤖 Отправляю вопрос Claude...");

        let output = std::process::Command::new("claude")
            .arg("-p")
            .arg(query)
            .output()
            .map_err(|e| SkillError::Execution(format!("ошибка запуска claude: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SkillError::Execution(format!("claude: {stderr}")));
        }

        let response = String::from_utf8_lossy(&output.stdout).trim().to_string();

        if response.is_empty() {
            return Err(SkillError::Execution("Claude вернул пустой ответ".into()));
        }

        // Сохраняем в файл
        let target = params
            .get("_target")
            .ok_or_else(|| SkillError::Execution("не указана директория (target)".into()))?;

        let dir = expand_tilde(target);
        std::fs::create_dir_all(&dir)
            .map_err(|e| SkillError::Execution(format!("ошибка создания директории: {e}")))?;

        let date = params.get("date").cloned().unwrap_or_default();
        let time = params.get("time").cloned().unwrap_or_default();
        let time_compact = time.replace(':', "");
        let filename = format!("{}-{}.md", date, time_compact);
        let filepath = dir.join(&filename);

        let mut file = std::fs::File::create(&filepath)
            .map_err(|e| SkillError::Execution(format!("ошибка создания файла: {e}")))?;

        let now = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
        writeln!(file, "# {query}\n")
            .map_err(|e| SkillError::Execution(format!("ошибка записи: {e}")))?;
        writeln!(file, "> {now}\n")
            .map_err(|e| SkillError::Execution(format!("ошибка записи: {e}")))?;
        write!(file, "{response}")
            .map_err(|e| SkillError::Execution(format!("ошибка записи: {e}")))?;

        // Уведомление macOS
        let summary: String = response.chars().take(80).collect();
        let _ = std::process::Command::new("osascript")
            .arg("-e")
            .arg(format!(
                "display notification \"{}\" with title \"Исин — Claude ответил\"",
                summary.replace('"', "\\\"").replace('\n', " ")
            ))
            .spawn();

        Ok(format!("Claude ответил: {}", filepath.display()))
    }
}

/// Раскрывает ~/ в домашнюю директорию.
fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = std::env::var("HOME").ok() {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(path)
}
