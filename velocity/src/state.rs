use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::SystemTime;

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;

use crate::config::{self, Config};
use crate::history::{HistoryEntry, TranscriptHistory};
use crate::hotkey::HotkeyManager;
use crate::logger;
use crate::output;

static APP_STATE: OnceLock<Arc<AppState>> = OnceLock::new();

pub struct AppState {
    config: RwLock<Config>,
    last_good_config: RwLock<Config>,
    config_modified_at: Mutex<Option<SystemTime>>,
    history: Mutex<TranscriptHistory>,
    recording: Arc<AtomicBool>,
    keep_talking: Arc<AtomicBool>,
    streaming_active: Arc<AtomicBool>,
    tray_hwnd: AtomicIsize,
    settings_hwnd: AtomicIsize,
    meter_level: AtomicUsize,
    last_error: Mutex<Option<String>>,
    hotkeys: Mutex<Option<HotkeyManager>>,
}

impl AppState {
    pub fn new(config: Config, modified_at: Option<SystemTime>) -> Self {
        Self {
            history: Mutex::new(TranscriptHistory::load(&config::history_path())),
            config: RwLock::new(config.clone()),
            last_good_config: RwLock::new(config),
            config_modified_at: Mutex::new(modified_at),
            recording: Arc::new(AtomicBool::new(false)),
            keep_talking: Arc::new(AtomicBool::new(false)),
            streaming_active: Arc::new(AtomicBool::new(false)),
            tray_hwnd: AtomicIsize::new(0),
            settings_hwnd: AtomicIsize::new(0),
            meter_level: AtomicUsize::new(0),
            last_error: Mutex::new(None),
            hotkeys: Mutex::new(None),
        }
    }

    pub fn config(&self) -> Config {
        self.config.read().unwrap().clone()
    }

    pub fn last_good_config(&self) -> Config {
        self.last_good_config.read().unwrap().clone()
    }

    pub fn recording_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.recording)
    }

    pub fn keep_talking_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.keep_talking)
    }

    pub fn streaming_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.streaming_active)
    }

    pub fn set_tray_hwnd(&self, hwnd: HWND) {
        self.tray_hwnd.store(hwnd.0 as isize, Ordering::Relaxed);
    }

    pub fn tray_hwnd(&self) -> Option<HWND> {
        let raw = self.tray_hwnd.load(Ordering::Relaxed);
        (raw != 0).then_some(HWND(raw as *mut _))
    }

    pub fn set_settings_hwnd(&self, hwnd: Option<HWND>) {
        let raw = hwnd.map(|hwnd| hwnd.0 as isize).unwrap_or(0);
        self.settings_hwnd.store(raw, Ordering::Relaxed);
    }

    pub fn settings_hwnd(&self) -> Option<HWND> {
        let raw = self.settings_hwnd.load(Ordering::Relaxed);
        (raw != 0).then_some(HWND(raw as *mut _))
    }

    pub fn set_meter_level(&self, level: u8) {
        self.meter_level.store(level as usize, Ordering::Relaxed);
    }

    pub fn meter_level(&self) -> u8 {
        self.meter_level.load(Ordering::Relaxed) as u8
    }

    pub fn set_recording(&self, value: bool) {
        self.recording.store(value, Ordering::Relaxed);
        self.notify_ui();
    }

    pub fn set_keep_talking(&self, value: bool) {
        self.keep_talking.store(value, Ordering::Relaxed);
        self.notify_ui();
    }

    pub fn set_streaming(&self, value: bool) {
        self.streaming_active.store(value, Ordering::Relaxed);
        self.notify_ui();
    }

    pub fn is_recording(&self) -> bool {
        self.recording.load(Ordering::Relaxed)
    }

    pub fn is_keep_talking(&self) -> bool {
        self.keep_talking.load(Ordering::Relaxed)
    }

    pub fn is_streaming(&self) -> bool {
        self.streaming_active.load(Ordering::Relaxed)
    }

    pub fn apply_config(&self, config: Config, modified_at: Option<SystemTime>) {
        {
            *self.config.write().unwrap() = config.clone();
            *self.last_good_config.write().unwrap() = config.clone();
            *self.config_modified_at.lock().unwrap() = modified_at;
        }

        {
            let mut history = self.history.lock().unwrap();
            history.trim_to(config.history_limit);
            if let Err(error) = history.save(&config::history_path()) {
                logger::verbose(&format!("Failed to persist history trim: {error}"));
            }
        }

        let _ = config::ensure_backup(&config);
        self.clear_error();
        self.notify_ui();
    }

    pub fn config_modified_at(&self) -> Option<SystemTime> {
        *self.config_modified_at.lock().unwrap()
    }

    pub fn push_history_and_deliver(&self, text: String) {
        let config = self.config();
        match output::deliver_text(&text, config.output_mode, config.append_newline) {
            Ok(_) => {
                let mut history = self.history.lock().unwrap();
                history.push(text, config.history_limit);
                if let Err(error) = history.save(&config::history_path()) {
                    self.set_error(format!("Failed to save transcript history: {error}"));
                }
                self.notify_ui();
            }
            Err(error) => self.set_error(error),
        }
    }

    pub fn recent_entries(&self) -> Vec<HistoryEntry> {
        self.history.lock().unwrap().entries.clone()
    }

    pub fn selected_history_index(&self) -> Option<usize> {
        self.history.lock().unwrap().selected_index
    }

    pub fn select_history(&self, index: usize) -> Option<String> {
        let mut history = self.history.lock().unwrap();
        let selected = history.select(index)?.text.clone();
        let _ = history.save(&config::history_path());
        self.notify_ui();
        Some(selected)
    }

    pub fn resend_selected(&self) {
        if let Some(text) = self.history.lock().unwrap().selected_text().map(|text| text.to_string()) {
            let config = self.config();
            if let Err(error) = output::deliver_text(&text, config.output_mode, config.append_newline) {
                self.set_error(error);
            }
        } else {
            self.set_error("No recent transcript is selected".to_string());
        }
    }

    pub fn last_error(&self) -> Option<String> {
        self.last_error.lock().unwrap().clone()
    }

    pub fn set_error(&self, error: String) {
        logger::verbose(&error);
        *self.last_error.lock().unwrap() = Some(error);
        self.notify_ui();
    }

    pub fn clear_error(&self) {
        *self.last_error.lock().unwrap() = None;
    }

    pub fn set_hotkeys(&self, hotkeys: HotkeyManager) {
        *self.hotkeys.lock().unwrap() = Some(hotkeys);
    }

    pub fn with_hotkeys<R>(&self, f: impl FnOnce(&mut HotkeyManager) -> R) -> Option<R> {
        let mut guard = self.hotkeys.lock().unwrap();
        let hotkeys = guard.as_mut()?;
        Some(f(hotkeys))
    }

    pub fn notify_ui(&self) {
        if let Some(hwnd) = self.tray_hwnd() {
            unsafe {
                let _ = PostMessageW(Some(hwnd), crate::tray::WM_APP_REFRESH_TRAY, WPARAM(0), LPARAM(0));
            }
        }
        if let Some(hwnd) = self.settings_hwnd() {
            unsafe {
                let _ = PostMessageW(Some(hwnd), crate::settings::WM_APP_SETTINGS_REFRESH, WPARAM(0), LPARAM(0));
            }
        }
    }
}

pub fn install_global(state: Arc<AppState>) {
    let _ = APP_STATE.set(state);
}

pub fn global() -> Arc<AppState> {
    APP_STATE.get().expect("App state not installed").clone()
}
