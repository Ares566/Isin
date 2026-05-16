# Исин — голосовой ассистент для macOS

**Исин** — CLI-демон для macOS (Apple Silicon). Нажимаешь `Cmd+Shift+Space`, говоришь фразу — она распознаётся локально через Whisper и выполняется через настраиваемую систему скиллов. Всё работает на устройстве, без облака.

Заточен под русский язык.

## Возможности

- **Голос → текст** — локальная транскрибация через whisper-rs (ggml-large-v3)
- **Роутер скиллов** — regex-триггеры + fallback на Ollama для intent recognition
- **Извлечение параметров** — Ollama (приоритет) или regex-fallback
- **Настраиваемые скиллы** через TOML-конфиг
- **Интеграция с Claude** — «спроси у Клода» отправляет вопрос локальному Claude CLI

### Скиллы из коробки

| Скилл | Триггер | Действие |
|---|---|---|
| `clipboard` | фоллбэк (нет триггера) | текст в буфер обмена |
| `file_append` | «создай заметку» | MD-файл в указанной директории |
| `reminder` | «напомни» | задача в obsidian-reminder формате |
| `claude` | «спроси у клода» | вопрос → Claude CLI → ответ в MD-файл + уведомление |

## Архитектура

```
Cmd+Shift+Space
       ↓
  запись с микрофона (cpal, 16kHz mono)
       ↓
  транскрибация (whisper-rs, локально)
       ↓
  роутер
  ├── regex-триггеры из config.toml
  └── Ollama intent recognition (fallback)
       ↓
  извлечение параметров (Ollama → regex fallback)
       ↓
  execute skill → результат
```

## Быстрый старт

```bash
# Клонируем
git clone https://github.com/Ares566/Isin.git
cd Isin

# Установка зависимостей, сборка, запуск
bash install.sh
```

`install.sh` установит всё необходимое: Rust, ffmpeg, Ollama, скачает модели Whisper и Qwen, соберёт бинарник.

После установки:

```bash
./target/release/isin
```

Нажми `Cmd+Shift+Space`, скажи фразу, отпусти — Исин распознает и выполнит действие.

## Конфигурация

Файл `config.toml` создаётся автоматически при первом запуске `install.sh`.

```toml
[whisper]
vocabulary = ["YouTrack", "JetBrains", "Jira"]

[ollama]
url = "http://localhost:11434"
model = "qwen2.5:1.5b"
auto_restart = true

# Фоллбэк — текст в буфер обмена
[[skill]]
name = "clipboard"
triggers = []
action = "clipboard"

# Заметка в файл
[[skill]]
name = "note"
triggers = ["(создать|создай|запиши|сделай|добавь) заметку"]
params = [
  { name = "title", description = "заголовок заметки" },
  { name = "text", description = "текст заметки" }
]
action = "file_append"
target = "~/Documents/WB/AINotes"
filename_template = "{{date}}-{{slug}}.md"

# Напоминание (obsidian-reminder формат)
[[skill]]
name = "reminder"
triggers = ["напомни", "поставь напоминание"]
params = [
  { name = "task", description = "о чём напомнить" },
  { name = "date", description = "дата, формат YYYY-MM-DD" },
  { name = "time", description = "время, формат HH:MM", optional = true }
]
action = "reminder"
target = "~/Documents/WB/AINotes/reminders.md"

# Вопрос Claude (нужен установленный Claude CLI)
[[skill]]
name = "ask_claude"
triggers = ["спроси у клод(а|а)", "задай вопрос клод(у|а)"]
params = [
  { name = "query", description = "вопрос или задача для Claude" }
]
action = "claude"
target = "~/Documents/WB/AINotes/claude"
```

## CLI

```
isin [OPTIONS]

Опции:
  -f, --file <PATH>        Транскрибация аудиофайла (вместо демона)
  -m, --model <PATH>       Путь к модели Whisper [default: ggml-large-v3.bin]
  -l, --language <LANG>    Язык распознавания [default: ru]
  -o, --output <PATH>      Выходной MD-файл (только для --file)
  -c, --config <PATH>      Путь к конфигу [default: config.toml]
      --silence-stop <SEC>  Секунд тишины до остановки [default: 3]
      --min-speech <SEC>    Минимальная длительность речи [default: 3]
      --list-devices        Показать микрофоны и выйти
```

## Структура проекта

```
src/
├── main.rs          # точка входа, daemon loop + file mode
├── audio.rs         # захват с микрофона (cpal) + декодинг файлов (ffmpeg)
├── transcriber.rs   # whisper-rs обёртка
├── router.rs        # regex-матчинг текста → скилл
├── llm.rs           # Ollama клиент (extract_params, match_skill, retry)
├── config.rs        # парсинг TOML-конфига
├── hotkey.rs        # глобальный хоткей (rdev)
├── formatter.rs     # Markdown-форматирование (для --file)
└── skills/
    ├── mod.rs       # trait Skill + SkillRegistry
    ├── clipboard.rs # текст в буфер обмена (pbcopy)
    ├── file.rs      # file_append: создаёт/дополняет MD-файл
    ├── reminder.rs  # obsidian-reminder формат
    └── claude.rs    # вопрос → Claude CLI → ответ в файл
```

## Требования

- macOS с Apple Silicon (M1/M2/M3/M4)
- Rust 1.75+
- Whisper модель `ggml-large-v3.bin` (~3GB)
- Ollama с моделью `qwen2.5:1.5b` (для параметризации)
- Claude CLI (опционально, для скилла «спроси у Клода»)

## Добавление своего скилла

1. Создай файл `src/skills/my_skill.rs` с реализацией `trait Skill`
2. Зарегистрируй в `src/main.rs`: `registry.register(Box::new(MySkill))`
3. Опиши в `config.toml`:

```toml
[[skill]]
name = "my_skill"
triggers = ["мой триггер"]
params = [{ name = "param1", description = "описание параметра" }]
action = "my_skill"
target = "..."
```

## Лицензия

MIT
