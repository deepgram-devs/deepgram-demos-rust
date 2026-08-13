mod app;
mod audio;
mod config;
mod logging;
mod persistence;
mod sagemaker;
mod theme;
mod tts;
mod tts_streaming;
mod ui;

use anyhow::Result;
use app::{App, CommandAction, CurrentScreen, Panel, AUDIO_FORMATS, DEFAULT_FORMAT_INDEX};
use clap::Parser;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event as CrosstermEvent, KeyCode,
        KeyModifiers, KeyboardEnhancementFlags, MouseEventKind, PopKeyboardEnhancementFlags,
        PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, supports_keyboard_enhancement, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{io, time::Duration};
use tokio_util::sync::CancellationToken;

const DEFAULT_REQUEST_TAGS: [&str; 3] = ["tts-tui", "appeng", "deepgram-demos-rust"];

#[derive(Parser, Debug)]
#[command(name = "tts-tui")]
#[command(about = "A Deepgram TTS terminal user interface")]
#[command(version)]
struct Args {
    /// Custom hosted or self-hosted Deepgram-compatible endpoint URL for TTS (overrides config file and env var)
    #[arg(long, env = "DEEPGRAM_TTS_ENDPOINT")]
    endpoint: Option<String>,

    /// TTS provider: deepgram or sagemaker (overrides config file and env var)
    #[arg(long, env = "TTS_TUI_PROVIDER")]
    provider: Option<String>,

    /// SageMaker endpoint name for self-hosted Deepgram TTS (overrides config file and env var)
    #[arg(long, env = "SAGEMAKER_ENDPOINT_NAME")]
    sagemaker_endpoint_name: Option<String>,

    /// AWS region for the SageMaker endpoint (overrides config file and env var)
    #[arg(long, env = "AWS_REGION")]
    aws_region: Option<String>,

    /// TTS audio encoding format (overrides config file and env var)
    /// Valid values: mp3, linear16, mulaw, alaw, opus, flac, aac
    #[arg(long, env = "DEEPGRAM_AUDIO_FORMAT")]
    audio_format: Option<String>,

    /// TTS output sample rate in Hz (overrides config file and env var)
    /// Valid values depend on the chosen format
    #[arg(long, env = "DEEPGRAM_SAMPLE_RATE")]
    sample_rate: Option<u32>,

    /// Maximum active log size in bytes before rotation (overrides config file and env var)
    #[arg(long, env = "TTS_TUI_LOG_MAX_SIZE_BYTES")]
    log_max_size_bytes: Option<u64>,

    /// Maximum number of log files to retain, including the active log (overrides config file and env var)
    #[arg(long, env = "TTS_TUI_LOG_MAX_FILES")]
    log_max_files: Option<usize>,

    /// Additional Deepgram request tags; may be repeated or comma-separated
    #[arg(long, value_delimiter = ',')]
    tags: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    // The workspace enables Rustls through multiple networking clients. Select
    // the ring provider explicitly before any HTTP or WebSocket TLS connection
    // is constructed so Rustls does not panic over ambiguous providers.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let args = Args::parse();
    let mut request_tags = DEFAULT_REQUEST_TAGS
        .iter()
        .map(|tag| (*tag).to_string())
        .collect::<Vec<_>>();
    request_tags.extend(args.tags);
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    // Enable kitty keyboard protocol if the terminal supports it.
    // DISAMBIGUATE_ESCAPE_CODES ensures the ESC key is reported as a
    // distinct event and is not confused with the start of a mouse or
    // other escape sequence.
    let kbd_enhancement = supports_keyboard_enhancement().unwrap_or(false);
    if kbd_enhancement {
        let _ = execute!(
            stdout,
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        );
    }

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Load config, then create app
    let mut app_config = config::load();
    if let Some(provider) = args.provider {
        app_config.api.provider = Some(config::normalized_tts_provider(Some(&provider)));
    }
    if let Some(endpoint_name) = args.sagemaker_endpoint_name {
        app_config.sagemaker.endpoint_name = Some(endpoint_name);
    }
    if let Some(region) = args.aws_region {
        app_config.sagemaker.region = Some(region);
    }
    if let Some(size) = args.log_max_size_bytes {
        app_config.logging.max_size_bytes = size.max(1);
    }
    if let Some(count) = args.log_max_files {
        app_config.logging.max_files = count.max(1);
    }
    logging::init(&app_config.logging);
    let endpoint = args
        .endpoint
        .or_else(|| app_config.api.endpoint.clone())
        .unwrap_or_else(|| "https://api.deepgram.com/v1/speak".to_string());
    // Resolve audio format: CLI > env > config > default (mp3)
    let format_str = args
        .audio_format
        .or_else(|| app_config.audio.format.clone())
        .unwrap_or_else(|| "mp3".to_string());
    let format_index = AUDIO_FORMATS
        .iter()
        .position(|f| f.encoding == format_str)
        .unwrap_or(DEFAULT_FORMAT_INDEX);

    // Resolve sample rate: CLI > env > config > format default
    let sample_rate = args
        .sample_rate
        .or(app_config.audio.sample_rate)
        .unwrap_or_else(|| AUDIO_FORMATS[format_index].default_sample_rate);

    let mut app = App::new(endpoint, format_index, sample_rate, app_config);
    app.persist_existing_logs();
    let res = run_app(&mut terminal, &mut app, request_tags).await;

    // Restore terminal
    if kbd_enhancement {
        let _ = execute!(terminal.backend_mut(), PopKeyboardEnhancementFlags);
    }
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("{:?}", err)
    }

    Ok(())
}

/// Kick off a TTS fetch+play in a background task.
/// Stops any in-progress playback first. When `force_regenerate` is true the
/// cache is bypassed and a fresh request is made to the selected Deepgram TTS provider.
fn kick_off_tts(
    app: &mut App,
    text: String,
    voice_id: String,
    backend: tts::TtsBackend,
    request_tags: Vec<String>,
    force_regenerate: bool,
) {
    // Stop any existing audio and signal an in-flight WebSocket stream to close.
    app.stop_audio_playback();

    app.start_loading(text.clone());
    app.set_status_message(format!("Generating audio: {}", text));

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    app.tts_receiver = Some(rx);

    if app.websocket_streaming_enabled {
        let (api_key, endpoint, voice_id) = match backend {
            tts::TtsBackend::Deepgram {
                api_key, endpoint, ..
            } => (api_key, endpoint, voice_id),
            tts::TtsBackend::SageMaker { .. } => {
                app.handle_tts_error(
                    "WebSocket streaming is available only with the hosted Deepgram provider."
                        .to_string(),
                );
                return;
            }
        };

        let sample_rate = tts_streaming::streaming_sample_rate(app.sample_rate);
        if sample_rate != app.sample_rate {
            app.add_log(format!(
                "WebSocket streaming uses Linear16 at {} Hz (the selected format's rate is unsupported)",
                sample_rate
            ));
        }
        if app.config.audio.normalize_volume {
            app.add_log("WebSocket streaming ignores volume normalization.".to_string());
        }
        let speed = app.playback_speed;
        let strategy = app.streaming_chunking_strategy;
        let protocol = if voice_id.starts_with("flux-") {
            tts_streaming::StreamingProtocol::Flux
        } else {
            tts_streaming::StreamingProtocol::Aura
        };
        let cancellation = CancellationToken::new();
        app.start_streaming_playback(cancellation.clone());
        let status = match protocol {
            tts_streaming::StreamingProtocol::Aura => {
                format!("Streaming {} chunks: {}", strategy.label(), text)
            }
            tts_streaming::StreamingProtocol::Flux => {
                format!("Streaming Flux turn: {}", text)
            }
        };
        app.set_status_message(status);

        tokio::spawn(async move {
            let result = tts_streaming::stream_speech(
                tts_streaming::StreamingRequest {
                    api_key: api_key.as_deref(),
                    endpoint: &endpoint,
                    voice_id: &voice_id,
                    protocol,
                    speed,
                    sample_rate,
                    chunking_strategy: strategy,
                    text: &text,
                    tags: &request_tags,
                },
                cancellation,
                tx.clone(),
            )
            .await;
            if let Err(error) = result {
                let _ = tx.send(app::TtsResult::Error(format!(
                    "WebSocket streaming failed: {error:#}"
                )));
            }
        });
        return;
    }

    let speed = app.playback_speed;
    let sample_rate = app.sample_rate;
    let encoding = app.current_audio_format().encoding.to_string();
    let normalize_volume = app.config.audio.normalize_volume;
    let extension = app.current_audio_format().extension.to_string();
    let cache_dir = app.audio_cache_dir.clone();

    tokio::spawn(async move {
        let result = match tts::fetch_audio_for_playback(
            &backend,
            &text,
            &voice_id,
            speed,
            sample_rate,
            &encoding,
            normalize_volume,
            &extension,
            &cache_dir,
            &request_tags,
            force_regenerate,
        )
        .await
        {
            Ok((msg, audio_data, is_cached, request_id)) => app::TtsResult::Success {
                message: msg,
                audio_data,
                is_cached,
                request_id,
            },
            Err(e) => {
                let mut error_msg = format!("Error fetching audio: {:#}", e);
                let mut source = e.source();
                while let Some(err) = source {
                    error_msg.push_str(&format!("\n  Caused by: {}", err));
                    source = err.source();
                }
                app::TtsResult::Error(error_msg)
            }
        };
        let _ = tx.send(result);
    });
}

fn build_tts_backend(app: &App) -> Result<tts::TtsBackend> {
    match app.tts_provider().as_str() {
        "deepgram" => Ok(tts::TtsBackend::Deepgram {
            api_key: app.api_key_override.clone(),
            endpoint: app.deepgram_endpoint.clone(),
        }),
        "sagemaker" => {
            let endpoint_name = app
                .config
                .sagemaker
                .endpoint_name
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "SageMaker endpoint name is required when provider is sagemaker"
                    )
                })?
                .to_string();
            let region = app
                .config
                .sagemaker
                .region
                .clone()
                .unwrap_or_else(|| "us-east-2".to_string());
            Ok(tts::TtsBackend::SageMaker {
                endpoint_name,
                region,
            })
        }
        other => Err(anyhow::anyhow!(
            "Unsupported TTS provider '{}'. Use 'deepgram' or 'sagemaker'.",
            other
        )),
    }
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    request_tags: Vec<String>,
) -> Result<()> {
    // Track mouse capture state so we can toggle it when popups open/close.
    // Mouse capture is enabled at startup (see main()), so start as true.
    let mut mouse_captured = true;

    loop {
        // Update spinner animation
        app.update_spinner();

        // Drain TTS events from background tasks. Streaming binary chunks are
        // appended to the active sink immediately, while HTTP results still
        // arrive as one complete buffer.
        for result in app.take_tts_results() {
            match result {
                app::TtsResult::Success {
                    message,
                    audio_data,
                    is_cached,
                    request_id,
                } => {
                    app.record_cached_tts_result(message, is_cached);
                    if let Some(request_id) = request_id {
                        app.add_log(format!("Deepgram request ID: {request_id}"));
                    }
                    let encoding = app.current_audio_format().encoding;
                    let sample_rate = app.sample_rate;
                    match app.audio_output.as_ref().map(|output| output.mixer.clone()) {
                        Some(mixer) => match tts::play_audio_data_sync(
                            &audio_data,
                            encoding,
                            sample_rate,
                            &mixer,
                        ) {
                            Ok((sink, duration_ms)) => {
                                app.audio_sink = Some(sink);
                                app.audio_duration_ms = duration_ms;
                                app.playback_start_time = std::time::Instant::now();
                            }
                            Err(error) => {
                                app.handle_tts_error(format!("Error starting playback: {error}"))
                            }
                        },
                        None => app.handle_tts_error(
                            "Error starting playback: no audio output device available".to_string(),
                        ),
                    }
                }
                app::TtsResult::StreamingStarted {
                    chunk_count,
                    sample_rate,
                } => {
                    app.add_log(format!(
                        "Deepgram WebSocket connected: {} chunks, Linear16 at {} Hz",
                        chunk_count, sample_rate
                    ));
                }
                app::TtsResult::StreamingAudio(audio_data) => {
                    let sample_rate = tts_streaming::streaming_sample_rate(app.sample_rate);
                    match app.audio_output.as_ref().map(|output| output.mixer.clone()) {
                        Some(mixer) => {
                            let playback = match app.audio_sink.as_ref() {
                                Some(sink) => tts::append_linear16_streaming_audio(
                                    sink,
                                    &audio_data,
                                    sample_rate,
                                )
                                .map(|duration| (sink.clone(), duration)),
                                None => tts::create_linear16_streaming_sink(
                                    &audio_data,
                                    sample_rate,
                                    &mixer,
                                ),
                            };
                            match playback {
                                Ok((sink, duration_ms)) => {
                                    if app.audio_sink.is_none() {
                                        app.audio_sink = Some(sink);
                                        app.playback_start_time = std::time::Instant::now();
                                    }
                                    app.audio_duration_ms =
                                        app.audio_duration_ms.saturating_add(duration_ms);
                                }
                                Err(error) => app.handle_tts_error(format!(
                                    "Error playing streaming audio: {error}"
                                )),
                            }
                        }
                        None => app.handle_tts_error(
                            "Error starting playback: no audio output device available".to_string(),
                        ),
                    }
                }
                app::TtsResult::StreamingChunk { index, total, text } => {
                    app.add_log(format!(
                        "Deepgram WebSocket Speak chunk {index}/{total}: {text}"
                    ));
                }
                app::TtsResult::StreamingRequestId(request_id) => {
                    app.add_log(format!("Deepgram request ID: {request_id}"));
                }
                app::TtsResult::StreamingWarning(warning) => {
                    app.add_log_with_level(
                        app::LogLevel::Warning,
                        format!("Deepgram streaming warning: {warning}"),
                    );
                }
                app::TtsResult::StreamingFlushed => app.mark_streaming_generation_complete(),
                app::TtsResult::Error(error) => app.handle_tts_error(error),
            }
        }

        // Check if audio playback is complete
        app.check_audio_playback();

        // Auto-advance the playback queue when previous track finishes
        if app.needs_queue_advance {
            app.needs_queue_advance = false;
            if let Some((text, voice_id)) = app.playback_queue.pop_front() {
                match build_tts_backend(app) {
                    Ok(backend) => {
                        let remaining = app.playback_queue.len();
                        if remaining > 0 {
                            app.add_log(format!(
                                "Queue: playing next item ({} remaining after this)",
                                remaining
                            ));
                        }
                        kick_off_tts(app, text, voice_id, backend, request_tags.clone(), false);
                    }
                    Err(e) => {
                        app.add_log(format!("Queue advance failed: {}", e));
                        app.playback_queue.clear();
                    }
                }
            }
        }

        terminal.draw(|f| ui::render_ui(f, app))?;

        // Disable mouse capture when a popup is open.  With mouse capture
        // active the terminal sends mouse events as \x1b-prefixed escape
        // sequences; crossterm's ESC disambiguation can misidentify a plain
        // ESC keypress as the start of one of those sequences, swallowing it.
        // Mouse interactions are only used on the Main screen, so there is no
        // functional cost to toggling capture here.
        let popup_open = app.current_screen != CurrentScreen::Main;
        if popup_open && mouse_captured {
            let _ = execute!(terminal.backend_mut(), DisableMouseCapture);
            mouse_captured = false;
        } else if !popup_open && !mouse_captured {
            let _ = execute!(terminal.backend_mut(), EnableMouseCapture);
            mouse_captured = true;
        }

        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                CrosstermEvent::Paste(content) => match app.current_screen {
                    CurrentScreen::Editing => {
                        app.input_buffer.push_str(&content);
                    }
                    CurrentScreen::ApiKeyInput => {
                        app.api_key_input_buffer.push_str(&content);
                    }
                    _ => {}
                },
                CrosstermEvent::Key(key) => {
                    // Handle ESC before key-kind filtering so popup dismissal
                    // works regardless of whether the terminal reports ESC as
                    // Press, Release, or Repeat (behaviour varies across terminals
                    // and crossterm versions when mouse capture is active).
                    if key.code == KeyCode::Esc {
                        match app.current_screen {
                            CurrentScreen::Main => {
                                if app.is_loading {
                                    app.stop_audio_playback();
                                } else if app.focused_panel == Panel::VoiceMenu
                                    && !app.voice_filter.is_empty()
                                {
                                    app.clear_voice_filter();
                                } else if app.focused_panel == Panel::TextList
                                    && !app.text_filter.is_empty()
                                {
                                    app.clear_text_filter();
                                }
                            }
                            CurrentScreen::Editing => app.exit_input_mode(),
                            CurrentScreen::Help => app.exit_help_screen(),
                            CurrentScreen::ApiKeyInput => app.exit_api_key_mode(),
                            CurrentScreen::VoiceFilter => app.cancel_voice_filter(),
                            CurrentScreen::TextFilter => app.cancel_text_filter(),
                            CurrentScreen::ThemeSelect => app.cancel_theme_mode(),
                            CurrentScreen::SampleRateSelect => app.cancel_sample_rate_mode(),
                            CurrentScreen::AudioFormatSelect => app.cancel_audio_format_mode(),
                            CurrentScreen::CommandPalette => app.exit_command_palette(),
                        }
                        continue;
                    }

                    // Skip Release events for all other keys to avoid double-firing
                    // on terminals that send both Press and Release events.
                    if key.kind == event::KeyEventKind::Release {
                        continue;
                    }

                    match app.current_screen {
                        CurrentScreen::Main => match key.code {
                            KeyCode::Char('q')
                                if !key.modifiers.contains(KeyModifiers::CONTROL) =>
                            {
                                if app.focused_panel == Panel::TextList {
                                    return Ok(());
                                }
                            }
                            KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                return Ok(());
                            }
                            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                app.enter_command_palette();
                            }
                            KeyCode::Char('?') => {
                                app.show_help_screen();
                            }
                            KeyCode::Char('k') => {
                                app.enter_api_key_mode();
                            }
                            KeyCode::Char('o') => {
                                app.open_audio_cache_in_finder();
                            }
                            KeyCode::Char('n') => {
                                if app.focused_panel == Panel::TextList {
                                    app.enter_input_mode();
                                }
                            }
                            KeyCode::Char('e') => {
                                if app.focused_panel == Panel::TextList {
                                    app.enter_edit_mode();
                                }
                            }
                            KeyCode::Char('d') => {
                                if app.focused_panel == Panel::TextList {
                                    app.delete_selected_text();
                                }
                            }
                            KeyCode::Char(' ') => {
                                if app.focused_panel == Panel::TextList {
                                    app.enqueue_current();
                                }
                            }
                            KeyCode::Char('*') => {
                                if app.focused_panel == Panel::VoiceMenu {
                                    app.toggle_favorite_voice();
                                }
                            }
                            KeyCode::Char('v') | KeyCode::Char('V') => {
                                app.toggle_volume_normalization();
                            }
                            KeyCode::Char('w') | KeyCode::Char('W') => {
                                app.toggle_websocket_streaming();
                            }
                            KeyCode::Char('c') | KeyCode::Char('C') => {
                                app.cycle_streaming_chunking_strategy();
                            }
                            KeyCode::Down => {
                                if key.modifiers.contains(KeyModifiers::CONTROL)
                                    && app.focused_panel == Panel::TextList
                                {
                                    app.move_text_down();
                                } else {
                                    app.scroll_text_list(1);
                                }
                            }
                            KeyCode::Up => {
                                if key.modifiers.contains(KeyModifiers::CONTROL)
                                    && app.focused_panel == Panel::TextList
                                {
                                    app.move_text_up();
                                } else {
                                    app.scroll_text_list(-1);
                                }
                            }
                            KeyCode::Right | KeyCode::Tab => app.focus_next_panel(),
                            KeyCode::Left => app.focus_prev_panel(),
                            KeyCode::Backspace => {
                                if app.focused_panel == Panel::VoiceMenu
                                    && !app.voice_filter.is_empty()
                                {
                                    app.voice_filter.pop();
                                    app.voice_menu_state.select(Some(0));
                                } else if app.focused_panel == Panel::TextList
                                    && !app.text_filter.is_empty()
                                {
                                    app.text_filter.pop();
                                    app.text_table_state.select(Some(0));
                                }
                            }
                            KeyCode::Char('+') | KeyCode::Char('=') => {
                                app.increase_speed();
                            }
                            KeyCode::Char('-') => {
                                app.decrease_speed();
                            }
                            KeyCode::Char('0') => {
                                app.reset_speed();
                            }
                            KeyCode::Enter => {
                                let force = key.modifiers.contains(KeyModifiers::CONTROL);
                                if let Some(selected_text) = app.get_selected_text() {
                                    if let Some(selected_voice) = app.get_selected_voice() {
                                        let voice_id = selected_voice.id.clone();
                                        match build_tts_backend(app) {
                                            Ok(backend) => {
                                                kick_off_tts(
                                                    app,
                                                    selected_text,
                                                    voice_id,
                                                    backend,
                                                    request_tags.clone(),
                                                    force,
                                                );
                                            }
                                            Err(e) => {
                                                app.add_log(format!("Error: {}", e));
                                                app.set_status_message(
                                                    "TTS configuration missing".to_string(),
                                                );
                                            }
                                        }
                                    } else {
                                        app.set_status_message("No voice selected".to_string());
                                    }
                                }
                            }
                            KeyCode::Char('/') => match app.focused_panel {
                                Panel::VoiceMenu => app.enter_voice_filter_mode(),
                                Panel::TextList => app.enter_text_filter_mode(),
                            },
                            KeyCode::Char('t') => {
                                app.enter_theme_select_mode();
                            }
                            KeyCode::Char('f') => {
                                app.enter_audio_format_mode();
                            }
                            KeyCode::Char('s') => {
                                app.enter_sample_rate_mode();
                            }
                            _ => {}
                        },
                        CurrentScreen::Editing => match key.code {
                            KeyCode::Enter => {
                                app.save_input_as_text();
                            }
                            // Backspace on an empty buffer cancels (same as Esc)
                            KeyCode::Backspace if app.input_buffer.is_empty() => {
                                app.exit_input_mode();
                            }
                            KeyCode::Backspace => {
                                app.input_buffer.pop();
                            }
                            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                app.delete_previous_word();
                            }
                            KeyCode::Char('v') | KeyCode::Char('V')
                                if key.modifiers.contains(KeyModifiers::CONTROL)
                                    || key.modifiers.contains(KeyModifiers::SUPER) =>
                            {
                                app.paste_from_clipboard();
                            }
                            KeyCode::Char(c) => {
                                app.input_buffer.push(c);
                            }
                            _ => {}
                        },
                        CurrentScreen::Help => match key.code {
                            KeyCode::Char('q') => {
                                app.exit_help_screen();
                            }
                            KeyCode::Up => {
                                app.scroll_help(-1, 70); // updated line count
                            }
                            KeyCode::Down => {
                                app.scroll_help(1, 70);
                            }
                            _ => {}
                        },
                        CurrentScreen::ApiKeyInput => match key.code {
                            KeyCode::Enter => {
                                app.save_api_key();
                            }
                            // Backspace on an empty buffer cancels (same as Esc)
                            KeyCode::Backspace if app.api_key_input_buffer.is_empty() => {
                                app.exit_api_key_mode();
                            }
                            KeyCode::Backspace => {
                                app.api_key_input_buffer.pop();
                            }
                            KeyCode::Char('v') | KeyCode::Char('V')
                                if key.modifiers.contains(KeyModifiers::CONTROL)
                                    || key.modifiers.contains(KeyModifiers::SUPER) =>
                            {
                                app.paste_from_clipboard_to_api_key();
                            }
                            KeyCode::Char(c) => {
                                app.api_key_input_buffer.push(c);
                            }
                            _ => {}
                        },
                        CurrentScreen::VoiceFilter => match key.code {
                            KeyCode::Enter => {
                                app.apply_voice_filter();
                            }
                            // Backspace on an empty buffer cancels filter mode
                            KeyCode::Backspace if app.voice_filter_buffer.is_empty() => {
                                app.cancel_voice_filter();
                            }
                            KeyCode::Backspace => {
                                app.voice_filter_buffer.pop();
                            }
                            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                app.clear_voice_filter_buffer();
                            }
                            KeyCode::Char(c) => {
                                app.voice_filter_buffer.push(c);
                            }
                            _ => {}
                        },
                        CurrentScreen::TextFilter => match key.code {
                            KeyCode::Enter => {
                                app.apply_text_filter();
                            }
                            // Backspace on an empty buffer cancels filter mode
                            KeyCode::Backspace if app.text_filter_buffer.is_empty() => {
                                app.cancel_text_filter();
                            }
                            KeyCode::Backspace => {
                                app.text_filter_buffer.pop();
                            }
                            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                app.clear_text_filter_buffer();
                            }
                            KeyCode::Char(c) => {
                                app.text_filter_buffer.push(c);
                            }
                            _ => {}
                        },
                        CurrentScreen::SampleRateSelect => match key.code {
                            KeyCode::Enter => {
                                app.apply_sample_rate();
                            }
                            KeyCode::Char('q') => {
                                app.cancel_sample_rate_mode();
                            }
                            KeyCode::Up => {
                                app.scroll_sample_rate_menu(-1);
                            }
                            KeyCode::Down => {
                                app.scroll_sample_rate_menu(1);
                            }
                            _ => {}
                        },
                        CurrentScreen::AudioFormatSelect => match key.code {
                            KeyCode::Enter => {
                                app.apply_audio_format();
                            }
                            KeyCode::Char('q') => {
                                app.cancel_audio_format_mode();
                            }
                            KeyCode::Up => {
                                app.scroll_audio_format_menu(-1);
                            }
                            KeyCode::Down => {
                                app.scroll_audio_format_menu(1);
                            }
                            _ => {}
                        },
                        CurrentScreen::ThemeSelect => match key.code {
                            KeyCode::Enter => {
                                app.apply_theme();
                            }
                            KeyCode::Char('q') => {
                                app.cancel_theme_mode();
                            }
                            KeyCode::Up => {
                                app.scroll_theme_menu(-1);
                            }
                            KeyCode::Down => {
                                app.scroll_theme_menu(1);
                            }
                            _ => {}
                        },
                        CurrentScreen::CommandPalette => match key.code {
                            KeyCode::Enter => {
                                if let Some(action) = app.execute_command_palette() {
                                    match action {
                                        CommandAction::Quit => return Ok(()),
                                        CommandAction::PlaySelected => {
                                            if let Some(selected_text) = app.get_selected_text() {
                                                if let Some(selected_voice) =
                                                    app.get_selected_voice()
                                                {
                                                    let voice_id = selected_voice.id.clone();
                                                    match build_tts_backend(app) {
                                                        Ok(backend) => kick_off_tts(
                                                            app,
                                                            selected_text,
                                                            voice_id,
                                                            backend,
                                                            request_tags.clone(),
                                                            false,
                                                        ),
                                                        Err(e) => {
                                                            app.add_log(format!("Error: {}", e));
                                                            app.set_status_message(
                                                                "TTS configuration missing"
                                                                    .to_string(),
                                                            );
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        _ => {} // handled inside execute_command_palette
                                    }
                                }
                            }
                            KeyCode::Up => app.scroll_command_palette(-1),
                            KeyCode::Down => app.scroll_command_palette(1),
                            KeyCode::Backspace if app.command_palette_buffer.is_empty() => {
                                app.exit_command_palette();
                            }
                            KeyCode::Backspace => {
                                app.command_palette_buffer.pop();
                                // Keep selection in bounds after filter narrows
                                let len = app.get_filtered_commands().len();
                                if let Some(sel) = app.command_palette_state.selected() {
                                    if sel >= len && len > 0 {
                                        app.command_palette_state.select(Some(len - 1));
                                    }
                                }
                            }
                            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                app.command_palette_buffer.clear();
                                app.command_palette_state.select(Some(0));
                            }
                            KeyCode::Char(c) => {
                                app.command_palette_buffer.push(c);
                                // Reset selection to top when filter changes
                                app.command_palette_state.select(Some(0));
                            }
                            _ => {}
                        },
                    }
                }
                CrosstermEvent::Mouse(mouse) => {
                    if app.current_screen == CurrentScreen::Main {
                        let over_logs = {
                            let b = app.log_panel_bounds;
                            mouse.column >= b.x
                                && mouse.column < b.x + b.width
                                && mouse.row >= b.y
                                && mouse.row < b.y + b.height
                        };
                        match mouse.kind {
                            MouseEventKind::ScrollUp => {
                                if over_logs {
                                    app.scroll_logs(1);
                                } else {
                                    app.scroll_text_list(-1);
                                }
                            }
                            MouseEventKind::ScrollDown => {
                                if over_logs {
                                    app.scroll_logs(-1);
                                } else {
                                    app.scroll_text_list(1);
                                }
                            }
                            MouseEventKind::Down(_) => {
                                app.handle_mouse_click(mouse.column, mouse.row);
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
    }
}
