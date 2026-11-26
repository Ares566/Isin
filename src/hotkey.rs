use rdev::{listen, Event, EventType, Key};
use std::thread;
use tokio::sync::mpsc as async_mpsc;

/// Сообщение от слушателя хоткея.
pub enum HotkeyEvent {
    /// Хоткей нажат — начать запись
    Pressed,
    /// Хоткей отпущен — остановить запись
    Released,
}

/// Глобальный слушатель клавиши Fn.
///
/// Запускает отдельный поток для перехвата клавиатурных событий через rdev.
/// Отправляет HotkeyEvent через tokio mpsc channel.
pub struct HotkeyListener;

impl HotkeyListener {
    pub fn new() -> Self {
        Self
    }

    /// Запускает слушатель в отдельном потоке.
    ///
    /// Возвращает tokio mpsc receiver для получения событий нажатия и отпускания Fn.
    pub fn start(&self) -> Result<async_mpsc::UnboundedReceiver<HotkeyEvent>, HotkeyError> {
        let (tx, rx) = async_mpsc::unbounded_channel();

        thread::spawn(move || {
            let mut fn_pressed = false;

            let callback = move |event: Event| match event.event_type {
                EventType::KeyPress(Key::Function) if !fn_pressed => {
                    fn_pressed = true;
                    let _ = tx.send(HotkeyEvent::Pressed);
                }
                EventType::KeyRelease(Key::Function) => {
                    fn_pressed = false;
                    let _ = tx.send(HotkeyEvent::Released);
                }
                _ => {}
            };

            // rdev::listen блокирует поток
            if let Err(e) = listen(callback) {
                eprintln!("ошибка rdev: {e:?}");
            }
        });

        Ok(rx)
    }
}

#[derive(Debug)]
pub enum HotkeyError {
    Listen(String),
}

impl std::fmt::Display for HotkeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Listen(e) => write!(f, "ошибка слушателя хоткея: {e}"),
        }
    }
}

impl std::error::Error for HotkeyError {}
