use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, SampleRate, StreamConfig};

/// Коэффициент программного усиления микрофона.
/// Встроенные микрофоны macOS часто дают тихий сигнал — усиливаем программно.
const MIC_GAIN: f32 = 4.0;


/// Декодирует аудиофайл в f32 PCM 16kHz mono через ffmpeg.
pub fn decode_to_pcm_16khz_mono(path: &Path) -> Result<Vec<f32>, AudioError> {
    let output = Command::new("ffmpeg")
        .args([
            "-i",
            path.to_str().ok_or(AudioError::InvalidPath)?,
            "-f",
            "f32le",
            "-ac",
            "1",
            "-ar",
            "16000",
            "-hide_banner",
            "-loglevel",
            "error",
            "-",
        ])
        .output()
        .map_err(AudioError::Ffmpeg)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AudioError::FfmpegOutput(stderr.to_string()));
    }

    let samples = output
        .stdout
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect();

    Ok(samples)
}

/// Захват звука с микрофона с автоматической остановкой по тишине.
///
/// Записывает аудио через cpal, конвертирует в 16kHz mono f32
/// и останавливается когда detected silence длится `silence_stop_secs`.
pub fn capture_live(silence_stop_secs: u64, min_speech_secs: u64) -> Result<Vec<f32>, AudioError> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or(AudioError::NoDevice)?;

    let supported = device
        .default_input_config()
        .map_err(AudioError::DeviceConfig)?;

    let sample_format = supported.sample_format();
    let mut config: StreamConfig = supported.config().clone();
    let channels = config.channels as usize;

    // Пытаемся получить 16kHz у устройства
    if let Some(rate) = select_sample_rate(&device, sample_format, config.channels, 16000) {
        config.sample_rate = SampleRate(rate);
    }

    let device_rate = config.sample_rate.0;
    let buffer: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));

    // Запуск потока захвата
    let stream = build_input_stream(&device, &config, sample_format, channels, &buffer)?;
    stream.play().map_err(AudioError::StreamPlay)?;

    // Цикл ожидания с определением тишины
    let poll_interval = Duration::from_millis(50);
    let window_samples = (device_rate as f64 * poll_interval.as_secs_f64()) as usize;
    let noise_floor_duration = Duration::from_secs(1);
    let silence_duration = Duration::from_secs(silence_stop_secs.max(1));
    let min_duration = Duration::from_secs(min_speech_secs);

    let start = std::time::Instant::now();
    let mut last_sound = start;
    let mut heard_sound = false;

    // Собираем уровни за первую секунду для калибровки шума
    let mut calibration_levels: Vec<f32> = Vec::new();

    loop {
        std::thread::sleep(poll_interval);
        let elapsed = start.elapsed();

        if let Ok(buf) = buffer.lock() {
            if !buf.is_empty() {
                let start_idx = buf.len().saturating_sub(window_samples.max(1));
                let level = window_level(&buf[start_idx..]);

                // В течение первой секунды собираем уровни для калибровки
                if elapsed <= noise_floor_duration {
                    calibration_levels.push(level);
                }

                // После калибровки: порог = медиана * множитель
                let threshold = if calibration_levels.len() >= 4 {
                    let mut sorted = calibration_levels.clone();
                    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                    let median = sorted[sorted.len() / 2];
                    (median * 3.0).max(0.003)
                } else {
                    0.003
                };

                let has_sound = level >= threshold;

                if has_sound {
                    last_sound = std::time::Instant::now();
                    heard_sound = true;
                }
            }
        }

        // Не слышали речи — таймаут
        if !heard_sound && elapsed >= silence_duration {
            break;
        }

        // Минимум речи пройден + тишина — стоп
        if heard_sound && elapsed >= min_duration && last_sound.elapsed() >= silence_duration {
            break;
        }
    }

    drop(stream);

    let mut data = buffer
        .lock()
        .map_err(|_| AudioError::BufferAccess)?
        .clone();

    // Resample до 16kHz если устройство выдаёт другую частоту
    if device_rate != 16000 && !data.is_empty() {
        data = resample_linear(&data, device_rate, 16000);
    }

    // Нормализация: приводим пик к 0.9 для чёткого распознавания
    if !data.is_empty() {
        normalize(&mut data);
    }

    Ok(data)
}

/// Выводит список доступных аудиоустройств ввода.
pub fn list_input_devices() {
    let host = cpal::default_host();
    if let Ok(mut devices) = host.input_devices() {
        while let Some(device) = devices.next() {
            let name = device.name().unwrap_or_else(|_| "Unknown".into());
            println!("  {name}");
        }
    }
}

// --- Внутренние функции ---

fn build_input_stream(
    device: &Device,
    config: &StreamConfig,
    sample_format: SampleFormat,
    channels: usize,
    buffer: &Arc<Mutex<Vec<f32>>>,
) -> Result<cpal::Stream, AudioError> {
    let writer = buffer.clone();

    match sample_format {
        SampleFormat::F32 => device
            .build_input_stream(
                config,
                move |data: &[f32], _| {
                    capture_samples(data, channels, &writer, |s| *s);
                },
                |err| eprintln!("ошибка аудио: {err}"),
                None,
            )
            .map_err(AudioError::BuildStream),
        SampleFormat::I16 => device
            .build_input_stream(
                config,
                move |data: &[i16], _| {
                    capture_samples(data, channels, &writer, |s| *s as f32 / i16::MAX as f32);
                },
                |err| eprintln!("ошибка аудио: {err}"),
                None,
            )
            .map_err(AudioError::BuildStream),
        SampleFormat::U16 => device
            .build_input_stream(
                config,
                move |data: &[u16], _| {
                    capture_samples(data, channels, &writer, |s| {
                        (*s as f32 - 32768.0) / 32768.0
                    });
                },
                |err| eprintln!("ошибка аудио: {err}"),
                None,
            )
            .map_err(AudioError::BuildStream),
        _ => Err(AudioError::UnsupportedFormat(sample_format)),
    }
}

/// Захват сэмплов из потока, даунмикс до моно и запись в буфер.
fn capture_samples<T>(
    data: &[T],
    channels: usize,
    buffer: &Arc<Mutex<Vec<f32>>>,
    convert: impl Fn(&T) -> f32,
) {
    if let Ok(mut buf) = buffer.lock() {
        let ch = channels.max(1);
        if ch == 1 {
            buf.extend(data.iter().map(|s| convert(s) * MIC_GAIN));
        } else {
            for frame in data.chunks_exact(ch) {
                let sum: f32 = frame.iter().map(&convert).sum();
                buf.push(sum / ch as f32 * MIC_GAIN);
            }
        }
    }
}

/// Средний уровень сигнала в окне (RMS).
fn window_level(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f32 = samples.iter().map(|s| s * s).sum();
    (sum / samples.len() as f32).sqrt()
}

/// Нормализует аудио: пик приводится к target_peak (0.9).
fn normalize(samples: &mut [f32]) {
    let peak = samples
        .iter()
        .map(|s| s.abs())
        .fold(0.0f32, f32::max);
    if peak > 1e-6 {
        let scale = 0.9 / peak;
        for s in samples.iter_mut() {
            *s *= scale;
        }
    }
}

/// Линейная передискретизация.
fn resample_linear(samples: &[f32], src_rate: u32, dst_rate: u32) -> Vec<f32> {
    if src_rate == dst_rate || samples.len() < 2 {
        return samples.to_vec();
    }
    // Быстрая даунсэмплинг если частоты кратны
    if src_rate % dst_rate == 0 {
        let factor = (src_rate / dst_rate) as usize;
        let mut out = Vec::with_capacity(samples.len() / factor);
        for chunk in samples.chunks_exact(factor) {
            let sum: f32 = chunk.iter().sum();
            out.push(sum / factor as f32);
        }
        return out;
    }
    let ratio = dst_rate as f64 / src_rate as f64;
    let out_len = ((samples.len() as f64) * ratio).max(1.0) as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let pos = i as f64 / ratio;
        let idx = pos.floor() as usize;
        let frac = (pos - idx as f64) as f32;
        let s0 = samples[idx];
        let s1 = samples.get(idx + 1).copied().unwrap_or(s0);
        out.push(s0 + (s1 - s0) * frac);
    }
    out
}

/// Проверяет поддерживает ли устройство запрошенную частоту.
fn select_sample_rate(
    device: &Device,
    sample_format: SampleFormat,
    channels: u16,
    requested: u32,
) -> Option<u32> {
    let configs = device.supported_input_configs().ok()?;
    for cfg in configs {
        if cfg.sample_format() != sample_format || cfg.channels() != channels {
            continue;
        }
        if requested >= cfg.min_sample_rate().0 && requested <= cfg.max_sample_rate().0 {
            return Some(requested);
        }
    }
    None
}

#[derive(Debug)]
pub enum AudioError {
    InvalidPath,
    Ffmpeg(std::io::Error),
    FfmpegOutput(String),
    NoDevice,
    DeviceConfig(cpal::DefaultStreamConfigError),
    UnsupportedFormat(SampleFormat),
    BuildStream(cpal::BuildStreamError),
    StreamPlay(cpal::PlayStreamError),
    BufferAccess,
}

impl std::fmt::Display for AudioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPath => write!(f, "неверный путь к файлу"),
            Self::Ffmpeg(e) => write!(f, "ошибка ffmpeg: {e}"),
            Self::FfmpegOutput(s) => write!(f, "ffmpeg: {s}"),
            Self::NoDevice => write!(f, "микрофон не найден"),
            Self::DeviceConfig(e) => write!(f, "ошибка конфигурации устройства: {e}"),
            Self::UnsupportedFormat(fmt) => write!(f, "неподдерживаемый формат: {fmt:?}"),
            Self::BuildStream(e) => write!(f, "ошибка создания потока: {e}"),
            Self::StreamPlay(e) => write!(f, "ошибка запуска потока: {e}"),
            Self::BufferAccess => write!(f, "ошибка доступа к буферу"),
        }
    }
}

impl std::error::Error for AudioError {}
