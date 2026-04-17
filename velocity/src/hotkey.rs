use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::*;

use crate::config::HotkeyConfig;

pub const HOTKEY_PTT: i32 = 1;
pub const HOTKEY_KT: i32 = 2;
pub const HOTKEY_STREAM: i32 = 3;
pub const HOTKEY_RESEND: i32 = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotkeyBinding {
    pub modifiers: HOT_KEY_MODIFIERS,
    pub vk: u32,
    pub display: String,
}

pub struct HotkeyManager {
    hwnd: HWND,
    recording: Arc<AtomicBool>,
    on_start: Arc<dyn Fn() + Send + Sync>,
    on_stop: Arc<dyn Fn() + Send + Sync>,
    keep_talking: Arc<AtomicBool>,
    streaming_active: Arc<AtomicBool>,
    on_stream_start: Arc<dyn Fn() + Send + Sync>,
    on_resend_selected: Arc<dyn Fn() + Send + Sync>,
    active_config: HotkeyConfig,
}

unsafe impl Send for HotkeyManager {}

impl HotkeyManager {
    pub fn new(
        hwnd: HWND,
        config: HotkeyConfig,
        recording: Arc<AtomicBool>,
        keep_talking: Arc<AtomicBool>,
        on_start: Arc<dyn Fn() + Send + Sync>,
        on_stop: Arc<dyn Fn() + Send + Sync>,
        streaming_active: Arc<AtomicBool>,
        on_stream_start: Arc<dyn Fn() + Send + Sync>,
        on_resend_selected: Arc<dyn Fn() + Send + Sync>,
    ) -> Self {
        Self {
            hwnd,
            recording,
            keep_talking,
            on_start,
            on_stop,
            streaming_active,
            on_stream_start,
            on_resend_selected,
            active_config: config,
        }
    }

    pub fn register(&mut self) -> Result<(), String> {
        self.apply_config(self.active_config.clone())
    }

    pub fn apply_config(&mut self, config: HotkeyConfig) -> Result<(), String> {
        let parsed = [
            (HOTKEY_PTT, parse_hotkey(&config.push_to_talk)?),
            (HOTKEY_KT, parse_hotkey(&config.keep_talking)?),
            (HOTKEY_STREAM, parse_hotkey(&config.streaming)?),
            (HOTKEY_RESEND, parse_hotkey(&config.resend_selected)?),
        ];

        unsafe {
            self.unregister_all();
            for (id, binding) in parsed {
                RegisterHotKey(
                    Some(self.hwnd),
                    id,
                    binding.modifiers | MOD_NOREPEAT,
                    binding.vk,
                )
                .map_err(|e| format!("Failed to register {}: {e}", binding.display))?;
            }
        }

        self.active_config = config;
        Ok(())
    }

    pub fn config(&self) -> HotkeyConfig {
        self.active_config.clone()
    }

    pub fn handle(&self, id: i32) {
        let was_recording = self.recording.load(Ordering::Relaxed);
        let keep = self.keep_talking.load(Ordering::Relaxed);
        let streaming = self.streaming_active.load(Ordering::Relaxed);

        match id {
            HOTKEY_STREAM => {
                if streaming {
                    self.streaming_active.store(false, Ordering::Relaxed);
                } else if !was_recording && !keep {
                    self.streaming_active.store(true, Ordering::Relaxed);
                    (self.on_stream_start)();
                }
            }
            HOTKEY_KT => {
                if streaming {
                    return;
                }
                if keep {
                    self.keep_talking.store(false, Ordering::Relaxed);
                    self.recording.store(false, Ordering::Relaxed);
                    (self.on_stop)();
                } else if was_recording {
                    self.keep_talking.store(true, Ordering::Relaxed);
                } else {
                    self.keep_talking.store(true, Ordering::Relaxed);
                    self.recording.store(true, Ordering::Relaxed);
                    (self.on_start)();
                }
            }
            HOTKEY_PTT => {
                if streaming {
                    return;
                }
                if !keep && !was_recording {
                    self.recording.store(true, Ordering::Relaxed);
                    (self.on_start)();
                    self.spawn_release_watcher();
                }
            }
            HOTKEY_RESEND => (self.on_resend_selected)(),
            _ => {}
        }
    }

    fn spawn_release_watcher(&self) {
        let recording = Arc::clone(&self.recording);
        let keep_talking = Arc::clone(&self.keep_talking);
        let on_stop = Arc::clone(&self.on_stop);

        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_millis(10));

            if keep_talking.load(Ordering::Relaxed) || !recording.load(Ordering::Relaxed) {
                break;
            }

            unsafe {
                let win_held = GetAsyncKeyState(VK_LWIN.0 as i32) < 0
                    || GetAsyncKeyState(VK_RWIN.0 as i32) < 0;
                let ctrl_held = GetAsyncKeyState(VK_LCONTROL.0 as i32) < 0
                    || GetAsyncKeyState(VK_RCONTROL.0 as i32) < 0;

                if !win_held || !ctrl_held {
                    recording.store(false, Ordering::Relaxed);
                    on_stop();
                    break;
                }
            }
        });
    }

    fn unregister_all(&self) {
        unsafe {
            let _ = UnregisterHotKey(Some(self.hwnd), HOTKEY_PTT);
            let _ = UnregisterHotKey(Some(self.hwnd), HOTKEY_KT);
            let _ = UnregisterHotKey(Some(self.hwnd), HOTKEY_STREAM);
            let _ = UnregisterHotKey(Some(self.hwnd), HOTKEY_RESEND);
        }
    }
}

impl Drop for HotkeyManager {
    fn drop(&mut self) {
        self.unregister_all();
    }
}

pub fn parse_hotkey(text: &str) -> Result<HotkeyBinding, String> {
    let parts = text
        .split('+')
        .map(|segment| segment.trim())
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();

    if parts.len() < 2 {
        return Err(format!("Hotkey must include at least one modifier and one key: {text}"));
    }

    let mut modifiers = HOT_KEY_MODIFIERS(0);
    for part in &parts[..parts.len() - 1] {
        modifiers |= parse_modifier(part)?;
    }

    if modifiers.0 == 0 {
        return Err(format!("Hotkey must include a modifier: {text}"));
    }

    let key = parts[parts.len() - 1];
    let vk = parse_key(key)?;
    Ok(HotkeyBinding {
        modifiers,
        vk,
        display: format_hotkey(modifiers, vk),
    })
}

pub fn format_hotkey(modifiers: HOT_KEY_MODIFIERS, vk: u32) -> String {
    let mut parts = Vec::new();
    if modifiers.contains(MOD_CONTROL) {
        parts.push("Ctrl".to_string());
    }
    if modifiers.contains(MOD_SHIFT) {
        parts.push("Shift".to_string());
    }
    if modifiers.contains(MOD_ALT) {
        parts.push("Alt".to_string());
    }
    if modifiers.contains(MOD_WIN) {
        parts.push("Win".to_string());
    }
    parts.push(format_key(vk));
    parts.join("+")
}

fn parse_modifier(part: &str) -> Result<HOT_KEY_MODIFIERS, String> {
    match part.to_ascii_lowercase().as_str() {
        "ctrl" | "control" => Ok(MOD_CONTROL),
        "shift" => Ok(MOD_SHIFT),
        "alt" => Ok(MOD_ALT),
        "win" | "windows" => Ok(MOD_WIN),
        _ => Err(format!("Unsupported modifier: {part}")),
    }
}

fn parse_key(part: &str) -> Result<u32, String> {
    let upper = part.trim().to_ascii_uppercase();
    match upper.as_str() {
        "[" => Ok(VK_OEM_4.0 as u32),
        "]" => Ok(VK_OEM_6.0 as u32),
        "'" => Ok(VK_OEM_7.0 as u32),
        ";" => Ok(VK_OEM_1.0 as u32),
        "\\" => Ok(VK_OEM_5.0 as u32),
        "," => Ok(VK_OEM_COMMA.0 as u32),
        "." => Ok(VK_OEM_PERIOD.0 as u32),
        "/" => Ok(VK_OEM_2.0 as u32),
        "-" => Ok(VK_OEM_MINUS.0 as u32),
        "=" => Ok(VK_OEM_PLUS.0 as u32),
        "SPACE" => Ok(VK_SPACE.0 as u32),
        "ENTER" => Ok(VK_RETURN.0 as u32),
        "TAB" => Ok(VK_TAB.0 as u32),
        "ESC" | "ESCAPE" => Ok(VK_ESCAPE.0 as u32),
        _ if upper.starts_with('F') => {
            let index = upper[1..]
                .parse::<u32>()
                .map_err(|_| format!("Unsupported function key: {part}"))?;
            if (1..=24).contains(&index) {
                Ok(VK_F1.0 as u32 + (index - 1))
            } else {
                Err(format!("Unsupported function key: {part}"))
            }
        }
        _ if upper.len() == 1 => {
            let c = upper.chars().next().unwrap();
            if c.is_ascii_alphanumeric() {
                Ok(c as u32)
            } else {
                Err(format!("Unsupported hotkey key: {part}"))
            }
        }
        _ => Err(format!("Unsupported hotkey key: {part}")),
    }
}

fn format_key(vk: u32) -> String {
    match vk {
        x if x == VK_OEM_4.0 as u32 => "[".to_string(),
        x if x == VK_OEM_6.0 as u32 => "]".to_string(),
        x if x == VK_OEM_7.0 as u32 => "'".to_string(),
        x if x == VK_OEM_1.0 as u32 => ";".to_string(),
        x if x == VK_OEM_5.0 as u32 => "\\".to_string(),
        x if x == VK_OEM_COMMA.0 as u32 => ",".to_string(),
        x if x == VK_OEM_PERIOD.0 as u32 => ".".to_string(),
        x if x == VK_OEM_2.0 as u32 => "/".to_string(),
        x if x == VK_OEM_MINUS.0 as u32 => "-".to_string(),
        x if x == VK_OEM_PLUS.0 as u32 => "=".to_string(),
        x if x == VK_SPACE.0 as u32 => "Space".to_string(),
        x if x == VK_RETURN.0 as u32 => "Enter".to_string(),
        x if x == VK_TAB.0 as u32 => "Tab".to_string(),
        x if x == VK_ESCAPE.0 as u32 => "Escape".to_string(),
        x if (VK_F1.0 as u32..=VK_F24.0 as u32).contains(&x) => format!("F{}", x - VK_F1.0 as u32 + 1),
        x if char::from_u32(x).is_some_and(|c| c.is_ascii_alphanumeric()) => {
            char::from_u32(x).unwrap_or('?').to_string()
        }
        _ => format!("VK({vk})"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_default_hotkeys() {
        let ptt = parse_hotkey("Win+Ctrl+'").unwrap();
        assert!(ptt.modifiers.contains(MOD_WIN));
        assert!(ptt.modifiers.contains(MOD_CONTROL));
        assert_eq!(ptt.vk, VK_OEM_7.0 as u32);

        let resend = parse_hotkey("Win+Ctrl+]").unwrap();
        assert_eq!(resend.vk, VK_OEM_6.0 as u32);
    }

    #[test]
    fn rejects_invalid_hotkeys() {
        assert!(parse_hotkey("A").is_err());
        assert!(parse_hotkey("Win+Ctrl+").is_err());
        assert!(parse_hotkey("Win+Hyper+P").is_err());
    }

    #[test]
    fn formats_hotkeys_consistently() {
        let binding = parse_hotkey("Ctrl+Win+[").unwrap();
        assert_eq!(binding.display, "Ctrl+Win+[");
    }
}
