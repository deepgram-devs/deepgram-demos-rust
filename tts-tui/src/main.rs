mod app;
mod config;
mod ui;
mod tts;
mod persistence;

use std::{io, time::Duration};
use anyhow::Result;
use crossterm::{
    event::{self, Event as CrosstermEvent, KeyCode, KeyModifiers, MouseEventKind, EnableMouseCapture, DisableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use app::{App, CurrentScreen, AUDIO_FORMATS, DEFAULT_FORMAT_INDEX};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "tts-tui")]
#[command(about = "A Deepgram TTS terminal user interface")]
#[command(version)]
struct Args {
    /// Custom Deepgram API endpoint URL for TTS (overrides config file and env var)
    #[arg(long, env = "DEEPGRAM_TTS_ENDPOINT")]
    endpoint_override: Option<String>,

    /// TTS audio encoding format (overrides config file and env var)
    /// Valid values: mp3, linear16, mulaw, alaw, opus, flac, aac
    #[arg(long, env = "DEEPGRAM_AUDIO_FORMAT")]
    audio_format: Option<String>,

    /// TTS output sample rate in Hz (overrides config file and env var)
    /// Valid values depend on the chosen format
    #[arg(long, env = "DEEPGRAM_SAMPLE_RATE")]
    sample_rate: Option<u32>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Load config, then create app
    let app_config = config::load();
    let endpoint = args.endpoint_override
        .or_else(|| app_config.api.endpoint.clone())
        .unwrap_or_else(|| "https://api.deepgram.com/v1/speak".to_string());
    // Resolve audio format: CLI > env > config > default (mp3)
    let format_str = args.audio_format
        .or_else(|| app_config.audio.format.clone())
        .unwrap_or_else(|| "mp3".to_string());
    let format_index = AUDIO_FORMATS.iter().position(|f| f.encoding == format_str)
        .unwrap_or(DEFAULT_FORMAT_INDEX);

    // Resolve sample rate: CLI > env > config > format default
    let sample_rate = args.sample_rate
        .or(app_config.audio.sample_rate)
        .unwrap_or_else(|| AUDIO_FORMATS[format_index].default_sample_rate);

    let mut app = App::new(endpoint, format_index, sample_rate, app_config);
    let res = run_app(&mut terminal, &mut app).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("{:?}", err)
    }

    Ok(())
}

async fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    loop {
        // Update spinner animation
        app.update_spinner();

        // Check for TTS result from background task
        if let Some(audio_data) = app.check_tts_result() {
            // Play audio on main thread (not Send-safe, must stay here)
            let encoding = app.current_audio_format().encoding;
            let sample_rate = app.sample_rate;
            match tts::play_audio_data_sync(&audio_data, encoding, sample_rate) {
                Ok((sink, stream, duration_ms)) => {
                    app.audio_sink = Some(sink);
                    app.audio_stream = Some(stream);
                    app.audio_duration_ms = duration_ms;
                    app.playback_start_time = std::time::Instant::now();
                }
                Err(e) => {
                    app.stop_loading();
                    app.add_log(format!("Error starting playback: {}", e));
                }
            }
        }

        // Check if audio playback is complete
        app.check_audio_playback();

        terminal.draw(|f| ui::render_ui(f, app))?;

        if event::poll(Duration::from_millis(250))? {
            match event::read()? {
                CrosstermEvent::Paste(content) => {
                    match app.current_screen {
                        CurrentScreen::Editing => {
                            app.input_buffer.push_str(&content);
                        }
                        CurrentScreen::ApiKeyInput => {
                            app.api_key_input_buffer.push_str(&content);
                        }
                        _ => {}
                    }
                }
                CrosstermEvent::Key(key) => {
                if key.kind == event::KeyEventKind::Release {
                    // Skip key-release events; only act on Press (and Repeat)
                    continue;
                }

                // Global Esc: close any open popup and return to Main.
                // Handled here (before the per-screen match) so that crossterm's
                // escape-sequence disambiguation never swallows or misroutes the key.
                if key.code == KeyCode::Esc && app.current_screen != CurrentScreen::Main {
                    app.close_current_popup();
                    continue;
                }

                match app.current_screen {
                    CurrentScreen::Main => match key.code {
                        KeyCode::Char('q') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                            if app.focused_panel == app::Panel::TextList {
                                return Ok(());
                            }
                        }
                        KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            return Ok(());
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
                            if app.focused_panel == app::Panel::TextList {
                                app.enter_input_mode();
                            }
                        }
                        KeyCode::Char('d') => {
                            if app.focused_panel == app::Panel::TextList {
                                app.delete_selected_text();
                            }
                        }
                        KeyCode::Down => app.scroll_text_list(1),
                        KeyCode::Up => app.scroll_text_list(-1),
                        KeyCode::Right | KeyCode::Tab => app.focus_next_panel(),
                        KeyCode::Left => app.focus_prev_panel(),
                        KeyCode::Esc => {
                            if app.is_loading {
                                app.stop_audio_playback();
                            } else if app.focused_panel == app::Panel::VoiceMenu && !app.voice_filter.is_empty() {
                                app.clear_voice_filter();
                            }
                        }
                        KeyCode::Backspace => {
                            if app.focused_panel == app::Panel::VoiceMenu && !app.voice_filter.is_empty() {
                                app.voice_filter.pop();
                                app.voice_menu_state.select(Some(0));
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
                            if let Some(selected_text) = app.get_selected_text() {
                                if let Some(selected_voice) = app.get_selected_voice() {
                                    let voice_id = selected_voice.id.clone();
                                    let api_key_result = if let Some(ref key) = app.api_key_override {
                                        Ok(key.clone())
                                    } else {
                                        tts::get_deepgram_api_key()
                                    };
                                    match api_key_result {
                                        Ok(dg_api_key) => {
                                            // Stop any existing audio playback before starting new clip
                                            if app.audio_sink.is_some() {
                                                if let Some(sink) = app.audio_sink.take() {
                                                    sink.stop();
                                                }
                                                app.audio_stream = None;
                                            }

                                            // Start loading state
                                            app.start_loading(selected_text.clone());
                                            app.set_status_message(format!("Generating audio: {}", selected_text));

                                            // Create channel for background task
                                            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
                                            app.tts_receiver = Some(rx);

                                            // Clone data for background task
                                            let api_key = dg_api_key.clone();
                                            let text = selected_text.clone();
                                            let speed = app.playback_speed;
                                            let sample_rate = app.sample_rate;
                                            let encoding = app.current_audio_format().encoding.to_string();
                                            let extension = app.current_audio_format().extension.to_string();
                                            let cache_dir = app.audio_cache_dir.clone();
                                            let endpoint = app.deepgram_endpoint.clone();

                                            // Spawn background task for TTS API call (network only)
                                            tokio::spawn(async move {
                                                let result = match tts::fetch_audio_for_playback(&api_key, &text, &voice_id, speed, sample_rate, &encoding, &extension, &cache_dir, &endpoint).await {
                                                    Ok((msg, audio_data, is_cached)) => {
                                                        app::TtsResult::Success { message: msg, audio_data, is_cached }
                                                    }
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
                                        Err(e) => {
                                            app.add_log(format!("Error: {}", e));
                                            app.set_status_message("API Key missing".to_string());
                                        }
                                    }
                                } else {
                                    app.set_status_message("No voice selected".to_string());
                                }
                            }
                        }
                        KeyCode::Char('/') => {
                            app.enter_voice_filter_mode();
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
                        KeyCode::Backspace => {
                            app.input_buffer.pop();
                        }
                        KeyCode::Char('v') | KeyCode::Char('V') if key.modifiers.contains(KeyModifiers::CONTROL) || key.modifiers.contains(KeyModifiers::SUPER) => {
                            app.paste_from_clipboard();
                        }
                        KeyCode::Char(c) => {
                            app.input_buffer.push(c);
                        }
                        _ => {}
                    },
                    CurrentScreen::Help => match key.code {
                        KeyCode::Up => {
                            app.scroll_help(-1, 42); // 42 help lines
                        }
                        KeyCode::Down => {
                            app.scroll_help(1, 42);
                        }
                        _ => {}
                    },
                    CurrentScreen::ApiKeyInput => match key.code {
                        KeyCode::Enter => {
                            app.save_api_key();
                        }
                        KeyCode::Backspace => {
                            app.api_key_input_buffer.pop();
                        }
                        KeyCode::Char('v') | KeyCode::Char('V') if key.modifiers.contains(KeyModifiers::CONTROL) || key.modifiers.contains(KeyModifiers::SUPER) => {
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
                    CurrentScreen::SampleRateSelect => match key.code {
                        KeyCode::Enter => {
                            app.apply_sample_rate();
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
                        KeyCode::Up => {
                            app.scroll_audio_format_menu(-1);
                        }
                        KeyCode::Down => {
                            app.scroll_audio_format_menu(1);
                        }
                        _ => {}
                    },
                }
                }
                CrosstermEvent::Mouse(mouse) => {
                    if app.current_screen == CurrentScreen::Main {
                        let over_logs = {
                            let b = app.log_panel_bounds;
                            mouse.column >= b.x && mouse.column < b.x + b.width
                                && mouse.row >= b.y && mouse.row < b.y + b.height
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
