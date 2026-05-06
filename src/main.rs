mod audio;
mod config;
mod formatter;
mod hotkey;
mod router;
mod skills;
mod transcriber;

use std::fs;
use std::path::PathBuf;

use clap::Parser;
use skills::clipboard::ClipboardSkill;
use skills::SkillRegistry;
use transcriber::Transcriber;

#[derive(Parser)]
#[command(name = "isin")]
#[command(about = "Исин — голосовой ассистент для macOS")]
struct Cli {
    /// Режим транскрибации файла (вместо демона)
    #[arg(short, long)]
    file: Option<PathBuf>,

    /// Путь к модели Whisper (GGML формат)
    #[arg(short, long, default_value = "ggml-large-v3.bin")]
    model: PathBuf,

    /// Язык распознавания
    #[arg(short, long, default_value = "ru")]
    language: String,

    /// Путь к выходному MD-файлу (только для --file)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Секунд тишины до остановки записи
    #[arg(long, default_value = "5")]
    silence_stop: u64,

    /// Минимальная длительность речи до остановки (секунд)
    #[arg(long, default_value = "3")]
    min_speech: u64,

    /// Показать список микрофонов и выйти
    #[arg(long)]
    list_devices: bool,

    /// Путь к конфигурационному файлу
    #[arg(short, long, default_value = "config.toml")]
    config: PathBuf,
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("Ошибка: {e}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Список аудиоустройств
    if cli.list_devices {
        println!("Доступные микрофоны:");
        audio::list_input_devices();
        return Ok(());
    }

    // Проверка модели
    if !cli.model.exists() {
        eprintln!("Модель не найдена: {}", cli.model.display());
        eprintln!(
            "Скачайте: curl -L -o ggml-large-v3.bin \
             https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin"
        );
        std::process::exit(1);
    }

    // Загрузка конфига (необязательный — если нет, работаем без скиллов)
    let cfg = if cli.config.exists() {
        Some(config::Config::load(&cli.config)?)
    } else {
        None
    };

    // Формируем initial_prompt из словаря конфига
    let initial_prompt = cfg.as_ref().and_then(|c| c.whisper_prompt());

    // Режим файла
    if let Some(ref file_path) = cli.file {
        return file_mode(&cli, &file_path, initial_prompt.as_deref());
    }

    // Режим демона
    daemon_mode(&cli, &cfg, initial_prompt.as_deref()).await
}

/// Транскрибация аудиофайла и запись в Markdown.
fn file_mode(
    cli: &Cli,
    file_path: &PathBuf,
    initial_prompt: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    if !file_path.exists() {
        eprintln!("Файл не найден: {}", file_path.display());
        std::process::exit(1);
    }

    println!("Загрузка модели {}...", cli.model.display());
    let t = Transcriber::new(&cli.model, &cli.language, initial_prompt.map(String::from))?;

    println!("Декодирование аудио...");
    let audio = audio::decode_to_pcm_16khz_mono(file_path)?;

    println!("Транскрибация (язык: {})...", cli.language);
    let text = t.transcribe(&audio)?;

    let output_path = cli
        .output
        .clone()
        .unwrap_or_else(|| file_path.with_extension("md"));

    let segments = vec![formatter::Segment {
        text,
        speaker: None,
    }];
    let md = formatter::format(
        &segments,
        file_path.file_name().unwrap().to_str().unwrap(),
        &cli.language,
    );

    fs::write(&output_path, md)?;
    println!("Готово! Результат: {}", output_path.display());

    Ok(())
}

/// Основной цикл демона: ждём хоткей → записываем → транскрибируем → роутер → скилл.
async fn daemon_mode(
    cli: &Cli,
    cfg: &Option<config::Config>,
    initial_prompt: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Исин запускается...");
    println!("Загрузка модели {}...", cli.model.display());
    let t = Transcriber::new(&cli.model, &cli.language, initial_prompt.map(String::from))?;

    // Реестр скиллов
    let mut registry = SkillRegistry::new();
    registry.register(Box::new(ClipboardSkill));
    registry.set_fallback(Box::new(ClipboardSkill));

    // Навыки из конфига
    let skills = cfg.as_ref().map(|c| c.skill.clone()).unwrap_or_default();
    let fallback_def = cfg.as_ref().and_then(|c| c.fallback_skill());

    println!("Запуск слушателя хоткея (Cmd+Shift+Space)...");
    let mut hotkey_rx = hotkey::HotkeyListener::new().start()?;

    println!("Исин готов. Нажмите Cmd+Shift+Space для записи.");
    println!("Ctrl+C — выход.\n");

    loop {
        // Ждём нажатия хоткея
        hotkey_rx.recv().await;

        println!("🎤 Запись...");
        let capture_start = std::time::Instant::now();

        // Захват аудио с микрофона
        let audio = match audio::capture_live(cli.silence_stop, cli.min_speech) {
            Ok(data) => data,
            Err(e) => {
                eprintln!("Ошибка захвата аудио: {e}");
                continue;
            }
        };

        let capture_time = capture_start.elapsed();

        if audio.is_empty() {
            println!("  (пусто — нет звука)\n");
            continue;
        }

        // Транскрибация
        let transcribe_start = std::time::Instant::now();
        match t.transcribe(&audio) {
            Ok(text) => {
                let transcribe_time = transcribe_start.elapsed();
                if text.trim().is_empty() {
                    println!("  (речь не распознана)\n");
                    continue;
                }

                println!("📝 {text}");
                println!(
                    "  ⏱ запись {:.1}с + транскрибация {:.1}с",
                    capture_time.as_secs_f64(),
                    transcribe_time.as_secs_f64()
                );

                // Роутер: матчим текст против скиллов
                if let Some(m) = router::match_skill(&text, &skills, fallback_def) {
                    if let Some(skill) = registry.get(&m.skill.action) {
                        match skill.execute(&text, &m.params) {
                            Ok(result) => println!("  ✅ {result}"),
                            Err(e) => eprintln!("  ❌ {e}"),
                        }
                    } else {
                        println!("  ⚠️ скилл '{}' не зарегистрирован", m.skill.action);
                    }
                }
                println!();
            }
            Err(e) => {
                eprintln!("Ошибка транскрибации: {e}\n");
            }
        }
    }
}
