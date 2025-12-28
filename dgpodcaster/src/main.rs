mod ui;
mod deepgram;
mod gemini;
mod audio;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};
use std::io;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
enum AppState {
    TopicInput,
    SpeakerCountSelection,
    VoiceAssignment,
    GeneratingPodcast,
    Completed,
    Error(String),
}

#[derive(Debug, Clone)]
struct SpeakerVoice {
    speaker_id: usize,
    voice_name: Option<String>,
}

struct App {
    state: AppState,
    topic: String,
    speaker_count: usize,
    speakers: Vec<SpeakerVoice>,
    selected_speaker: usize,
    selected_voice: usize,
    focused_pane: FocusedPane,
    available_voices: Vec<String>,
    generation_progress: String,
    should_quit: bool,
}

#[derive(Debug, Clone, PartialEq)]
enum FocusedPane {
    Left,
    Right,
}

impl App {
    fn new() -> Self {
        Self {
            state: AppState::TopicInput,
            topic: String::new(),
            speaker_count: 2,
            speakers: Vec::new(),
            selected_speaker: 0,
            selected_voice: 0,
            focused_pane: FocusedPane::Left,
            available_voices: get_aura2_voices(),
            generation_progress: String::new(),
            should_quit: false,
        }
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        match self.state {
            AppState::TopicInput => self.handle_topic_input(key),
            AppState::SpeakerCountSelection => self.handle_speaker_count_selection(key),
            AppState::VoiceAssignment => self.handle_voice_assignment(key),
            AppState::GeneratingPodcast | AppState::Completed | AppState::Error(_) => {
                if key.code == KeyCode::Char('q') {
                    self.should_quit = true;
                }
            }
        }
    }

    fn handle_topic_input(&mut self, key: KeyEvent) {
        match (key.code, key.modifiers) {
            (KeyCode::Char('v'), KeyModifiers::SUPER) | (KeyCode::Char('v'), KeyModifiers::CONTROL) => {
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    if let Ok(text) = clipboard.get_text() {
                        self.topic.push_str(&text);
                    }
                }
            }
            (KeyCode::Char(c), KeyModifiers::NONE) | (KeyCode::Char(c), KeyModifiers::SHIFT) => {
                self.topic.push(c);
            }
            (KeyCode::Backspace, _) => {
                self.topic.pop();
            }
            (KeyCode::Enter, _) => {
                if !self.topic.is_empty() {
                    self.state = AppState::SpeakerCountSelection;
                }
            }
            (KeyCode::Esc, _) => {
                self.should_quit = true;
            }
            _ => {}
        }
    }

    fn handle_speaker_count_selection(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('1') => self.speaker_count = 1,
            KeyCode::Char('2') => self.speaker_count = 2,
            KeyCode::Char('3') => self.speaker_count = 3,
            KeyCode::Char('4') => self.speaker_count = 4,
            KeyCode::Enter => {
                self.speakers = (0..self.speaker_count)
                    .map(|id| SpeakerVoice {
                        speaker_id: id + 1,
                        voice_name: None,
                    })
                    .collect();
                self.selected_speaker = 0;
                self.selected_voice = 0;
                self.state = AppState::VoiceAssignment;
            }
            KeyCode::Esc => {
                self.state = AppState::TopicInput;
            }
            _ => {}
        }
    }

    fn handle_voice_assignment(&mut self, key: KeyEvent) {
        match (key.code, key.modifiers) {
            (KeyCode::Char('g'), KeyModifiers::CONTROL) => {
                if self.speakers.iter().all(|s| s.voice_name.is_some()) {
                    self.state = AppState::GeneratingPodcast;
                }
            }
            (KeyCode::Up, _) => {
                match self.focused_pane {
                    FocusedPane::Left => {
                        if self.selected_speaker > 0 {
                            self.selected_speaker -= 1;
                        }
                    }
                    FocusedPane::Right => {
                        if self.selected_voice > 0 {
                            self.selected_voice -= 1;
                        }
                    }
                }
            }
            (KeyCode::Down, _) => {
                match self.focused_pane {
                    FocusedPane::Left => {
                        if self.selected_speaker < self.speakers.len() - 1 {
                            self.selected_speaker += 1;
                        }
                    }
                    FocusedPane::Right => {
                        if self.selected_voice < self.available_voices.len() - 1 {
                            self.selected_voice += 1;
                        }
                    }
                }
            }
            (KeyCode::Left, _) => {
                self.focused_pane = FocusedPane::Left;
            }
            (KeyCode::Right, _) => {
                self.focused_pane = FocusedPane::Right;
            }
            (KeyCode::Enter, _) => {
                if self.focused_pane == FocusedPane::Right {
                    let voice = self.available_voices[self.selected_voice].clone();
                    self.speakers[self.selected_speaker].voice_name = Some(voice);
                }
            }
            (KeyCode::Esc, _) => {
                self.state = AppState::SpeakerCountSelection;
            }
            _ => {}
        }
    }
}

fn get_aura2_voices() -> Vec<String> {
    vec![
        "aura-2-amalthea-en".to_string(),
        "aura-2-andromeda-en".to_string(),
        "aura-2-apollo-en".to_string(),
        "aura-2-arcas-en".to_string(),
        "aura-2-aries-en".to_string(),
        "aura-2-asteria-en".to_string(),
        "aura-2-athena-en".to_string(),
        "aura-2-atlas-en".to_string(),
        "aura-2-aurora-en".to_string(),
        "aura-2-callista-en".to_string(),
        "aura-2-cora-en".to_string(),
        "aura-2-cordelia-en".to_string(),
        "aura-2-delia-en".to_string(),
        "aura-2-draco-en".to_string(),
        "aura-2-electra-en".to_string(),
        "aura-2-harmonia-en".to_string(),
        "aura-2-helena-en".to_string(),
        "aura-2-hera-en".to_string(),
        "aura-2-hermes-en".to_string(),
        "aura-2-hyperion-en".to_string(),
        "aura-2-iris-en".to_string(),
        "aura-2-janus-en".to_string(),
        "aura-2-juno-en".to_string(),
        "aura-2-jupiter-en".to_string(),
        "aura-2-luna-en".to_string(),
        "aura-2-mars-en".to_string(),
        "aura-2-minerva-en".to_string(),
        "aura-2-neptune-en".to_string(),
        "aura-2-odysseus-en".to_string(),
        "aura-2-ophelia-en".to_string(),
        "aura-2-orion-en".to_string(),
        "aura-2-orpheus-en".to_string(),
        "aura-2-pandora-en".to_string(),
        "aura-2-phoebe-en".to_string(),
        "aura-2-pluto-en".to_string(),
        "aura-2-saturn-en".to_string(),
        "aura-2-selene-en".to_string(),
        "aura-2-thalia-en".to_string(),
        "aura-2-theia-en".to_string(),
        "aura-2-vesta-en".to_string(),
        "aura-2-zeus-en".to_string(),
    ]
}

#[tokio::main]
async fn main() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let result = run_app(&mut terminal, &mut app).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(err) = result {
        eprintln!("Error: {:?}", err);
    }

    Ok(())
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<()> {
    loop {
        terminal.draw(|f| ui::render(f, app))?;

        if app.should_quit {
            break;
        }

        if app.state == AppState::GeneratingPodcast {
            let result = generate_podcast(app).await;
            match result {
                Ok(_) => {
                    app.state = AppState::Completed;
                }
                Err(e) => {
                    app.state = AppState::Error(e.to_string());
                }
            }
            continue;
        }

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                app.handle_key_event(key);
            }
        }
    }

    Ok(())
}

async fn generate_podcast(app: &mut App) -> Result<()> {
    app.generation_progress = "Generating podcast script...".to_string();

    let voice_map: HashMap<usize, String> = app.speakers
        .iter()
        .map(|s| (s.speaker_id, s.voice_name.clone().unwrap()))
        .collect();

    let script = gemini::generate_podcast_script(
        &app.topic,
        app.speaker_count,
    ).await?;

    app.generation_progress = "Parsing script...".to_string();
    let utterances = parse_script(&script, app.speaker_count)?;

    app.generation_progress = format!("Generating {} audio clips...", utterances.len());
    let audio_files = deepgram::generate_audio_clips(&utterances, &voice_map).await?;

    app.generation_progress = "Concatenating audio files...".to_string();
    audio::concatenate_audio_files(&audio_files, "podcast_output.wav")?;

    app.generation_progress = "Podcast generated successfully! Saved to podcast_output.wav".to_string();

    Ok(())
}

#[derive(Debug, Clone)]
struct Utterance {
    speaker_id: usize,
    text: String,
}

fn parse_script(script: &str, speaker_count: usize) -> Result<Vec<Utterance>> {
    let mut utterances = Vec::new();

    for line in script.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        for speaker_id in 1..=speaker_count {
            let prefix = format!("Speaker {}:", speaker_id);
            if line.starts_with(&prefix) {
                let text = line[prefix.len()..].trim().to_string();
                if !text.is_empty() {
                    utterances.push(Utterance {
                        speaker_id,
                        text,
                    });
                }
                break;
            }
        }
    }

    if utterances.is_empty() {
        anyhow::bail!("Failed to parse any utterances from the script");
    }

    Ok(utterances)
}
