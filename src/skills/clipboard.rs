use std::collections::HashMap;
use std::process::Stdio;

use crate::skills::{Skill, SkillError};

/// Фоллбэк-скилл: копирует распознанный текст в буфер обмена через pbcopy.
pub struct ClipboardSkill;

impl Skill for ClipboardSkill {
    fn name(&self) -> &str {
        "clipboard"
    }

    fn execute(&self, text: &str, _params: &HashMap<String, String>) -> Result<String, SkillError> {
        let mut child = std::process::Command::new("pbcopy")
            .stdin(Stdio::piped())
            .spawn()
            .map_err(|e| SkillError::Execution(format!("pbcopy: {e}")))?;

        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            stdin
                .write_all(text.as_bytes())
                .map_err(|e| SkillError::Execution(format!("запись в pbcopy: {e}")))?;
        }

        child
            .wait()
            .map_err(|e| SkillError::Execution(format!("pbcopy wait: {e}")))?;

        Ok(format!("Скопировано в буфер: {text}"))
    }
}
