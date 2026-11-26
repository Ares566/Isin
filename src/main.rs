mod audio;
mod config;
mod formatter;
mod hotkey;
mod transcriber;

use std::fs;
use std::path::PathBuf;

use clap::Parser;
use transcriber::Transcriber;

#[derive(Parser)]
#[command(name = "isin")]
#[command(about = "Исин — голосовой ассистент для macOS")]
struct Cli {
    /// Режим транскрибации файла (вместо демона)
    #[arg(short, long)]
    file: Option<PathBuf>,

    /// Путь к модели Whisper (GGML формат)
    #[arg(short, long, default_value_os_t = default_model_path())]
    model: PathBuf,

    /// Язык распознавания
    #[arg(short, long, default_value = "ru")]
    language: String,

    /// Путь к выходному MD-файлу (только для --file)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Показать список микрофонов и выйти
    #[arg(long)]
    list_devices: bool,

    /// Путь к конфигурационному файлу
    #[arg(short, long, default_value_os_t = default_config_path())]
    config: PathBuf,
}

fn default_model_path() -> PathBuf {
    let local = PathBuf::from("ggml-large-v3.bin");
    if local.exists() {
        return local;
    }

    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".local/share"))
        .join("isin/ggml-large-v3.bin")
}

fn default_config_path() -> PathBuf {
    let local = PathBuf::from("config.toml");
    if local.exists() {
        return local;
    }

    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".config"))
        .join("isin/config.toml")
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
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
    daemon_mode(&cli, initial_prompt.as_deref())
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum UiStatus {
    Idle,
    Recording,
    Processing,
    Success,
}

#[derive(Clone, Copy)]
enum UiEvent {
    Status(UiStatus),
    Exit,
}

/// Запускает иконку в строке меню и рабочий поток голосового ввода.
fn daemon_mode(cli: &Cli, initial_prompt: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    const ANIMATION_INTERVAL: std::time::Duration = std::time::Duration::from_millis(120);
    const SUCCESS_DURATION: std::time::Duration = std::time::Duration::from_secs(2);

    let event_loop = tao::event_loop::EventLoopBuilder::<UiEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let model = cli.model.clone();
    let language = cli.language.clone();
    let initial_prompt = initial_prompt.map(String::from);
    let worker_proxy = proxy.clone();

    std::thread::spawn(move || {
        if let Err(e) = voice_loop(
            &model,
            &language,
            initial_prompt.as_deref(),
            worker_proxy.clone(),
        ) {
            eprintln!("Ошибка: {e}");
            let _ = worker_proxy.send_event(UiEvent::Exit);
        }
    });

    let mut tray: Option<tray_icon::TrayIcon> = None;
    let mut status = UiStatus::Idle;
    let mut animation_frame = 0_u64;
    let mut next_animation = None;
    let mut success_until = None;
    event_loop.run(move |event, _, control_flow| {
        *control_flow = tao::event_loop::ControlFlow::Wait;
        match event {
            tao::event::Event::NewEvents(tao::event::StartCause::Init) => {
                match tray_icon::TrayIconBuilder::new()
                    .with_tooltip(status_tooltip(UiStatus::Idle))
                    .with_icon(status_icon(UiStatus::Idle, 0))
                    .build()
                {
                    Ok(icon) => tray = Some(icon),
                    Err(e) => {
                        eprintln!("Ошибка создания иконки в трее: {e}");
                        *control_flow = tao::event_loop::ControlFlow::Exit;
                    }
                }
            }
            tao::event::Event::UserEvent(UiEvent::Status(new_status)) => {
                status = new_status;
                animation_frame = 0;
                next_animation = (status == UiStatus::Processing)
                    .then(|| std::time::Instant::now() + ANIMATION_INTERVAL);
                success_until = (status == UiStatus::Success)
                    .then(|| std::time::Instant::now() + SUCCESS_DURATION);
                if let Some(icon) = &tray {
                    update_tray(icon, status, animation_frame);
                }
            }
            tao::event::Event::NewEvents(tao::event::StartCause::ResumeTimeReached { .. }) => {
                let now = std::time::Instant::now();
                if status == UiStatus::Processing
                    && next_animation.is_some_and(|deadline| deadline <= now)
                {
                    animation_frame = animation_frame.wrapping_add(1);
                    next_animation = Some(now + ANIMATION_INTERVAL);
                    if let Some(icon) = &tray {
                        if let Err(e) = icon.set_icon(Some(status_icon(status, animation_frame))) {
                            eprintln!("Ошибка обновления иконки: {e}");
                        }
                    }
                }
                if status == UiStatus::Success
                    && success_until.is_some_and(|deadline| deadline <= now)
                {
                    status = UiStatus::Idle;
                    success_until = None;
                    if let Some(icon) = &tray {
                        update_tray(icon, status, 0);
                    }
                }
            }
            tao::event::Event::UserEvent(UiEvent::Exit) => {
                *control_flow = tao::event_loop::ControlFlow::Exit;
            }
            _ => {}
        }

        if !matches!(*control_flow, tao::event_loop::ControlFlow::Exit) {
            if let Some(deadline) = next_animation.into_iter().chain(success_until).min() {
                *control_flow = tao::event_loop::ControlFlow::WaitUntil(deadline);
            }
        }
    })
}

fn update_tray(icon: &tray_icon::TrayIcon, status: UiStatus, animation_frame: u64) {
    if let Err(e) = icon.set_icon(Some(status_icon(status, animation_frame))) {
        eprintln!("Ошибка обновления иконки: {e}");
    }
    if let Err(e) = icon.set_tooltip(Some(status_tooltip(status))) {
        eprintln!("Ошибка обновления подсказки: {e}");
    }
}

fn status_tooltip(status: UiStatus) -> &'static str {
    match status {
        UiStatus::Idle => "Исин — удерживайте Fn для записи",
        UiStatus::Recording => "Исин — идёт запись",
        UiStatus::Processing => "Исин — распознаёт речь",
        UiStatus::Success => "Исин — текст распознан",
    }
}

/// Основной цикл: удержание Fn → запись → транскрибация → буфер обмена и вставка.
fn voice_loop(
    model: &std::path::Path,
    language: &str,
    initial_prompt: Option<&str>,
    proxy: tao::event_loop::EventLoopProxy<UiEvent>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Исин запускается...");
    println!("Загрузка модели {}...", model.display());
    let t = Transcriber::new(model, language, initial_prompt.map(String::from))?;

    println!("Запуск слушателя клавиши Fn...");
    let mut hotkey_rx = hotkey::HotkeyListener::new().start()?;

    println!("Исин готов. Удерживайте Fn для записи, отпустите для транскрибации.");
    println!("Ctrl+C — выход.\n");

    loop {
        match hotkey_rx.blocking_recv() {
            Some(hotkey::HotkeyEvent::Pressed) => {}
            Some(hotkey::HotkeyEvent::Released) => continue,
            None => return Ok(()),
        }

        println!("🎤 Запись... Отпустите Fn для завершения.");
        let capture_start = std::time::Instant::now();

        let capture = match audio::LiveCapture::start() {
            Ok(capture) => capture,
            Err(e) => {
                eprintln!("Ошибка захвата аудио: {e}");
                continue;
            }
        };
        let _ = proxy.send_event(UiEvent::Status(UiStatus::Recording));

        loop {
            match hotkey_rx.blocking_recv() {
                Some(hotkey::HotkeyEvent::Released) => break,
                Some(hotkey::HotkeyEvent::Pressed) => {}
                None => return Ok(()),
            }
        }

        let audio = capture.stop();
        let _ = proxy.send_event(UiEvent::Status(UiStatus::Processing));
        let audio = match audio {
            Ok(data) => data,
            Err(e) => {
                let _ = proxy.send_event(UiEvent::Status(UiStatus::Idle));
                eprintln!("Ошибка завершения записи: {e}");
                continue;
            }
        };
        let capture_time = capture_start.elapsed();

        if audio.is_empty() {
            let _ = proxy.send_event(UiEvent::Status(UiStatus::Idle));
            println!("  (пусто — нет звука)\n");
            continue;
        }

        let transcribe_start = std::time::Instant::now();
        match t.transcribe(&audio) {
            Ok(text) => {
                let transcribe_time = transcribe_start.elapsed();
                if text.trim().is_empty() {
                    let _ = proxy.send_event(UiEvent::Status(UiStatus::Idle));
                    println!("  (речь не распознана)\n");
                    continue;
                }

                let _ = proxy.send_event(UiEvent::Status(UiStatus::Success));
                println!("📝 {text}");
                println!(
                    "  ⏱ запись {:.1}с + транскрибация {:.1}с",
                    capture_time.as_secs_f64(),
                    transcribe_time.as_secs_f64()
                );

                match copy_to_clipboard(&text) {
                    Ok(()) => match paste_into_active_app() {
                        Ok(()) => {
                            println!("  📋 Скопировано в буфер обмена и вставлено");
                            notify_transcription_complete(true);
                        }
                        Err(e) => {
                            println!("  📋 Скопировано в буфер обмена");
                            eprintln!("  ⚠️ Не удалось вставить текст: {e}");
                            eprintln!(
                                "     Разрешите терминалу доступ: Конфиденциальность и безопасность → Универсальный доступ"
                            );
                            notify_transcription_complete(false);
                        }
                    },
                    Err(e) => eprintln!("  ❌ Ошибка копирования: {e}"),
                }
                println!();
            }
            Err(e) => {
                let _ = proxy.send_event(UiEvent::Status(UiStatus::Idle));
                eprintln!("Ошибка транскрибации: {e}\n");
            }
        }
    }
}

fn status_icon(status: UiStatus, animation_frame: u64) -> tray_icon::Icon {
    const SIZE: u32 = 22;
    let mut color = match status {
        UiStatus::Idle => [142, 142, 147, 255],
        UiStatus::Recording => [255, 59, 48, 255],
        UiStatus::Processing => [255, 214, 10, 255],
        UiStatus::Success => [52, 199, 89, 255],
    };
    if status == UiStatus::Processing {
        let pulse = ((animation_frame as f32 * 0.35).sin() + 1.0) / 2.0;
        let brightness = 0.65 + pulse * 0.35;
        for channel in &mut color[..3] {
            *channel = (*channel as f32 * brightness).round() as u8;
        }
    }
    let mut rgba = vec![0; (SIZE * SIZE * 4) as usize];
    let center = (SIZE as f32 - 1.0) / 2.0;

    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            if dx * dx + dy * dy <= 8.0 * 8.0 {
                let offset = ((y * SIZE + x) * 4) as usize;
                rgba[offset..offset + 4].copy_from_slice(&color);
            }
        }
    }

    tray_icon::Icon::from_rgba(rgba, SIZE, SIZE).expect("размер иконки задан корректно")
}

fn copy_to_clipboard(text: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut child = Command::new("pbcopy").stdin(Stdio::piped()).spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(text.as_bytes())?;
    }
    let status = child.wait()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "pbcopy завершился со статусом {status}"
        )))
    }
}

fn paste_into_active_app() -> std::io::Result<()> {
    run_osascript(r#"tell application "System Events" to keystroke "v" using command down"#)
}

fn run_osascript(script: &str) -> std::io::Result<()> {
    let output = std::process::Command::new("osascript")
        .args(["-e", script])
        .output()?;
    if output.status.success() {
        return Ok(());
    }

    let details = String::from_utf8_lossy(&output.stderr);
    let details = details.trim();
    if details.is_empty() {
        Err(std::io::Error::other(format!(
            "osascript завершился со статусом {}",
            output.status
        )))
    } else {
        Err(std::io::Error::other(details))
    }
}

fn notify_transcription_complete(pasted: bool) {
    let message = if pasted {
        "Текст скопирован и вставлен"
    } else {
        "Текст скопирован в буфер обмена"
    };
    let script = format!(r#"display notification "{message}" with title "Исин" sound name "Ping""#);
    if let Err(e) = run_osascript(&script) {
        eprintln!("Ошибка системного уведомления: {e}");
    }
}
