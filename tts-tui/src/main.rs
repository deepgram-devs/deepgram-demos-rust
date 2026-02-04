mod app;
mod ui;
mod tts;

use std::{io, time::Duration};
use anyhow::Result;
use crossterm::{
    event::{self, Event as CrosstermEvent, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use app::{App, CurrentScreen};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "tts-tui")]
#[command(about = "A Deepgram TTS terminal user interface")]
#[command(version)]
struct Args {
    /// Custom Deepgram API endpoint URL for TTS
    #[arg(long, env = "DEEPGRAM_TTS_ENDPOINT", default_value = "https://api.deepgram.com/v1/speak")]
    endpoint: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run it
    let mut app = App::new(args.endpoint);
    let res = run_app(&mut terminal, &mut app).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("{:?}", err)
    }

    Ok(())
}

async fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|f| ui::render_ui(f, app))?;

        if event::poll(Duration::from_millis(250))? {
            match event::read()? {
                CrosstermEvent::Paste(content) => {
                    if app.current_screen == CurrentScreen::Editing {
                        app.input_buffer.push_str(&content);
                    }
                }
                CrosstermEvent::Key(key) => {
                if key.kind == event::KeyEventKind::Release {
                    // Skip events that are not KeyEventKind::Press
                    continue;
                }
                match app.current_screen {
                    CurrentScreen::Main => match key.code {
                        KeyCode::Char('q') => {
                            if app.focused_panel == app::Panel::TextList {
                                return Ok(());
                            }
                        }
                        KeyCode::Char('?') => {
                            app.show_help_screen();
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
                            if app.focused_panel == app::Panel::VoiceMenu && !app.voice_filter.is_empty() {
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
                                app.set_status_message(format!("Playing: {}", selected_text));
                                if let Some(selected_voice) = app.get_selected_voice() {
                                    let voice_id = selected_voice.id.clone();
                                    match tts::get_deepgram_api_key() {
                                        Ok(dg_api_key) => {
                                            match tts::play_text_with_deepgram(&dg_api_key, &selected_text, &voice_id, app.playback_speed, &app.audio_cache_dir, &app.deepgram_endpoint).await {
                                                Ok(msg) => {
                                                    app.add_log(msg);
                                                    app.set_status_message(format!("Finished playing: {}", selected_text));
                                                }
                                                Err(e) => {
                                                    // Log detailed error with full chain
                                                    app.add_log(format!("Error playing audio: {:#}", e));
                                                    // Log error source chain if available
                                                    let mut source = e.source();
                                                    while let Some(err) = source {
                                                        app.add_log(format!("  Caused by: {}", err));
                                                        source = err.source();
                                                    }
                                                    app.set_status_message("Error occurred during playback".to_string());
                                                }
                                            }
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
                        KeyCode::Char(c) => {
                            if app.focused_panel == app::Panel::VoiceMenu {
                                app.voice_filter.push(c);
                                app.voice_menu_state.select(Some(0));
                            }
                        }
                        _ => {}
                    },
                    CurrentScreen::Editing => match key.code {
                        KeyCode::Enter => {
                            app.save_input_as_text();
                        }
                        KeyCode::Esc => {
                            app.exit_input_mode();
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
                        KeyCode::Esc => {
                            app.exit_help_screen();
                        }
                        _ => {}
                    },
                }
                }
                _ => {}
            }
        }
    }
}
