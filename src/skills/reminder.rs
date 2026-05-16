use std::collections::HashMap;
use std::path::PathBuf;

use crate::skills::{Skill, SkillError};

/// Скилл reminder: добавляет задачу с датой в формате obsidian-reminder.
///
/// Формат: `- [ ] текст задачи (@YYYY-MM-DD HH:MM)`
pub struct ReminderSkill;

impl Skill for ReminderSkill {
    fn name(&self) -> &str {
        "reminder"
    }

    fn execute(&self, _text: &str, params: &HashMap<String, String>) -> Result<String, SkillError> {
        let target = params
            .get("_target")
            .ok_or_else(|| SkillError::Execution("не указан файл для напоминаний (target)".into()))?;

        let task = params
            .get("task")
            .or_else(|| params.get("text"))
            .ok_or_else(|| SkillError::Execution("не указано описание задачи".into()))?;

        let date = params
            .get("date")
            .ok_or_else(|| SkillError::Execution("не указана дата напоминания".into()))?;

        let time = params.get("time").map(|t| format!(" {t}")).unwrap_or_default();

        // Формат obsidian-reminder: - [ ] task (@YYYY-MM-DD HH:MM)
        let line = format!("- [ ] {task} (@{date}{time})");

        let filepath = expand_tilde(target);

        // Создаём директорию если нет
        if let Some(parent) = filepath.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| SkillError::Execution(format!("ошибка создания директории: {e}")))?;
        }

        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&filepath)
            .map_err(|e| SkillError::Execution(format!("ошибка открытия файла: {e}")))?;

        writeln!(file, "{line}")
            .map_err(|e| SkillError::Execution(format!("ошибка записи: {e}")))?;

        Ok(format!("Напоминание: {line}"))
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
