use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use windows::Win32::UI::Input::KeyboardAndMouse::*;

pub const HOTKEY_PTT:    i32 = 1; // WIN+CTRL+'       push-to-talk (hold to record)
pub const HOTKEY_KT:     i32 = 2; // SHIFT+CTRL+WIN+' keep-talking toggle
pub const HOTKEY_STREAM: i32 = 3; // CTRL+WIN+[       streaming mode toggle

pub struct HotkeyManager {
    recording:        Arc<AtomicBool>,
    on_start:         Arc<dyn Fn() + Send + Sync>,
    on_stop:          Arc<dyn Fn() + Send + Sync>,
    keep_talking:     Arc<AtomicBool>,
    streaming_active: Arc<AtomicBool>,
    on_stream_start:  Arc<dyn Fn() + Send + Sync>,
}

impl HotkeyManager {
    pub fn new(
        recording:        Arc<AtomicBool>,
        on_start:         Arc<dyn Fn() + Send + Sync>,
        on_stop:          Arc<dyn Fn() + Send + Sync>,
        streaming_active: Arc<AtomicBool>,
        on_stream_start:  Arc<dyn Fn() + Send + Sync>,
    ) -> Self {
        Self {
            recording,
            on_start,
            on_stop,
            keep_talking:    Arc::new(AtomicBool::new(false)),
            streaming_active,
            on_stream_start,
        }
    }

    /// Register all global hotkeys with the OS.
    /// Must be called from the thread that runs the Win32 message loop,
    /// because WM_HOTKEY is posted to the registering thread's queue.
    pub fn register(&self) {
        unsafe {
            RegisterHotKey(
                None,
                HOTKEY_PTT,
                MOD_CONTROL | MOD_WIN | MOD_NOREPEAT,
                VK_OEM_7.0 as u32, // ' / "
            ).expect("Failed to register WIN+CTRL+' hotkey");

            RegisterHotKey(
                None,
                HOTKEY_KT,
                MOD_CONTROL | MOD_WIN | MOD_SHIFT | MOD_NOREPEAT,
                VK_OEM_7.0 as u32,
            ).expect("Failed to register SHIFT+CTRL+WIN+' hotkey");

            RegisterHotKey(
                None,
                HOTKEY_STREAM,
                MOD_CONTROL | MOD_WIN | MOD_NOREPEAT,
                VK_OEM_4.0 as u32, // [
            ).expect("Failed to register CTRL+WIN+[ hotkey");
        }
    }

    /// Handle a WM_HOTKEY message. Pass `msg.wParam.0 as i32` as `id`.
    pub fn handle(&self, id: i32) {
        let was_recording = self.recording.load(Ordering::Relaxed);
        let keep          = self.keep_talking.load(Ordering::Relaxed);
        let streaming     = self.streaming_active.load(Ordering::Relaxed);

        match id {
            // CTRL+WIN+[ — toggle streaming mode
            HOTKEY_STREAM => {
                if streaming {
                    // Already streaming → stop.
                    self.streaming_active.store(false, Ordering::Relaxed);
                } else if !was_recording && !keep {
                    // Idle → start streaming.
                    self.streaming_active.store(true, Ordering::Relaxed);
                    (self.on_stream_start)();
                }
            }

            // SHIFT+CTRL+WIN+' — toggle keep-talking mode
            // Blocked while streaming is active.
            HOTKEY_KT => {
                if streaming { return; }
                if keep {
                    // Already in keep-talking → stop.
                    self.keep_talking.store(false, Ordering::Relaxed);
                    self.recording.store(false, Ordering::Relaxed);
                    (self.on_stop)();
                } else if was_recording {
                    // Mid push-to-talk → promote to keep-talking without
                    // interrupting the recording.
                    self.keep_talking.store(true, Ordering::Relaxed);
                } else {
                    // Idle → enter keep-talking and start recording.
                    self.keep_talking.store(true, Ordering::Relaxed);
                    self.recording.store(true, Ordering::Relaxed);
                    (self.on_start)();
                }
            }

            // WIN+CTRL+' — push-to-talk: start recording and watch for release
            // Blocked while streaming is active.
            HOTKEY_PTT => {
                if streaming { return; }
                if !keep && !was_recording {
                    self.recording.store(true, Ordering::Relaxed);
                    (self.on_start)();
                    self.spawn_release_watcher();
                }
            }

            _ => {}
        }
    }

    /// Spawn a thread that polls GetAsyncKeyState until WIN or CTRL is
    /// released, then signals stop. Exits early if keep-talking is activated
    /// or recording is cleared by another code path.
    fn spawn_release_watcher(&self) {
        let recording    = Arc::clone(&self.recording);
        let keep_talking = Arc::clone(&self.keep_talking);
        let on_stop      = Arc::clone(&self.on_stop);

        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_millis(10));

            // Keep-talking was activated — hand off; don't stop the recording.
            if keep_talking.load(Ordering::Relaxed) {
                break;
            }

            // Recording was stopped by another path.
            if !recording.load(Ordering::Relaxed) {
                break;
            }

            unsafe {
                let win_held  = GetAsyncKeyState(VK_LWIN.0 as i32) < 0
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
}

impl Drop for HotkeyManager {
    fn drop(&mut self) {
        unsafe {
            let _ = UnregisterHotKey(None, HOTKEY_PTT);
            let _ = UnregisterHotKey(None, HOTKEY_KT);
            let _ = UnregisterHotKey(None, HOTKEY_STREAM);
        }
    }
}
