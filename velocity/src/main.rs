// Velocity — global push-to-talk transcription for Windows 11
// Hold Win+Ctrl to record; release to transcribe and type the result.

#![windows_subsystem = "windows"]

mod api_key_dialog;
mod audio;
mod beep;
mod clipboard;
mod config;
mod history;
mod hotkey;
mod logger;
mod output;
mod settings;
mod sidecar_ui;
mod state;
mod streaming;
mod transcribe;
mod tray;
mod typer;

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use windows::core::BOOL;
use windows::Win32::System::Console::*;
use windows::Win32::Foundation::{LPARAM, WPARAM};
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

    let loaded_state = config::load_state().unwrap_or_else(|error| {
        logger::verbose(&format!("Config load failed: {error}"));
        config::ConfigFileState {
            config: config::Config::default(),
            modified_at: None,
        }
    });
    let mut cfg = loaded_state.config;
    if smart_format_flag {
        cfg.smart_format = true;
    }
    if let Some(model) = model_flag {
        cfg.model = model;
    }

    let _api_key = loop {
        match cfg.api_key.clone().filter(|k| !k.trim().is_empty()) {
            Some(key) => break key,
            None => match api_key_dialog::prompt_for_api_key() {
                Some(key) => {
                    cfg.api_key = Some(key.clone());
                    let _ = config::save(&cfg);
                    break key;
                }
                None => return,
            },
        }
    };

    let _ = config::ensure_backup(&cfg);

    let app = Arc::new(state::AppState::new(cfg.clone(), loaded_state.modified_at));
    state::install_global(Arc::clone(&app));

    let tray_hwnd = tray::create_tray_window();
    app.set_tray_hwnd(tray_hwnd);

    let recording = app.recording_flag();
    let keep_talking = app.keep_talking_flag();
    let streaming_active = app.streaming_flag();

    let audio_thread_app = Arc::clone(&app);
    let audio_thread_recording = Arc::clone(&recording);
    let audio_thread = std::thread::spawn(move || {
        let mut samples = Vec::new();
        loop {
            std::thread::park();
            if !audio_thread_recording.load(Ordering::Relaxed) {
                continue;
            }

            samples.clear();
            let config = audio_thread_app.config();
            let api_key = match config.api_key.clone().filter(|key| !key.trim().is_empty()) {
                Some(key) => key,
                None => {
                    audio_thread_app.set_error("Deepgram API key is not configured".to_string());
                    audio_thread_app.set_recording(false);
                    continue;
                }
            };

            let mut capture = match audio::AudioCapture::new(config.audio_input.as_deref()) {
                Some(capture) => capture,
                None => {
                    audio_thread_app.set_error("Failed to open audio input device".to_string());
                    audio_thread_app.set_recording(false);
                    continue;
                }
            };

            if capture.actual_device.fell_back_to_default {
                logger::verbose(&format!(
                    "Requested microphone {:?} unavailable, using {}",
                    capture.actual_device.requested_name,
                    capture.actual_device.actual_name
                ));
            }

            capture.start();
            while audio_thread_recording.load(Ordering::Relaxed) {
                let start = samples.len();
                capture.collect_ready(&mut samples);
                if samples.len() > start {
                    audio_thread_app.set_meter_level(audio::peak_level_percent(&samples[start..]));
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            capture.stop(&mut samples);
            audio_thread_app.set_meter_level(0);

            if samples.is_empty() {
                logger::verbose("Audio capture returned empty - check microphone permissions");
                continue;
            }

            logger::verbose(&format!("Captured {} samples, sending to Deepgram", samples.len()));
            let wav = audio::encode_wav(&samples, 48_000);
            if let Some(text) = transcribe::transcribe(
                wav,
                &api_key,
                config.smart_format,
                &config.model,
                &config.key_terms,
            ) {
                logger::verbose(&format!("Transcript: {text}"));
                audio_thread_app.push_history_and_deliver(text);
            }
        }
    });

    let audio_thread_handle = audio_thread.thread().clone();
    let on_start_app = Arc::clone(&app);
    let on_start: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
        logger::verbose("Recording started");
        on_start_app.set_recording(true);
        std::thread::spawn(|| beep::play_start());
        audio_thread_handle.unpark();
    });

    let on_stop_app = Arc::clone(&app);
    let keep_flag_for_stop = Arc::clone(&keep_talking);
    let on_stop: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
        logger::verbose("Recording stopped");
        on_stop_app.set_recording(false);
        if !keep_flag_for_stop.load(Ordering::Relaxed) {
            on_stop_app.set_keep_talking(false);
        }
        on_stop_app.set_meter_level(0);
        beep::play_end();
    });

    let on_stream_app = Arc::clone(&app);
    let on_stream_start: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
        logger::verbose("Streaming mode started");
        on_stream_app.set_streaming(true);
        let stream_app = Arc::clone(&on_stream_app);
        std::thread::spawn(move || {
            let config = stream_app.config();
            let api_key = match config.api_key.clone().filter(|key| !key.trim().is_empty()) {
                Some(key) => key,
                None => {
                    stream_app.set_error("Deepgram API key is not configured".to_string());
                    stream_app.set_streaming(false);
                    return;
                }
            };

            let transcript_target = Arc::clone(&stream_app);
            let on_transcript: Arc<dyn Fn(String) + Send + Sync> = Arc::new(move |text| {
                transcript_target.push_history_and_deliver(format!("{text} "));
            });

            streaming::run(
                &api_key,
                config.smart_format,
                &config.model,
                &config.key_terms,
                config.audio_input.as_deref(),
                stream_app.streaming_flag(),
                on_transcript,
            );

            stream_app.set_streaming(false);
            stream_app.set_meter_level(0);
        });
    });

    let on_resend_app = Arc::clone(&app);
    let on_resend_selected: Arc<dyn Fn() + Send + Sync> =
        Arc::new(move || on_resend_app.resend_selected());

    let mut hotkeys = hotkey::HotkeyManager::new(
        tray_hwnd,
        cfg.hotkeys.clone(),
        Arc::clone(&recording),
        Arc::clone(&keep_talking),
        on_start,
        on_stop,
        Arc::clone(&streaming_active),
        on_stream_start,
        on_resend_selected,
    );
    if let Err(error) = hotkeys.register() {
        app.set_error(error);
    }
    app.set_hotkeys(hotkeys);

    spawn_config_watcher(Arc::clone(&app));

    unsafe {
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

fn spawn_config_watcher(app: Arc<state::AppState>) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(1));
        let Some(hwnd) = app.tray_hwnd() else {
            continue;
        };

        let modified_at = std::fs::metadata(config::config_path())
            .ok()
            .and_then(|meta| meta.modified().ok());
        if modified_at.is_some() && modified_at != app.config_modified_at() {
            unsafe {
                let _ = PostMessageW(Some(hwnd), tray::WM_APP_RELOAD_CONFIG, WPARAM(0), LPARAM(0));
            }
        }
    });
}
