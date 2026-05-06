# Исин — голосовой ассистент для macOS

**Исин** — CLI-демон для macOS (Apple Silicon), который слушает горячую клавишу, записывает речь, транскрибирует локально через Whisper и выполняет действия через настраиваемую систему скиллов. Всё работает локально, без облака.

Заточен под русский язык.

## Архитектура

```
горячая клавиша → запись с микрофона → транскрибация (whisper-rs)
                                              ↓
                                         роутер (pattern matching)
                                         ↙              ↘
                                   matched               fallback
                                      ↓                     ↓
                                extract params         LLM API (опционально)
                                      ↓                  ↓
                                execute skill      intent → skill
                                      ↓
                                результат (уведомление / тихо)
```

## Система скиллов

Скиллы описываются в `config.toml`. Каждый скилл имеет триггерные фразы, правила извлечения параметров и тип действия.

Пример конфигурации:

```toml
[[skill]]
name = "create_task"
triggers = ["создай задачу", "новая задача"]
params = [
  { name = "title", pattern = "(?:создай задачу|новая задача) (.+?)(?: в |$)" },
  { name = "project", pattern = "в (.+?)$", optional = true }
]
action = "youtrack"
confirm = true

[[skill]]
name = "note"
triggers = ["запиши", "заметка"]
params = [
  { name = "text", pattern = "(?:запиши|заметка) (.+)" }
]
action = "file_append"
target = "~/notes/voice-notes.md"

[[skill]]
name = "clipboard"
triggers = []          # пусто = скилл по умолчанию (фоллбэк)
action = "clipboard"   # просто вставить текст в буфер обмена
```

## Структура проекта

```
src/
├── main.rs          # точка входа, цикл демона (hotkey loop)
├── audio.rs         # захват звука с микрофона (cpal)
├── transcriber.rs   # транскрибация через whisper-rs
├── router.rs        # матчинг текста → скилл
├── skills/
│   ├── mod.rs       # trait Skill + реестр скиллов
│   ├── clipboard.rs # фоллбэк — текст в буфер
│   ├── youtrack.rs  # API-вызовы YouTrack
│   ├── file.rs      # запись в файл
│   └── shell.rs     # запуск shell-команды
├── config.rs        # парсинг TOML-конфига
└── llm.rs           # обёртка над LLM API (опционально, позже)
```

## Trait Skill

```rust
trait Skill {
    fn matches(&self, text: &str) -> bool;
    fn extract_params(&self, text: &str) -> HashMap<String, String>;
    fn execute(&self, params: HashMap<String, String>) -> Result<String>;
    fn confirm_prompt(&self, params: &HashMap<String, String>) -> Option<String>;
}
```

## Шаги реализации

### Шаг 1: Базовая транскрибация (уже работает)
- [x] whisper-rs транскрибация аудиофайлов в текст
- [x] CLI-интерфейс для обработки файлов

### Шаг 2: Захват с микрофона + хоткей
- [ ] Модуль `audio.rs` — запись с микрофона через cpal (16kHz mono)
- [ ] Детекция тишины (стоп по паузе в речи)
- [ ] Глобальный хоткей через CGEventTap (macOS)
- [ ] Основной цикл демона: хоткей → запись → транскрибация → вывод в консоль

### Шаг 3: Роутер и система скиллов
- [ ] Парсинг `config.toml` со списком скиллов
- [ ] `router.rs` — pattern matching по триггерным фразам (regex)
- [ ] Извлечение параметров из текста по правилам из конфига
- [ ] `trait Skill` + реестр скиллов
- [ ] Скилл-фоллбэк: текст в буфер обмена

### Шаг 4: Базовые скиллы
- [ ] `clipboard` — текст в буфер обмена + автопаст
- [ ] `file_append` — дописать текст в файл
- [ ] `shell` — выполнить shell-команду
- [ ] `open` — открыть файл/URL
- [ ] Подтверждение перед выполнением (опционально)

### Шаг 5: LLM-обёртка (опционально)
- [ ] Интеграция с Claude API / Ollama
- [ ] Фоллбэк на LLM если pattern matching не сработал
- [ ] Использование контекста сессии в промптах
- [ ] Извлечение intent через LLM для сложных команд

### Шаг 6: Интеграции (по необходимости)
- [ ] YouTrack — создание/поиск задач
- [ ] Notion — создание заметок
- [ ] Другие API через настраиваемые скиллы

## Зависимости

```toml
whisper-rs = "0.16"    # транскрибация (whisper.cpp bindings)
cpal = "0.15"          # захват звука с микрофона
tokio = "1"            # async runtime
serde = "1"            # сериализация
serde_json = "1"       # JSON парсинг
toml = "0.8"           # конфигурация
regex = "1"            # pattern matching для триггеров
reqwest = "0.12"       # HTTP-клиент (для LLM и API-скиллов)
clap = "4"             # CLI аргументы
core-graphics = "0.24" # глобальный хоткей (macOS)
```

## Требования

- macOS с Apple Silicon (M1/M2/M3/M4)
- Rust 1.75+
- Whisper модель (ggml-large-v3.bin)
- Для LLM: API-ключ Claude или запущенный Ollama

## Запуск

```bash
# Сборка
cargo build --release

# Запуск демона
./target/release/isin

# С указанием конфига
./target/release/isin --config path/to/config.toml

# Тестовая транскрибация файла
./target/release/isin --file audio.mp3
```
