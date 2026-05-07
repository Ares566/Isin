use std::collections::HashMap;
use std::path::PathBuf;

use crate::skills::{Skill, SkillError};

/// Шаблоны для подстановки в имя файла.
const TEMPLATE_VARS: &[&str] = &["{{date}}", "{{title}}", "{{slug}}", "{{time}}"];

/// Скилл file_append: создаёт/дополняет файл в указанной директории.
pub struct FileAppendSkill;

impl Skill for FileAppendSkill {
    fn name(&self) -> &str {
        "file_append"
    }

    fn execute(&self, text: &str, params: &HashMap<String, String>) -> Result<String, SkillError> {
        let target = params
            .get("_target")
            .ok_or_else(|| SkillError::Execution("не указана директория (target)".into()))?;

        let default_template = "{{date}}.md".to_string();
        let filename_template = params
            .get("_filename_template")
            .unwrap_or(&default_template);

        let dir = expand_tilde(target);
        std::fs::create_dir_all(&dir)
            .map_err(|e| SkillError::Execution(format!("ошибка создания директории: {e}")))?;

        let filename = render_template(filename_template, params);
        let filepath = dir.join(&filename);

        let content = build_content(text, params);

        // Дописываем в файл или создаём новый
        let existed = filepath.exists();
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&filepath)
            .map_err(|e| SkillError::Execution(format!("ошибка открытия файла: {e}")))?;

        // Если файл новый — пишем заголовок
        if !existed {
            if let Some(title) = params.get("title") {
                writeln!(file, "# {title}\n")
                    .map_err(|e| SkillError::Execution(format!("ошибка записи: {e}")))?;
            }
        }

        writeln!(file, "{content}")
            .map_err(|e| SkillError::Execution(format!("ошибка записи: {e}")))?;

        Ok(format!(
            "{}: {}",
            if existed { "Дописано в" } else { "Создан" },
            filepath.display()
        ))
    }
}

/// Рендерит шаблон имени файла, подставляя переменные.
fn render_template(template: &str, params: &HashMap<String, String>) -> String {
    let mut result = template.to_string();
    for var in TEMPLATE_VARS {
        let key = &var[2..var.len() - 2]; // снимаем {{ }}
        let value = params.get(key).cloned().unwrap_or_default();
        result = result.replace(var, &value);
    }
    // Очищаем неиспользованные переменные
    result
}

/// Формирует содержимое для записи.
fn build_content(text: &str, params: &HashMap<String, String>) -> String {
    if let Some(note_text) = params.get("text") {
        note_text.clone()
    } else {
        text.to_string()
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
