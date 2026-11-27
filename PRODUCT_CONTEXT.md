# Исин → Home Assistant Voice: контекст продукта

**Дата создания:** 2026-07-28
**Последнее обновление:** 2026-07-29 (добавлен подраздел 7.10 — совместимость с non-HA системами)
**Стадия:** pre-build validation (следующие 2 недели — проверка спроса, не код)

---

## 1. Что строим

**Локальный voice assistant для Home Assistant на базе Whisper для Raspberry Pi 5.**

Подписка ($9-15/мес) надстройкой к существующему HA. Не конкурирует с Nabu Casa (remote access $6.50/мес) — дополняет: локальный AI-voice, который не зависит от Google/Amazon и работает на устройстве.

**Ядро продукта:**
1. Push-to-talk (физическая кнопка или wake word) → Whisper локально на RP5
2. Маршрутизатор команд на русском языке → HA через WebSocket/REST API
3. Готовые типовые сценарии: свет, климат, медиа, сцены
4. Конфигурация под себя через TOML (как в текущем Исине)

**Что уже есть в Исине (переиспользуем):**
- whisper-rs + ggml-large-v3 локальная транскрибация
- Роутер скиллов: regex + Ollama fallback
- TOML-конфиг с описанием скиллов
- Заточенность под русский язык
- Rust + tokio + cpal стек

**Что нужно добавить для HA:**
- Перенос macOS → Linux на Raspberry Pi 5
- Замена rdev hotkey на push-to-talk через GPIO кнопку или wake word (Porcupine/openWakeWord)
- HA WebSocket API клиент (состояния устройств, вызов сервисов)
- Готовый набор HA-скиллов: свет, климат, media_player, сцены, группы
- Упаковка как HA Add-On для лёгкой установки
- Биллинг + лицензирование

## 2. Почему это может работать

### Рынок реальный и быстрорастущий
- Home Assistant: **2 миллиона домохозяйств** (апрель 2025), удвоение за год
- ~200 000 активных установок (analytics.home-assistant.io)
- Сообщество активно: forum.home-assistant.io, r/homeassistant, Telegram-каналы РФ

### Зазор в конкурентном ландшафте
- **Nabu Casa** ($6.50/мес) — фокус на remote access + Google/Alexa voice. **Локальный Whisper voice не делает.**
- **HA Assist (встроенный)** — работает, но требует cloud Wake Word + облачный STT. Локальный Whisper на edge — пустая ниша.
- **Wyoming Whisper protocol** — есть, но голос-команды поверх слабые.

### Соответствие профилю основателя
- YouTube-канал https://www.youtube.com/@RenatAbaidullin с IoT/DIY/HA аудиторией (заброшен, возрождается)
- 5 лет опыта с Whisper (ggml-large-v3, Ollama, whisper-rs)
- 10 лет backend архитектуры (Go, Rust)
- Опыт работы с Raspberry Pi 5, pose estimation, embedded
- Уже есть работающий прототип (Исин для macOS)

## 3. Конкуренты и дифференциация

| Решение | Цена | Что делает | Что НЕ делает |
|---|---|---|---|
| **Nabu Casa** | $6.50/мес | Remote access + Google/Alexa + backup | Локальный AI voice на edge |
| **HA Assist (built-in)** | Бесплатно | Voice commands | Локальный STT слабый, нужен cloud |
| **Tailscale/Cloudflare** | Бесплатно | Remote access | Никакого AI voice |
| **Yandex Smart Home** | Бесплатно | Voice через Алису | Только экосистема Yandex, не HA |
| **Исин (этот проект)** | $9-15/мес | Локальный Whisper voice + сценарии | Remote access (оставляем Nabu Casa) |

**Ключевое позиционирование:** надстройка к Nabu Casa, не замена. Подписчик Nabu Casa + Исин = $15-22/мес суммарно. Доступно для энтузиастов.

## 4. Целевая аудитория

**Основная:** продвинутые HA-пользователи с Raspberry Pi 5, у которых:
- Уже есть HA на RP5 (или планируют)
- Устали от Google/Alexa cloud зависимости
- Хотят локальный voice без прослушки
- Готовы платить за convenience ($10-15/мес для ENTHUSIAST — нормально)

**Вторичная:** DIY-сообщество на YouTube (аудитория канала), которое:
- Смотрит видео про HA, IoT, Raspberry Pi
- Хочет готовое решение, а не собирать самим
- Снизу воронки: от зрителя → к подписчику → к платящему

**Не наша аудитория:** новички, которым нужно «всё из коробки» (им Yandex/Apple HomeKit); enterprise (им нужно своё).

## 5. Ценовая модель и экономика

**Подписка:** $12/мес или $120/год (стандартная SaaS-вилка).

**Экономика на единицу:**
- Себестоимость инфраструктуры на клиента: $0.10-0.50/мес (edge-вычисления на устройстве клиента, cloud только для биллинга + телеметрии + обновления моделей)
- Чек: $12/мес
- Маржа: 95%+

**Путь к $5K MRR:**
| Месяц | Подписчики YouTube | Платящих | MRR ($12/мес) |
|---|---|---|---|
| 0 | 0 (старт возрождения) | 0 | $0 |
| 3 | 5K | 10-30 (ранние) | $120-360 |
| 6 | 15K | 50-100 | $600-1200 |
| 12 | 30-50K | 200-400 | $2400-4800 |
| 18 | 50-80K | 400-700 | $4800-8400 |

---

## 6. Техническая архитектура

### 6.1 Разделение зон с Nabu Casa (технически)

Nabu Casa — cloud-прослойка для **внешнего доступа** к HA (remote access, backup, Google/Alexa bridge, cloud Web Speech API). Наш продукт — **локальный** voice pipeline целиком на устройстве клиента. Зоны не пересекаются:

| Слой | Nabu Casa | Исин |
|---|---|---|
| Remote access (снаружи → HA) | Да | Нет |
| Cloud backup | Да | Нет |
| Google/Alexa bridge | Да | Нет |
| STT (распознавание речи) | Облако Google | Локально Whisper на RP5 |
| NLU/роутинг команд | Облако | Локально (regex + Ollama на RP5) |
| TTS (ответ голосом) | Облако | Локально Piper на RP5 |

**Совместимость:** подписчик Nabu Casa + Исин = $6.50 + $12 = $18.50/мес. Разные боли для одной и той же аудитории.

### 6.2 Edge-слой: поток данных

```
[GPIO-кнопка или wake word "Исин"]
        ↓ ~100ms
[Audio capture — cpal на Linux, 16kHz mono]
        ↓ ~1500ms (фраза 1-2 сек)
[Whisper STT — whisper-rs + ggml-large-v3, локально]
        ↓ ~2-3 сек на RP5
[Skill Router]
   ├─ regex: "выключи свет в {room}" → быстрый путь
   └─ Ollama qwen2.5:1.5b fallback — нестандартные команды
        ↓ ~50ms (regex) / ~500ms (Ollama)
[HA WebSocket Client — tungstenite]
        ↓ ~50ms
[HA Core → executes service call → device]
        ↓
[TTS — Piper русские голоса, локально]
        ↓ ~500ms
[Audio output — динамик / умная колонка]
```

**Полный latency: 4-5 секунд** от конца фразы до действия устройства. Сопоставимо с Alexa/Google (у них cloud round-trip в том же диапазоне).

### 6.3 Стек — что переиспользуем, что добавляем

**Уже есть в Исине (переиспользуем напрямую):**
- `whisper-rs` интеграция + ggml-large-v3
- Skill router (regex + Ollama fallback)
- TOML-конфиг
- `tokio` async runtime

**Замена для Linux:**
- `rdev` (macOS hotkey) → `gpio-cdev` (GPIO-кнопка на RP5) + `openWakeWord` subprocess для wake word
- `cpal` остаётся, настраивается под ALSA на RP5
- macOS pasteboard API → не нужен

**Добавляем новое:**
- `tungstenite` — HA WebSocket client
- `piper-rs` (или subprocess к piper binary) — TTS
- `reqwest` — уже есть, для license validation
- `ed25519-dalek` — подпись лицензии

**Новый skill domain:** вместо clipboard/reminder — `ha_light`, `ha_climate`, `ha_media_player`, `ha_scene`, `ha_group`. Каждый скилл мапит русскую команду на HA `service.call` через WebSocket.

---

## 7. Доставка продукта и распространение

### 7.1 Что доставляем — Docker-образ, multi-arch

**Канонический формат: Docker-образ, публикуется в container registry.** HA Supervisor автоматически ставит нужный вариант под архитектуру устройства. Не исходник, не бинарник.

**Почему не исходник + компиляция на устройстве:**
1. Rust-билд с `whisper-rs` тянет C++ toolchain + cmake + LLVM — 2GB на время сборки
2. Компиляция на RP5 (4-core Cortex-A76) — 12-20 минут, юзер не будет ждать
3. HA Add-On build system (`build_from_source: true`) существует, но сообщество её ненавидит

**Почему не голый бинарник:**
1. `whisper-rs` динамически линкует `libggml.so`
2. `cpal` на Linux хочет `libasound2`
3. Под каждую архитектуру — отдельный артефакт
4. Не convention HA ecosystem — там всё через Docker

### 7.2 Multi-arch сборка

GitHub Actions с `docker buildx`, публикуется в `ghcr.io/ares566/isin-ha:addon-x.y.z`:

- **`linux/arm64/v8`** — RP5 (4GB и 8GB), RP4. **95% юзеров здесь**
- **`linux/amd64`** — home lab сервера на Intel/AMD. **5% юзеров, важно для dev/test**
- **`arm/v7`** (RP3 и старье) — **НЕ поддерживаем**. Whisper large-v3 не выживет на 1GB RAM

Multi-stage Dockerfile:

```dockerfile
FROM rust:1.80-bookworm AS builder
RUN apt-get update && apt-get install -y \
    git cmake libclang-dev llvm-dev \
    libssl-dev pkg-config libasound2-dev
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/
RUN cargo build --release --bin isin-ha

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    libssl3 libasound2 ca-certificates tini \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/isin-ha /usr/local/bin/
COPY rootfs/ /usr/local/share/isin/
ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/isin-ha"]
```

**Размер финального образа: 80-120MB.** Без моделей.

### 7.3 Три слоя артефактов (раздельно)

Важно разделить три разных артефакта доставки:

#### Слой 1: Docker-образ (public, multi-arch)

В образе едут:
- `isin-ha` binary (Rust)
- Базовые free skills (свет, климат, status) — встроены в бинарник
- Минимальный config template
- Один piper-голос (базовый, для free tier)

#### Слой 2: Модели (public, тянутся при первом запуске)

Гигантские файлы, тащить в образ — безумие (образ был бы 3GB+). Тянутся отдельно:

| Модель | Размер | Откуда | Когда |
|---|---|---|---|
| Whisper ggml-large-v3 (Q5) | ~600MB | HuggingFace | Первый запуск |
| Whisper ggml-medium (fallback) | ~250MB | HuggingFace | Если large-v3 не тянет |
| Ollama qwen2.5:1.5b | ~1GB | ollama registry | Первый запуск |
| Piper русские голоса (irina, aleksandr) | ~150MB | GitHub Releases | Первый запуск |
| openWakeWord модели (v2) | ~50MB | GitHub Releases | Только для premium |

Хранятся в `/data/isin/models/` — HA Supervisor делает бакап этого volume автоматически.

**Все модели — public. Мы не продаём модели. Мы продаём pipeline, навыки, конфигурацию и обновления.**

#### Слой 3: Premium-контент (после валидации лицензии)

**Принятие:** ВСЕ skills в одном образе. Premium-функции разблокируются флагом из валидированной лицензии. Не делаем отдельный приватный образ — двойная установка, сложно.

```rust
match skill {
    Skill::Light => execute(...),           // free
    Skill::Climate => execute(...),         // free
    Skill::Scene => {
        if license.has_feature("scenes") {  // premium check
            execute(...)
        } else {
            tts_say("Сцены доступны по подписке")
        }
    }
}
```

Premium skill templates (готовые конфиги под конкретные устройства: xiaomi_vacuum, sonos_sync, и т.д.) — отдельный JSON-манифест, тянется с нашего сервера после валидации:

```
GET https://api.isin.io/v1/skills/manifest
Authorization: Bearer <license_token>
→ 200 OK
   [{"name": "xiaomi_vacuum", ...}, {"name": "sonos_sync", ...}, ...]
```

### 7.4 Trial / Free / Premium — механика

**Что в каком тире:**

| Тир | Что включает |
|---|---|
| **Free** | Push-to-talk через GPIO, 3 базовых скилла (свет on/off, climate set, status query), один голос TTS, русский язык |
| **Premium $12/мес** | Wake word (openWakeWord — hands-free), полная библиотека скиллов (медиа, сцены, шторы, роботы-пылесосы, группы), custom skill builder, multi-room sync, несколько голосов TTS, обновления Whisper-моделей, premium поддержка в Telegram, английский язык (после $5K MRR) |

**Почему именно так:** free tier достаточно хорош для реальной пользы, но недостаточен для полного комфорта — wake word и расширенные скиллы тянут на апгрейд.

**Trial без карты** — 14 дней полного premium. Снижает friction, HA-сообщество ненавидит paywall до того как попробовали.

**Offline grace period:** после последнего успешного `validate` Add-On работает 14 дней без интернета. Дальше HTTPS-запрос обязателен — иначе деградирует до free.

### 7.5 Cloud-бэкенд (наша сторона, минимальный)

Один Go-сервис на одной VPS (Hetzner CX21, €4/мес). Развёрнуто через docker-compose:

```
license-server/  (Go + chi)
├── POST /api/v1/device/register         → trial_id (14 дней)
├── POST /api/v1/license/validate        → {tier, expires_at, features[], signature}
├── POST /api/v1/billing/webhook         (Stripe или CloudPayments)
├── POST /api/v1/telemetry               (opt-in, агрегированные метрики)
├── GET  /api/v1/updates/manifest        (список версий моделей/скиллов)
├── GET  /api/v1/skills/manifest         (premium skill templates, gated)
│
├── PostgreSQL — licenses, subscriptions, devices
├── Redis — rate limit (1 validate/час на устройство)
└── S3-compatible storage — ggml модели, piper голоса
```

**Проверка лицензии технически:**

```rust
async fn check_license(license_key: &str) -> License {
    let cached = load_from_disk();
    if cached.fresh() { return cached; }        // кэш 1 час
    
    let resp = http.post("https://api.isin.io/v1/license/validate")
        .json({license_key, device_id})
        .send().await;
    
    match resp {
        Ok(license) => save_to_disk(license),
        Err(_) => load_offline_grace(),         // 14 дней
    }
}
```

Подпись ответа — Ed25519, чтобы убрать риск MITM на отзыв лицензии.

### 7.6 Стратегия открытости кода — open core

HA-сообщество **paranoia** по поводу закрытых бинарников в их сети. Любой тред «is this Add-On safe?» на forum.home-assistant.io подтверждает. Если код закрыт — нас заподозрят в лазейках для прослушки (что иронично для voice-продукта).

**Принятие: open core.**

**Открыто (MIT/Apache 2.0, public GitHub):**
- `isin-ha` core: audio capture, whisper integration, router framework, HA WebSocket client
- Базовые 3 skills (light, climate, status)
- License validator client
- Add-On metadata

**Закрыто (private repo или server-side):**
- Premium skill templates (xiaomi_vacuum, sonos_sync, и т.д.) — скачиваются только после валидации
- License server (наш Go-бэкенд)
- Billing

**Что это даёт:**
1. Доверие сообщества — возможность аудировать, что аудио не уходит наружу
2. Organic contributions в core
3. Защиту premium-сегмента через server-side gate

### 7.7 Полный flow покупки

```
1. Юзер видит видео на YouTube
2. Идёт в HA → Add-On Store → добавляет наш repository
3. Жмёт Install → HA Supervisor тянет ghcr.io/ares566/isin-ha:addon-1.0.0
   (100MB, ~2 минуты на RP5)
4. Add-On первый запуск:
   - POST /api/v1/device/register → trial_id (14 дней)
   - Скачиваются модели (1.5GB, ~5-10 минут на SSD)
5. Юзер настраивает через Add-On UI: GPIO-пин кнопки, язык, комнаты
6. Юзер пробует: «Исин, выключи свет на кухне» — работает
7. Через 14 дней trial кончается → free tier → если хочет wake word
   и premium skills — кликает «Subscribe» в Add-On UI
8. Stripe checkout (или CloudPayments для РФ) → webhook на наш сервер
   → license обновляется → Add-On в течение часа подхватывает
```

### 7.8 Объёмы и bandwidth на доставку

| Артефакт | Размер | Хостинг | Bandwidth |
|---|---|---|---|
| Docker image (arm64) | 100MB | ghcr.io | Бесплатно unlimited |
| Docker image (amd64) | 100MB | ghcr.io | Бесплатно unlimited |
| Whisper large-v3 Q5 | 600MB | HuggingFace | Бесплатно unlimited |
| Piper голоса (2 шт) | 300MB | GitHub Releases | Может упереться в лимит при росте |
| Ollama qwen2.5 | 1GB | Ollama registry | Их带宽 |

**Первая установка на RP5: ~2GB трафика, 5-10 минут на SSD, 15-20 минут на SD card** (дополнительный аргумент за требование SSD).

### 7.9 Целевые платформы HA

HA поддерживает **четыре** способа установки. Не все дают доступ к Add-Ons:

| Способ | Доля рынка | Add-Ons | Наш статус |
|---|---|---|---|
| **HA OS** (HA Operating System) | ~75-80% | Да, нативно через Supervisor | **Главный таргет** |
| **HA Container** (Docker на Linux) | ~15-20% | Нет, Supervisor отсутствует | Совместимость позже (Phase 2) |
| **HA Supervised** | ~3-5% | Да, но deprecated | Автоматически работает |
| **HA Core** (pip install) | <1% | Нет | Игнорируем |

Цифры — по [analytics.home-assistant.io](https://analytics.home-assistant.io/). Add-On работает «из коробки» на ~80% целевой аудитории.

**HA OS — это три слоя:**

```
┌──────────────────────────────────────┐
│  Add-Ons (Docker containers)        │  ← мы здесь
│   ├─ Isin Voice                      │
│   ├─ Mosquitto broker                │
│   └─ ...                             │
├──────────────────────────────────────┤
│  Home Assistant Core (Python)        │  ← "HA" как юзер его видит
├──────────────────────────────────────┤
│  Home Assistant Supervisor (Go)      │  ← управляет Add-Ons
├──────────────────────────────────────┤
│  HAOS (Buildroot Linux, read-only)   │  ← ОС
├──────────────────────────────────────┤
│  Raspberry Pi 5 / x86 hardware       │
└──────────────────────────────────────┘
```

Supervisor — наш интерфейс установки: тянет Docker образ, пробрасывает devices (USB, GPIO, audio), даёт Add-On доступ к HA Core через внутренний API, управляет бакапом конфигурации.

**Add-On API полностью стандартизован.** Один Add-On работает на любой HA OS / HA Supervised установке без правок. Различия между платформами скрываются за multi-arch сборкой Docker образа.

#### config.yaml — манифест Add-On

```yaml
name: Isin Voice
version: "1.0.0"
slug: isin_voice
description: Локальный voice assistant на базе Whisper
arch:
  - arm64       # RP5, RP4
  - amd64       # home lab
url: https://github.com/ares566/isin-ha
image: ghcr.io/ares566/isin-ha:{version}

homeassistant: "2024.7.0"   # минимальная версия HA

usb: true                   # для USB-микрофона
gpio: true                  # для кнопки на RP5
audio: true                 # /dev/snd/* доступ
host_dbus: true             # альтернативный путь к GPIO

options:
  gpio_pin: 5
  language: "ru"
  whisper_model: "large-v3-q5"
  wake_word_enabled: false
schema:
  gpio_pin: int
  language: list("ru"|"en")
  whisper_model: list("large-v3-q5"|"medium-q5")
  wake_word_enabled: bool

hassio_api: true
```

#### Supervisor API — что Add-On может вызывать

- `http://supervisor/core/api/services/...` — HA Core REST API
- `ws://supervisor/core/websocket` — HA WebSocket (состояния, события)
- `http://supervisor/addons/self/options` — чтение настроек юзера

Supervisor проксирует и авторизует. Не нужно хардкодить HA token — Supervisor даёт Add-On long-lived token автоматически.

#### Нюансы платформ

Архитектурные различия внутри HA OS минимальны — всё, что меняется:

| Платформа | Whisper large-v3 latency | GPIO | Замечание |
|---|---|---|---|
| RP5 8GB (arm64) | 2-3 сек | Да, `/dev/gpiochip*` | **Primary target** |
| RP5 4GB (arm64) | 2-3 сек, Ollama медленнее | Да | OK |
| RP4 4-8GB (arm64) | 3-4 сек | Да | Работает, медленнее |
| x86 NUC / home lab (amd64) | 0.5-1 сек | Нет, USB кнопка или wake word | HA Container юзеры тут |

GPIO недоступен на x86 — для них wake word или USB-кнопка. Это автоматически через config.yaml: на amd64 юзер выбирает wake word в UI.

#### Стратегия совместимости по фазам

**Phase 0 (демо-видео):**
- Только HA OS на RP5
- HAOS image на NVMe SSD (через NVMe base) или USB SSD
- Multi-arch сборка не нужна — пушим только arm64
- License server не нужен — hardcode "trial forever" на демо

**Phase 1 (MVP):**
- HA OS на RP5 (arm64) — основной таргет
- HA OS на x86 (amd64) — добавляем в multi-arch build, лёгкая совместимость
- HA Supervised — автоматически работает (Supervisor тот же)
- **HA Container — НЕ поддерживаем.** Честно в документации: «requires HA OS or HA Supervised»

**Phase 2 (после $5K MRR, расширение):**
- HA Container: отдельный `docker-compose.yml` + инструкция
- Те же multi-arch образы из ghcr.io
- License server работает идентично
- Эти юзеры технически продвинутые — справятся с docker-compose

#### Железо для демо-стенда (Phase 0)

| Компонент | Что | Цена |
|---|---|---|
| RP5 8GB | Основное устройство | $80 |
| NVMe base + SSD 256GB | Для скорости Whisper (SD card тормозит 5-10x) | $30 + $25 |
| Официальный HAOS image | С сайта home-assistant.io | Бесплатно |
| USB микрофон | Boya BY-M1 или K&F | $10-15 |
| GPIO кнопка + резистор + провода | Tactile button 4-pin, 10kΩ, dupont | $2 |
| Yeelight или zigbee-реле | Что угодно для демонстрации «выключи свет» | $15-25 |
| MicroSD 32GB | Для записи видео, не для HAOS | $5 |

**Итого железа: ~$170-200.** Разовая инвестиция в production setup, который после демо остаётся как дев-стенд.

### 7.10 Архитектурная совместимость с non-HA системами

Voice pipeline **полностью платформо-независимый** — ~70% кода (аудио → Whisper → роутер → TTS). Привязка к HA только в слое executor (~10-15% кода) и в Add-On packaging (Docker через Supervisor — уникально для HA).

Технически можем поддержать другие системы. Сейчас не делаем — из соображений фокуса, не из-за ограничений.

#### Ландшафт альтернатив

| Система | Установок | Voice поддержка | Приоритет для нас |
|---|---|---|---|
| **Home Assistant** | ~2M | HA Assist (требует cloud) | **Primary target** |
| **ioBroker** | ~80K | слабая (sayit + cloud) | Сильный кандидат Phase 2 |
| **openHAB** | ~100K | почти нет | Средний кандидат |
| **Jeedom** | ~30-50K | есть, франкоязычная | Низкий (Франция фокус) |
| **Domoticz** | ~50K | нет | Низкий (нет Add-On ecosystem) |
| **MajorDoMo** | ~20K (РФ/СНГ) | базовая | Средний для РФ-сегмента |
| **Hubitat** | ~50K (USA) | есть | Низкий (закрытая платформа) |

HA занимает 80-85% рынка self-hosted Умного Дома. Это не локаут — это фокус там, где рынок.

#### Технические отличия систем

| Слой | HA | openHAB | ioBroker | Domoticz |
|---|---|---|---|---|
| API | REST + WebSocket | REST + SSE | WebSocket + REST | JSON/HTTP |
| Конфигурация | YAML | .items files | JSON adapter | GUI |
| Discovery | multicast + интеграции | Paper UI | admin adapter | hardware |
| Add-On mechanism | **Docker via Supervisor** (уникально) | OSGi bindings (Java) | adapters (node.js) | plugins (C++) |
| Voice pipeline | HA Assist | нет родного | sayit | нет |

**Add-On система у каждой своя.** Никакой «универсальной» инсталляции. Для openHAB — OSGi binding на Java. Для ioBroker — adapter на node.js. Разные кодовые базы.

Но если voice pipeline установлен **standalone** (Docker образ без HA Supervisor) — он работает с любой системой, имеющей REST/WebSocket/MQTT API.

#### Архитектурное решение — заложить расширяемость сейчас бесплатно

Trait abstraction в коде с первого дня. Цена — 0, просто правильная декомпозиция:

```rust
#[async_trait]
trait HomeAutomationBackend: Send + Sync {
    async fn call_service(&self, domain: &str, service: &str, payload: Value) -> Result<()>;
    async fn get_state(&self, entity_id: &str) -> Result<EntityState>;
    async fn list_entities(&self, domain: Option<&str>) -> Result<Vec<EntityState>>;
    async fn subscribe_events(&self, event_types: &[&str]) -> Result<EventStream>;
}

// Реализации
struct HomeAssistantBackend { ws: WebSocketClient }      // Phase 0-1
struct OpenHabBackend { http: ReqwestClient }            // Phase 3
struct IoBrokerBackend { ws: WebSocketClient }           // Phase 3
struct MqttBridgeBackend { client: MqttClient }          // универсальный fallback
```

В `config.toml`:

```toml
[backend]
type = "homeassistant"     # или "openhab", "iobroker", "mqtt"
url = "ws://supervisor/core/websocket"
token = "${SUPERVISOR_TOKEN}"
```

Skill router не знает, какой backend внизу — работает с `HomeAutomationBackend` trait. Skill описывает **что сделать** (`turn_off light in {room}`), а HA-специфичный маппинг в `service_call` — в отдельном слое.

#### Standalone режим для non-HA систем

Для всех non-HA систем наш продукт = **standalone voice daemon + тонкий bridge** под конкретную платформу. Не Add-On, а companion-сервис.

- **openHAB:** наш Docker образ рядом, openHAB binding (Java) ходит к нашему REST API
- **ioBroker:** наш Docker образ рядом, ioBroker adapter (node.js) делает REST/WebSocket вызовы
- **MQTT universal mode:** наш сервис подписывается на MQTT топики, любая система может слать команды

#### Реалистичная стратегия расширения по фазам

| Фаза | Поддерживаемые системы | Почему |
|---|---|---|
| **Phase 0-1** | только HA | Фокус, 80% рынка, лучший UX через Add-On |
| **Phase 2** (после $5K MRR) | + openHAB (standalone) | Community голосует за voice годами |
| **Phase 2.5** | + ioBroker (standalone) | РФ/СНГ сегмент, наш родной рынок |
| **Phase 3** (после $15K MRR) | + MQTT universal mode | Любая система с MQTT commands |

Каждое расширение — неделя работы на backend integration + 2-3 недели на документацию и тестирование. **Если архитектура заложена правильно с первого дня.**

#### Что закладываем в код СЕЙЧАС (бесплатно)

1. **Trait `HomeAutomationBackend`** — обязательная абстракция с первого дня
2. **Backend-agnostic skill format** в TOML — skill описывает что сделать, HA-специфичный маппинг в `service_call` — в отдельном слое
3. **Configuration switch** `[backend].type` — даже если сейчас поддерживает только `"homeassistant"`, fallback на ошибку с понятным сообщением
4. **Standalone mode** — возможность запустить isin-ha вне HA Supervisor (через docker-compose), даже без GUI

Это не откладывает MVP. Это правильная декомпозиция, которую всё равно пришлось бы сделать при расширении.

#### Что НЕ делаем

- **Не обещаем совместимость в маркетинге.** Пока не поддерживается openHAB — не пишем про него на лендинге. Иначе — issue на GitHub от юзеров, ожидающих рабочий openHAB.
- **Не абстрагируем HA-specific фичи.** HA Supervisor API, discovery через HA — оставляем как есть, в backend-реализации.
- **Не пишем сейчас OpenHabBackend/IoBrokerBackend.** Только trait и HomeAssistantBackend. Остальное — когда будет спрос.

#### Matter / Thread — не решение

Стандарт Matter унифицирует связь между устройствами и хабами на protocol layer. Но не на application layer — UI автоматизации (HA, openHAB, и т.д.) остаются разными. Matter device работает с любой системой, но сама система остаётся отдельной. Не снимает наш вопрос.

---

## 8. План фаз

### Фаза 0: Pre-build validation (2 недели, ~10-15 ч/нед)
**Цель:** проверить, есть ли спрос, до написания кода.

- [ ] Лендинг (Tilda или простой HTML) с полем «email для раннего доступа»
- [ ] 5-минутное видео на YouTube: показываете прототип Whisper на RP5, который слушает «выключи свет на кухне» и HA выполняет
- [ ] Пост на Хабр: тот же контент + опрос «заплатили бы $10/мес?»
- [ ] Создать Telegram-канал-компаньон под проект
- [ ] 5 разговоров в r/homeassistant и HA Community

**GO/NO-GO критерий через 2 недели:**
- 200+ email-подписок → сильный сигнал, начинаем MVP
- 50-200 → средний сигнал, делаем ещё одно видео/пост
- <50 → переосмыслить угол (возможно, не voice, а другое AI-приложение)

### Фаза 1: MVP (5-8 недель после GO сигнала)
**Цель:** первая продажа.

- [ ] Перенос macOS → Linux/RP5 (audio через ALSA, hotkey через GPIO)
- [ ] HA WebSocket клиент (состояния, сервисы, события)
- [ ] 5-10 базовых HA-скиллов (свет, климат, media, сцены)
- [ ] Биллинг (Stripe или CloudPayments)
- [ ] Упаковка как HA Add-On (multi-arch Docker образ, ghcr.io)
- [ ] License server на Go + PostgreSQL + Redis
- [ ] Документация для первых 10 пользователей

### Фаза 2: Ранний рост (3-6 месяцев после MVP)
- [ ] Регулярный контент на YouTube (1 видео в неделю)
- [ ] Поддержка wake word (openWakeWord)
- [ ] Расширенные сценарии (медиа, шторы,Robots)
- [ ] Расширение на ESP32-клиентов (опциональный хаб)

### Фаза 3: Масштабирование (6-18 месяцев)
- [ ] Англоязычная локализация интерфейса
- [ ] Premium tier ($25/мес): multi-room sync, advanced automations
- [ ] Партнёрства с HA-инфлюенсерами
- [ ] Выход на GopherCon/Home Assistant Conference

## 9. Главные риски и mitigation

### Риск 1: HA Foundation выпускает свой local AI voice
**Вероятность:** низкая (4 года не делали, их фокус — infrastructure).
**Mitigation:** быстрая итерация, занять нишу раньше, если они всё же сделают — позиционироваться как «голос-на-edge для энтузиастов, custom скиллы».

### Риск 2: Готовность платить низкая (DIY-сообщество любит бесплатное)
**Вероятность:** средняя.
**Mitigation:** правильно поставить ценность — не «платишь за voice», а «платишь за convenience + ready-to-use + support + обновления моделей». Сравнение с Netflix ($15/мес) или Spotify ($10/мес).

### Риск 3: Технические сложности с Linux audio на RP5
**Вероятность:** средняя (cpal на Linux капризный).
**Mitigation:** начать с проверенной связки (ALSA direct или PulseAudio). Резервный вариант — Python-обёртка с sounddevice.

### Риск 4: Apple Silicon native-интеграция (HomeKit)
**Вероятность:** низкая.
**Mitigation:** не наша целевая аудитория. HomeKit-юзеры не используют HA.

### Риск 5: Локализация Voice на русский — низкое качество
**Вероятность:** низкая (Whisper large-v3 на русском WER 6-9%, в Исине уже работает).
**Mitigation:** custom vocabulary (уже реализовано в Исине), fine-tuning при необходимости.

### Риск 6: Недоверие сообщества к закрытому голосовому Add-On
**Вероятность:** средняя (HA-сообщество paranoia по privacy).
**Mitigation:** open core стратегия (раздел 7.6) — ядро открыто, аудируется. Premium — только server-side gate и skill templates.

## 10. Что НЕ делать (фокус)

- **Не конкурируем с Nabu Casa по remote access.** Это их зона, у них фронт. Мы — надстройка.
- **Не делаем cloud STT/TTS.** Только локальный Whisper на edge.
- **Не поддерживаем Apple HomeKit-only.** HA = наша единственная платформа.
- **Не делаем enterprise.** Маленький чек, B2C, никакого sales team.
- **Не расширяем стек на Python/JS.** Только Rust (ваш опыт, производительность, low resource).
- **Не поддерживаем arm/v7 (RP3 и старье).** Whisper large-v3 не выживет на 1GB RAM, не обещаем невозможное.

## 11. Ближайшие 2 недели — конкретный план

| День | Действие | Часы |
|---|---|---|
| 1-2 | Снять прототип-демо (RP5 + Whisper + HA + голосовая команда «выключи свет») | 4-6 |
| 3 | Лендинг на Tilda с полем email | 3-4 |
| 4 | Опубликовать видео на YouTube, анонс в Telegram | 2 |
| 5-6 | Пост на Хабр (с тем же контентом, текст + видео) | 3-4 |
| 7 | Пост в Reddit r/homeassistant, HA Community forum | 2 |
| 8-10 | Ответы на комментарии, общение с интересующимися | 3-4 |
| 11-12 | Анализ отклика, решение GO/NO-GO | 2 |
| 13-14 | Если GO — план переноса macOS → Linux/RP5 | 4 |

**Итого за 2 недели:** ~25-30 часов работы.

## 12. Источники и ссылки

- [State of the Open Home 2025 — 2M домохозяйств](https://www.home-assistant.io/blog/2025/04/16/state-of-the-open-home-recap/)
- [Nabu Casa Pricing](https://www.nabucasa.com/pricing/)
- [HA Assist documentation](https://www.home-assistant.io/voice_control/)
- [Wyoming Whisper protocol](https://www.home-assistant.io/integrations/wyoming/)
- [HA Community forum](https://community.home-assistant.io/)
- [Reddit r/homeassistant](https://www.reddit.com/r/homeassistant/)
- [Isin текущий репозиторий](https://github.com/Ares566/Isin)

## 13. Связанные проекты и память

- **YouTube-канал:** https://www.youtube.com/@RenatAbaidullin (возрождается под HA+AI тематику)
- **Верус (предыдущий проект):** приостановлен 2026-07-23, голосовой ИИ-спутник для рефлексии — может вернуться
- **Направление Go+arch образование:** отложено 2026-07-28, может вернуться позже
