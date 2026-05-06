pub mod clipboard;

use std::collections::HashMap;

/// Общий интерфейс скилла.
pub trait Skill {
    /// Имя скилла (совпадает с action в конфиге).
    fn name(&self) -> &str;

    /// Выполнить действие с переданными параметрами.
    fn execute(&self, text: &str, params: &HashMap<String, String>) -> Result<String, SkillError>;
}

/// Реестр скиллов: хранит все зарегистрированные скиллы по action name.
pub struct SkillRegistry {
    skills: HashMap<String, Box<dyn Skill>>,
    fallback: Option<Box<dyn Skill>>,
}

impl SkillRegistry {
    /// Создаёт пустой реестр.
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
            fallback: None,
        }
    }

    /// Регистрирует скилл по имени его action.
    pub fn register(&mut self, skill: Box<dyn Skill>) {
        let name = skill.name().to_string();
        self.skills.insert(name, skill);
    }

    /// Регистрирует фоллбэк-скилл (вызывается когда никто не matched).
    pub fn set_fallback(&mut self, skill: Box<dyn Skill>) {
        self.fallback = Some(skill);
    }

    /// Возвращает скилл по имени action.
    pub fn get(&self, action: &str) -> Option<&dyn Skill> {
        self.skills.get(action).map(|s| s.as_ref())
    }

}

#[derive(Debug)]
pub enum SkillError {
    Execution(String),
}

impl std::fmt::Display for SkillError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Execution(e) => write!(f, "ошибка выполнения скилла: {e}"),
        }
    }
}

impl std::error::Error for SkillError {}
