mod app;
mod ui;
mod tts;

use std::{io, time::Duration};
use anyhow::Result;
use crossterm::{
    event::{self, Event as CrosstermEvent, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use app::{App, CurrentScreen, CurrentlyEditing};

#[tokio::main]
async fn main() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run it
    let mut app = App::new();
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
            if let CrosstermEvent::Key(key) = event::read()? {
                if key.kind == event::KeyEventKind::Release {
                    // Skip events that are not KeyEventKind::Press
                    continue;
                }
                match app.current_screen {
                    CurrentScreen::Main => match key.code {
                        KeyCode::Char('q') => {
                            return Ok(());
                        }
                        KeyCode::Char('n') => {
                            app.enter_input_mode();
                        }
                        KeyCode::Char('d') => {
                            app.delete_selected_text();
                        }
                        KeyCode::Down => app.scroll_text_list(1),
                        KeyCode::Up => app.scroll_text_list(-1),
                        KeyCode::Right | KeyCode::Tab => app.focus_next_panel(),
                        KeyCode::Left => app.focus_prev_panel(),
                        KeyCode::Enter => {
                            if let Some(selected_text) = app.get_selected_text() {
                                app.set_status_message(format!("Playing: {}", selected_text));
                                let voice_id = app.get_selected_voice_id();
                                match tts::get_deepgram_api_key() {
                                    Ok(dg_api_key) => {
                                        match tts::play_text_with_deepgram(&dg_api_key, &selected_text, &voice_id, &app.audio_cache_dir).await {
                                            Ok(msg) => {
                                                app.add_log(msg);
                                                app.set_status_message(format!("Finished playing: {}", selected_text));
                                            }
                                            Err(e) => {
                                                app.add_log(format!("Error playing audio: {}", e));
                                                app.set_status_message("Error occurred during playback".to_string());
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        app.add_log(format!("Error: {}", e));
                                        app.set_status_message("API Key missing".to_string());
                                    }
                                }
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
                        KeyCode::Char(c) => {
                            app.input_buffer.push(c);
                        }
                        _ => {}
                    },
                }
            }
        }
    }
}
