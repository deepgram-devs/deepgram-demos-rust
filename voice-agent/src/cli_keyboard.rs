use std::{
    io::{self, IsTerminal},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use tokio::sync::mpsc;

#[derive(Clone, Copy)]
pub enum KeyboardCommand {
    ForceEndTurn,
    Stop,
}

pub struct KeyboardControls {
    pub commands: mpsc::UnboundedReceiver<KeyboardCommand>,
    pub enabled: bool,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    raw_mode: Option<RawModeGuard>,
}

impl KeyboardControls {
    pub fn start() -> Self {
        let (command_tx, commands) = mpsc::unbounded_channel();
        if !io::stdin().is_terminal() {
            return Self::disabled(commands);
        }

        if let Err(error) = enable_raw_mode() {
            log::warn!("Keyboard controls unavailable: {error}");
            return Self::disabled(commands);
        }

        let raw_mode = RawModeGuard;
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let worker = thread::Builder::new()
            .name("voice-agent-keyboard".into())
            .spawn(move || {
                while !worker_stop.load(Ordering::Relaxed) && !command_tx.is_closed() {
                    match event::poll(Duration::from_millis(100)) {
                        Ok(false) => continue,
                        Ok(true) => match event::read() {
                            Ok(Event::Key(key)) => {
                                let Some(command) = command_for_key(key) else {
                                    continue;
                                };
                                if command_tx.send(command).is_err()
                                    || matches!(command, KeyboardCommand::Stop)
                                {
                                    break;
                                }
                            }
                            Ok(_) => {}
                            Err(error) => {
                                log::warn!("Keyboard controls stopped: {error}");
                                let _ = command_tx.send(KeyboardCommand::Stop);
                                break;
                            }
                        },
                        Err(error) => {
                            log::warn!("Keyboard controls stopped: {error}");
                            let _ = command_tx.send(KeyboardCommand::Stop);
                            break;
                        }
                    }
                }
            });

        match worker {
            Ok(worker) => Self {
                commands,
                enabled: true,
                stop,
                worker: Some(worker),
                raw_mode: Some(raw_mode),
            },
            Err(error) => {
                drop(raw_mode);
                log::warn!("Unable to start keyboard controls: {error}");
                Self::disabled(commands)
            }
        }
    }

    fn disabled(commands: mpsc::UnboundedReceiver<KeyboardCommand>) -> Self {
        Self {
            commands,
            enabled: false,
            stop: Arc::new(AtomicBool::new(true)),
            worker: None,
            raw_mode: None,
        }
    }
}

impl Drop for KeyboardControls {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        self.raw_mode.take();
    }
}

struct RawModeGuard;

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        if let Err(error) = disable_raw_mode() {
            log::warn!("Unable to restore terminal input mode: {error}");
        }
    }
}

fn command_for_key(key: KeyEvent) -> Option<KeyboardCommand> {
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return Some(KeyboardCommand::Stop);
    }

    if matches!(key.code, KeyCode::Char('f' | 'F'))
        && !key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
    {
        return Some(KeyboardCommand::ForceEndTurn);
    }

    None
}
