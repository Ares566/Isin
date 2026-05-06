use rdev::{listen, Event, EventType, Key};
use std::sync::mpsc;
use std::thread;
use tokio::sync::mpsc as async_mpsc;

/// Сообщение от слушателя хоткея.
pub enum HotkeyEvent {
    /// Хоткей нажат — начать запись
    Pressed,
    /// Хоткей отпущен — остановить запись
    Released,
}

/// Глобальный слушатель хоткея.
///
/// Запускает отдельный поток для перехвата клавиатурных событий через rdev.
/// Отправляет HotkeyEvent через tokio mpsc channel.
pub struct HotkeyListener {
    /// Ключ-модификатор 1 (например, MetaLeft = Cmd)
    modifier1: Key,
    /// Ключ-модификатор 2 (например, ShiftLeft)
    modifier2: Key,
    /// Основная клавиша (например, Space)
    trigger: Key,
}

impl HotkeyListener {
    /// Создаёт слушателя с комбинацией по умолчанию: Cmd+Shift+Space.
    pub fn new() -> Self {
        Self {
            modifier1: Key::MetaLeft,
            modifier2: Key::ShiftLeft,
            trigger: Key::Space,
        }
    }

    /// Запускает слушатель в отдельном потоке.
    ///
    /// Возвращает tokio mpsc receiver для получения событий хоткея.
    pub fn start(&self) -> Result<async_mpsc::UnboundedReceiver<HotkeyEvent>, HotkeyError> {
        let (tx, rx) = async_mpsc::unbounded_channel();
        let (ready_tx, ready_rx) = mpsc::channel::<()>();

        let mod1 = self.modifier1;
        let mod2 = self.modifier2;
        let trigger = self.trigger;

        thread::spawn(move || {
            // Состояние модификаторов
            let mut mod1_pressed = false;
            let mut mod2_pressed = false;
            let mut trigger_pressed = false;

            let callback = move |event: Event| {
                let (tx, mod1, mod2, trigger) = (&tx, mod1, mod2, trigger);
                match event.event_type {
                    EventType::KeyPress(key) => {
                        if key == mod1 { mod1_pressed = true; }
                        if key == mod2 { mod2_pressed = true; }
                        if key == trigger && mod1_pressed && mod2_pressed && !trigger_pressed {
                            trigger_pressed = true;
                            let _ = tx.send(HotkeyEvent::Pressed);
                        }
                    }
                    EventType::KeyRelease(key) => {
                        if key == mod1 { mod1_pressed = false; }
                        if key == mod2 { mod2_pressed = false; }
                        if key == trigger { trigger_pressed = false; }
                    }
                    _ => {}
                }
            };

            // rdev::listen блокирует поток
            if let Err(e) = listen(callback) {
                eprintln!("ошибка rdev: {e:?}");
            }
        });

        // Даём потоку немного времени на запуск
        drop(ready_tx);
        let _ = ready_rx.recv();

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
