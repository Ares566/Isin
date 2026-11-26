#!/usr/bin/env bash
# Установка Исин из корня репозитория.

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PREFIX="${ISIN_PREFIX:-$HOME/.local}"
BIN_DIR="$PREFIX/bin"
DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"
DATA_DIR="$DATA_HOME/isin"
CONFIG_DIR="$CONFIG_HOME/isin"
WHISPER_MODEL="ggml-large-v3.bin"
MODEL_PATH="$DATA_DIR/$WHISPER_MODEL"
MODEL_PART="$MODEL_PATH.part"
WHISPER_URL="https://huggingface.co/ggerganov/whisper.cpp/resolve/main/$WHISPER_MODEL"
CONFIG_PATH="$CONFIG_DIR/config.toml"

info() { echo -e "\033[1;34m→\033[0m $1"; }
ok() { echo -e "\033[1;32m✓\033[0m $1"; }
die() { echo -e "\033[1;31m✗\033[0m $1" >&2; exit 1; }

[[ "$(uname -s)" == "Darwin" ]] || die "Исин поддерживает только macOS."
[[ "$(uname -m)" == "arm64" ]] || die "Нужен Mac с Apple Silicon (arm64)."
[[ -f "$SCRIPT_DIR/Cargo.toml" ]] || die "Запустите установщик из репозитория Исин."

if ! xcrun --find clang >/dev/null 2>&1; then
    xcode-select --install >/dev/null 2>&1 || true
    die "Установите Command Line Tools и повторите: xcode-select --install"
fi
ok "Xcode Command Line Tools"

if ! command -v brew >/dev/null 2>&1; then
    die "Нужен Homebrew: https://brew.sh"
fi

packages=()
for package in cmake pkg-config ffmpeg; do
    if ! command -v "$package" >/dev/null 2>&1; then
        packages+=("$package")
    fi
done
if (( ${#packages[@]} > 0 )); then
    info "Установка зависимостей: ${packages[*]}..."
    brew install "${packages[@]}"
fi
ok "cmake, pkg-config и ffmpeg"

if ! command -v cargo >/dev/null 2>&1; then
    info "Установка Rust..."
    curl --proto '=https' --tlsv1.2 --fail --silent --show-error https://sh.rustup.rs | sh -s -- -y
    # shellcheck disable=SC1091
    source "$HOME/.cargo/env"
fi
ok "Rust $(rustc --version | awk '{print $2}')"

mkdir -p "$BIN_DIR" "$DATA_DIR" "$CONFIG_DIR"

if [[ -s "$MODEL_PATH" ]]; then
    ok "Модель Whisper уже установлена"
elif [[ -s "$SCRIPT_DIR/$WHISPER_MODEL" ]]; then
    info "Копирование локальной модели Whisper..."
    cp "$SCRIPT_DIR/$WHISPER_MODEL" "$MODEL_PATH"
    ok "Модель: $MODEL_PATH"
else
    info "Скачивание Whisper large-v3 (~3GB)..."
    curl --fail --location --retry 3 --continue-at - --progress-bar \
        --output "$MODEL_PART" "$WHISPER_URL"
    mv "$MODEL_PART" "$MODEL_PATH"
    ok "Модель: $MODEL_PATH"
fi

info "Сборка release-версии..."
cd "$SCRIPT_DIR"
export BINDGEN_EXTRA_CLANG_ARGS="--target=arm64-apple-darwin${BINDGEN_EXTRA_CLANG_ARGS:+ $BINDGEN_EXTRA_CLANG_ARGS}"
cargo build --release
install -m 755 "$SCRIPT_DIR/target/release/isin" "$BIN_DIR/isin"
ok "Команда: $BIN_DIR/isin"

if [[ -f "$CONFIG_PATH" ]]; then
    ok "Существующий конфиг сохранён"
else
    install -m 644 "$SCRIPT_DIR/config.toml" "$CONFIG_PATH"
    ok "Конфиг: $CONFIG_PATH"
fi

echo ""
echo -e "\033[1;32mУстановка завершена!\033[0m"
echo ""

case ":$PATH:" in
    *":$BIN_DIR:"*)
        echo "Запуск: isin"
        ;;
    *)
        echo "Добавьте каталог в PATH:"
        echo "  export PATH=\"$BIN_DIR:\$PATH\""
        echo "Затем запустите: isin"
        ;;
esac

echo "Удерживайте Fn для записи, отпустите для транскрибации."
echo "При первом запуске разрешите доступ к микрофону, мониторингу ввода, универсальному доступу и уведомлениям."
