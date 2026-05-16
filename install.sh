#!/bin/bash
# install.sh — Установка Исин: зависимости, модели, сборка, конфиг.

set -e

WHISPER_MODEL="ggml-large-v3.bin"
WHISPER_URL="https://huggingface.co/ggerganov/whisper.cpp/resolve/main/${WHISPER_MODEL}"
OLLAMA_MODEL="qwen2.5:1.5b"
NOTES_DIR="$HOME/Documents/WB/AINotes"

info()  { echo -e "\033[1;34m→\033[0m $1"; }
ok()    { echo -e "\033[1;32m✓\033[0m $1"; }
warn()  { echo -e "\033[1;33m!\033[0m $1"; }
die()   { echo -e "\033[1;31m✗\033[0m $1"; exit 1; }

# --- Rust ---
if command -v cargo &>/dev/null; then
    ok "Rust $(rustc --version 2>/dev/null | awk '{print $2}')"
else
    info "Установка Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
    ok "Rust $(rustc --version | awk '{print $2}')"
fi

# --- brew ---
if ! command -v brew &>/dev/null; then
    die "Homebrew не найден. Установите: /bin/bash -c \"\$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\""
fi
ok "Homebrew"

# --- ffmpeg ---
if command -v ffmpeg &>/dev/null; then
    ok "ffmpeg"
else
    info "Установка ffmpeg..."
    brew install ffmpeg
    ok "ffmpeg"
fi

# --- Ollama ---
if command -v ollama &>/dev/null; then
    ok "Ollama"
else
    info "Установка Ollama..."
    brew install ollama
    ok "Ollama"
fi

# Запуск Ollama если не работает
if ! curl -s http://localhost:11434/api/tags >/dev/null 2>&1; then
    info "Запуск Ollama..."
    ollama serve &>/dev/null &
    OLLAMA_PID=$!
    for i in $(seq 1 30); do
        if curl -s http://localhost:11434/api/tags >/dev/null 2>&1; then
            break
        fi
        sleep 1
    done
    if ! curl -s http://localhost:11434/api/tags >/dev/null 2>&1; then
        die "Ollama не запустилась. Запустите вручную: ollama serve"
    fi
    ok "Ollama запущена (PID ${OLLAMA_PID})"
else
    ok "Ollama работает"
fi

# --- Ollama модель ---
if ollama list 2>/dev/null | grep -q "${OLLAMA_MODEL}"; then
    ok "Модель ${OLLAMA_MODEL}"
else
    info "Скачивание модели ${OLLAMA_MODEL} (~1GB)..."
    ollama pull "${OLLAMA_MODEL}"
    ok "Модель ${OLLAMA_MODEL}"
fi

# --- Whisper модель ---
if [ -f "${WHISPER_MODEL}" ]; then
    ok "Модель ${WHISPER_MODEL}"
else
    info "Скачивание Whisper модели ${WHISPER_MODEL} (~3GB)..."
    curl -L --progress-bar -o "${WHISPER_MODEL}" "${WHISPER_URL}"
    ok "Модель ${WHISPER_MODEL}"
fi

# --- Сборка ---
info "Сборка isin (release)..."
cargo build --release
ok "Бинарник: target/release/isin"

# --- config.toml ---
if [ -f "config.toml" ]; then
    ok "config.toml"
else
    info "Создание config.toml..."
    cat > config.toml << 'CONF'
# Исин — конфигурация

[whisper]
vocabulary = ["YouTrack", "JetBrains", "Jira", "Notion", "Slack"]

[ollama]
url = "http://localhost:11434"
model = "qwen2.5:1.5b"
auto_restart = true

[[skill]]
name = "clipboard"
triggers = []
action = "clipboard"

[[skill]]
name = "note"
triggers = ["(создать|создай|запиши|сделай|добавь) заметку"]
params = [
  { name = "title", description = "заголовок заметки" },
  { name = "text", description = "текст или содержимое заметки" }
]
action = "file_append"
target = "~/Documents/WB/AINotes"
filename_template = "{{date}}-{{slug}}.md"

[[skill]]
name = "reminder"
triggers = ["напомни", "поставь напоминание", "напомни мне"]
params = [
  { name = "task", description = "о чём напомнить, описание задачи" },
  { name = "date", description = "дата напоминания, формат YYYY-MM-DD" },
  { name = "time", description = "время напоминания, формат HH:MM", optional = true }
]
action = "reminder"
target = "~/Documents/WB/AINotes/reminders.md"

[[skill]]
name = "ask_claude"
triggers = ["спроси у клод(а|а)", "задай вопрос клод(у|а)"]
params = [
  { name = "query", description = "вопрос или задача для Claude" }
]
action = "claude"
target = "~/Documents/WB/AINotes/claude"
CONF
    ok "config.toml"
fi

# --- Директории для скиллов ---
mkdir -p "${NOTES_DIR}/claude"
ok "Директория ${NOTES_DIR}"

# --- Claude CLI (опционально) ---
if command -v claude &>/dev/null; then
    ok "Claude CLI"
else
    warn "Claude CLI не найден — скилл «спроси у Клода» будет недоступен"
    warn "Установите: https://docs.anthropic.com/en/docs/claude-code"
fi

echo ""
echo -e "\033[1;32mГотово!\033[0m"
echo ""
echo "Запуск демона:"
echo "  ./target/release/isin"
echo ""
echo "Нажмите Cmd+Shift+Space для записи голоса."
echo "Ctrl+C — выход."
