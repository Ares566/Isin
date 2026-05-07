#!/bin/bash
# Установка зависимостей Исин: ffmpeg, Ollama, модели whisper и qwen.

set -e

WHISPER_MODEL="ggml-large-v3.bin"
WHISPER_URL="https://huggingface.co/ggerganov/whisper.cpp/resolve/main/${WHISPER_MODEL}"
OLLAMA_MODEL="qwen2.5:1.5b"

info()  { echo -e "\033[1;34m→\033[0m $1"; }
ok()    { echo -e "\033[1;32m✓\033[0m $1"; }
warn()  { echo -e "\033[1;33m!\033[0m $1"; }
die()   { echo -e "\033[1;31m✗\033[0m $1"; exit 1; }

# --- brew ---
if ! command -v brew &>/dev/null; then
    die "Homebrew не найден. Установите: /bin/bash -c \"\$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\""
fi
ok "Homebrew"

# --- ffmpeg ---
if command -v ffmpeg &>/dev/null; then
    ok "ffmpeg уже установлен"
else
    info "Установка ffmpeg..."
    brew install ffmpeg
    ok "ffmpeg"
fi

# --- Ollama ---
if command -v ollama &>/dev/null; then
    ok "Ollama уже установлена"
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
    # Ждём пока поднимется
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
    ok "Модель ${OLLAMA_MODEL} уже скачана"
else
    info "Скачивание модели ${OLLAMA_MODEL} (может занять пару минут)..."
    ollama pull "${OLLAMA_MODEL}"
    ok "Модель ${OLLAMA_MODEL}"
fi

# --- Whisper модель ---
if [ -f "${WHISPER_MODEL}" ]; then
    ok "Модель ${WHISPER_MODEL} уже есть"
else
    info "Скачивание Whisper модели ${WHISPER_MODEL} (~3GB)..."
    curl -L --progress-bar -o "${WHISPER_MODEL}" "${WHISPER_URL}"
    ok "Модель ${WHISPER_MODEL}"
fi

echo ""
echo -e "\033[1;32mГотово!\033[0m Запустите: ./target/release/isin"
