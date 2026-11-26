use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};

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

/// Запись с микрофона, управляемая явным запуском и остановкой.
pub struct LiveCapture {
    stream: cpal::Stream,
    device_rate: u32,
    buffer: Arc<Mutex<Vec<f32>>>,
}

impl LiveCapture {
    pub fn start() -> Result<Self, AudioError> {
        let host = cpal::default_host();
        let device = host.default_input_device().ok_or(AudioError::NoDevice)?;

        let supported = device
            .default_input_config()
            .map_err(AudioError::DeviceConfig)?;

        let sample_format = supported.sample_format();
        let mut config: StreamConfig = supported.config().clone();
        let channels = config.channels as usize;

        if let Some(rate) = select_sample_rate(&device, sample_format, config.channels, 16000) {
            config.sample_rate = SampleRate(rate);
        }

        let device_rate = config.sample_rate.0;
        let buffer: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
        let stream = build_input_stream(&device, &config, sample_format, channels, &buffer)?;
        stream.play().map_err(AudioError::StreamPlay)?;

        Ok(Self {
            stream,
            device_rate,
            buffer,
        })
    }

    /// Останавливает запись и возвращает PCM 16kHz mono.
    pub fn stop(self) -> Result<Vec<f32>, AudioError> {
        drop(self.stream);

        let mut data = self
            .buffer
            .lock()
            .map_err(|_| AudioError::BufferAccess)?
            .clone();

        if self.device_rate != 16000 && !data.is_empty() {
            data = resample_linear(&data, self.device_rate, 16000);
        }

        if !data.is_empty() {
            normalize(&mut data);
        }

        Ok(data)
    }
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
                    capture_samples(data, channels, &writer, |s| (*s as f32 - 32768.0) / 32768.0);
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

/// Нормализует аудио: пик приводится к target_peak (0.9).
fn normalize(samples: &mut [f32]) {
    let peak = samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
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
