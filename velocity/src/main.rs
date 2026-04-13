// Velocity — global push-to-talk transcription for Windows 11
// Hold Win+Ctrl to record; release to transcribe and type the result.

#![windows_subsystem = "windows"]

mod api_key_dialog;
mod audio;
mod beep;
mod config;
mod hotkey;
mod logger;
mod streaming;
mod transcribe;
mod tray;
mod typer;

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

use windows::core::BOOL;
use windows::Win32::System::Console::*;
use windows::Win32::UI::WindowsAndMessaging::*;

unsafe extern "system" fn ctrl_handler(ctrl_type: u32) -> BOOL {
    if ctrl_type == CTRL_C_EVENT {
        std::process::exit(0);
    }
    BOOL(0)
}

/// If the process was launched from a terminal, attach to its console and
/// register a Ctrl+C handler so the user can exit with Ctrl+C.
fn setup_ctrl_c() {
    unsafe {
        if AttachConsole(ATTACH_PARENT_PROCESS).is_ok() {
            let _ = SetConsoleCtrlHandler(Some(ctrl_handler), true);
        }
    }
}

fn main() {
    setup_ctrl_c();

    let args: Vec<String> = std::env::args().collect();
    let verbose = args.iter().any(|a| a == "--verbose");
    let smart_format_flag = args.iter().any(|a| a == "--smart-format");
    let model_flag = args.windows(2)
        .find(|w| w[0] == "--model")
        .map(|w| w[1].clone());
    logger::init(verbose);

    // Load or prompt for API key
    let mut cfg = config::load();
    let api_key = loop {
        match cfg.api_key.clone().filter(|k| !k.trim().is_empty()) {
            Some(key) => break key,
            None => match api_key_dialog::prompt_for_api_key() {
                Some(key) => {
                    cfg.api_key = Some(key.clone());
                    config::save(&cfg);
                    break key;
                }
                None => return, // user cancelled
            },
        }
    };

    // --smart-format CLI flag takes priority; otherwise fall back to config value.
    let smart_format = smart_format_flag || cfg.smart_format.unwrap_or(false);

    // --model CLI flag takes priority; then config; then default "nova-3".
    let model = model_flag
        .or_else(|| cfg.model.clone())
        .unwrap_or_else(|| "nova-3".to_string());

    let api_key = Arc::new(api_key);
    let recording = Arc::new(AtomicBool::new(false));

    // Clone model before it is moved into the audio thread closure.
    let model_for_stream = model.clone();

    let recording_for_audio = Arc::clone(&recording);
    let api_key_for_thread = Arc::clone(&api_key);

    // Open the audio device once at startup so there is no initialization
    // delay when the user presses the hotkey.  The device is shared between
    // the regular recording path and the streaming path via Arc<Mutex<>>;
    // the two modes are mutually exclusive so there is never real contention.
    let capture = Arc::new(Mutex::new(
        audio::AudioCapture::new().expect("Failed to open audio input device"),
    ));
    let capture_for_audio  = Arc::clone(&capture);
    let capture_for_stream = Arc::clone(&capture);

    // Audio thread: parked until the hook signals start, then captures + transcribes.
    let audio_thread = std::thread::spawn(move || {
        let mut samples = Vec::new();
        loop {
            std::thread::park();

            samples.clear();

            // Lock the shared device for the duration of this recording session.
            {
                let mut cap = capture_for_audio.lock().unwrap();
                cap.start();
                while recording_for_audio.load(Ordering::Relaxed) {
                    cap.collect_ready(&mut samples);
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                cap.stop(&mut samples);
            } // MutexGuard dropped — streaming mode can now acquire the device.

            if samples.is_empty() {
                logger::verbose("Audio capture returned empty — check microphone permissions");
                continue;
            }

            logger::verbose(&format!("Captured {} samples, sending to Deepgram", samples.len()));

            let wav = audio::encode_wav(&samples, 48_000);
            if let Some(text) = transcribe::transcribe(wav, &api_key_for_thread, smart_format, &model) {
                logger::verbose(&format!("Transcript: {text}"));
                typer::type_text(&text);
            }
        }
    });

    let audio_thread_handle = audio_thread.thread().clone();
    let recording_for_hook = Arc::clone(&recording);

    let on_start: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
        logger::verbose("Recording started");
        std::thread::spawn(|| beep::play_start());
        audio_thread_handle.unpark();
    });

    let on_stop: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
        logger::verbose("Recording stopped");
        beep::play_end();
    });

    // Streaming mode state and callbacks.
    let streaming_active = Arc::new(AtomicBool::new(false));
    let streaming_active_for_start = Arc::clone(&streaming_active);
    let api_key_for_stream = Arc::clone(&api_key);

    let on_stream_start: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
        logger::verbose("Streaming mode started");
        let active  = Arc::clone(&streaming_active_for_start);
        let key     = Arc::clone(&api_key_for_stream);
        let mdl     = model_for_stream.clone();
        let cap     = Arc::clone(&capture_for_stream);
        std::thread::spawn(move || {
            streaming::run(&key, smart_format, &mdl, active, cap);
        });
    });

    let hotkey_mgr = hotkey::HotkeyManager::new(
        recording_for_hook,
        on_start,
        on_stop,
        streaming_active,
        on_stream_start,
    );
    let _tray_hwnd = tray::create_tray_window();

    // Register hotkeys after the tray window exists so the message loop is ready.
    hotkey_mgr.register();

    // Main thread runs the Win32 message loop — required for RegisterHotKey.
    unsafe {
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            if msg.message == WM_HOTKEY {
                hotkey_mgr.handle(msg.wParam.0 as i32);
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
