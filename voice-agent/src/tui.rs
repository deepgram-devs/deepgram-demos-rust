//! Interactive terminal UI for the voice-agent client.
use std::{
    fs::File,
    io::{self, BufWriter, Write},
    sync::{mpsc as std_mpsc, Arc},
    time::Duration,
};

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
        MouseButton, MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures_util::{SinkExt, StreamExt};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Terminal,
};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use super::*;
use crate::config::{self, VoiceAgentConfig as UserConfig};

const TTS_VOICES: &[&str] = &[
    // Aura-2 English
    "aura-2-amalthea-en",
    "aura-2-andromeda-en",
    "aura-2-apollo-en",
    "aura-2-arcas-en",
    "aura-2-aries-en",
    "aura-2-asteria-en",
    "aura-2-athena-en",
    "aura-2-atlas-en",
    "aura-2-aurora-en",
    "aura-2-callista-en",
    "aura-2-cora-en",
    "aura-2-cordelia-en",
    "aura-2-delia-en",
    "aura-2-draco-en",
    "aura-2-electra-en",
    "aura-2-harmonia-en",
    "aura-2-helena-en",
    "aura-2-hera-en",
    "aura-2-hermes-en",
    "aura-2-hyperion-en",
    "aura-2-iris-en",
    "aura-2-janus-en",
    "aura-2-juno-en",
    "aura-2-jupiter-en",
    "aura-2-luna-en",
    "aura-2-mars-en",
    "aura-2-minerva-en",
    "aura-2-neptune-en",
    "aura-2-odysseus-en",
    "aura-2-ophelia-en",
    "aura-2-orion-en",
    "aura-2-orpheus-en",
    "aura-2-pandora-en",
    "aura-2-phoebe-en",
    "aura-2-pluto-en",
    "aura-2-saturn-en",
    "aura-2-selene-en",
    "aura-2-thalia-en",
    "aura-2-theia-en",
    "aura-2-vesta-en",
    "aura-2-zeus-en",
    // Aura-2 Spanish
    "aura-2-sirio-es",
    "aura-2-nestor-es",
    "aura-2-carina-es",
    "aura-2-celeste-es",
    "aura-2-alvaro-es",
    "aura-2-diana-es",
    "aura-2-aquila-es",
    "aura-2-selena-es",
    "aura-2-estrella-es",
    "aura-2-javier-es",
    "aura-2-agustina-es",
    "aura-2-antonia-es",
    "aura-2-gloria-es",
    "aura-2-luciano-es",
    "aura-2-olivia-es",
    "aura-2-silvia-es",
    "aura-2-valerio-es",
    // Aura-2 Dutch
    "aura-2-beatrix-nl",
    "aura-2-daphne-nl",
    "aura-2-cornelia-nl",
    "aura-2-sander-nl",
    "aura-2-hestia-nl",
    "aura-2-lars-nl",
    "aura-2-roman-nl",
    "aura-2-rhea-nl",
    "aura-2-leda-nl",
    // Aura-2 French
    "aura-2-agathe-fr",
    "aura-2-hector-fr",
    // Aura-2 German
    "aura-2-elara-de",
    "aura-2-aurelia-de",
    "aura-2-lara-de",
    "aura-2-julius-de",
    "aura-2-fabian-de",
    "aura-2-kara-de",
    "aura-2-viktoria-de",
    // Aura-2 Italian
    "aura-2-melia-it",
    "aura-2-elio-it",
    "aura-2-flavio-it",
    "aura-2-maia-it",
    "aura-2-cinzia-it",
    "aura-2-cesare-it",
    "aura-2-livia-it",
    "aura-2-dionisio-it",
    "aura-2-demetra-it",
    // Aura-2 Japanese
    "aura-2-uzume-ja",
    "aura-2-ebisu-ja",
    "aura-2-fujin-ja",
    "aura-2-izanami-ja",
    "aura-2-ama-ja",
    // Flux TTS English (all current voices)
    "flux-hannah-en",
    "flux-kit-en",
    "flux-alexis-en",
    "flux-cliff-en",
    "flux-sienna-en",
    "flux-cole-en",
    "flux-brooke-en",
    "flux-colin-en",
    "flux-gemma-en",
    "flux-haley-en",
    "flux-heather-en",
    "flux-miles-en",
    "flux-sean-en",
    "flux-bree-en",
    "flux-brittany-en",
    "flux-bruce-en",
    "flux-conor-en",
    "flux-donovan-en",
    "flux-drew-en",
    "flux-elise-en",
    "flux-jack-en",
    "flux-kai-en",
    "flux-kelsey-en",
    "flux-maeve-en",
    "flux-marcelo-en",
    "flux-marcus-en",
    "flux-meena-en",
    "flux-meghan-en",
    "flux-naveen-en",
    "flux-paige-en",
    "flux-priya-en",
    "flux-rufus-en",
    "flux-sharon-en",
    "flux-tanner-en",
    "flux-wade-en",
    "flux-wes-en",
];
const STT_MODELS: &[(&str, &str, &str)] = &[
    ("Flux English", "flux-general-en", "v2"),
    ("Flux Multilingual", "flux-general-multi", "v2"),
    ("Nova-3", "nova-3", "v1"),
];
const STT_LANGUAGES: &[&str] = &["en", "es", "fr", "de", "pt", "multi"];
const COLOR_TEAL: Color = Color::Rgb(0, 243, 255);
const COLOR_PURPLE: Color = Color::Rgb(189, 0, 255);
const COLOR_BG: Color = Color::Rgb(10, 10, 15);
const COLOR_DARK_GRAY: Color = Color::Rgb(50, 50, 60);

fn update_listen_message(
    model: &str,
    version: &str,
    language: &str,
    preferences: &crate::config::ListenConfig,
) -> serde_json::Value {
    let mut provider = serde_json::Map::new();
    provider.insert("type".into(), serde_json::json!("deepgram"));
    provider.insert("model".into(), serde_json::json!(model));
    provider.insert("version".into(), serde_json::json!(version));

    if version == "v1" {
        provider.insert("language".into(), serde_json::json!(language));
    } else {
        if let Some(threshold) = preferences.eot_threshold {
            provider.insert("eot_threshold".into(), serde_json::json!(threshold));
        }
        if let Some(threshold) = preferences.eager_eot_threshold {
            provider.insert("eager_eot_threshold".into(), serde_json::json!(threshold));
        }
        if !preferences.keyterms.is_empty() {
            provider.insert("keyterms".into(), serde_json::json!(preferences.keyterms));
        }
        if model == "flux-general-multi" {
            provider.insert(
                "language_hints".into(),
                serde_json::json!(preferences.language_hints),
            );
        }
    }

    serde_json::json!({"type": "UpdateListen", "listen": {"provider": provider}})
}

fn update_speak_message(model: &str) -> serde_json::Value {
    let version = deepgram_speak_version(model)
        .expect("TTS selector only contains documented Aura and Flux voice models");
    serde_json::json!({
        "type": "UpdateSpeak",
        "speak": {"provider": {
            "type": "deepgram",
            "version": version,
            "model": model,
        }},
    })
}

#[derive(Clone)]
enum Command {
    Connect,
    Disconnect,
    Prompt,
    HistoricalPrompt,
    Tts,
    Stt,
    SttLanguage,
    UpdateThink,
    ToggleTimestamps,
    ToggleJsonLogging,
    CopyConversation,
    CopyRequestId,
    InjectAgent,
    InjectUser,
    ForceEnd,
    KeepAlive,
    Help,
    Quit,
}

#[derive(Clone, Copy)]
enum InputMode {
    AgentMessage,
    UserMessage,
    ThinkModel,
}

const COMMANDS: &[(&str, &str, Command, Option<&str>)] = &[
    (
        "Connect voice agent",
        "Start microphone, WebSocket, and playback",
        Command::Connect,
        Some("Space"),
    ),
    (
        "Disconnect voice agent",
        "Close the current Voice Agent session",
        Command::Disconnect,
        Some("Space"),
    ),
    (
        "Edit system prompt",
        "Open the full-screen prompt editor",
        Command::Prompt,
        None,
    ),
    (
        "Select historical system prompt",
        "Choose a saved system prompt for this connection",
        Command::HistoricalPrompt,
        None,
    ),
    (
        "Select TTS voice",
        "Switch Aura-2 or Flux and choose a voice",
        Command::Tts,
        None,
    ),
    (
        "Select STT provider",
        "Switch Flux English, Flux Multilingual, or Nova-3",
        Command::Stt,
        None,
    ),
    (
        "Select STT language",
        "Choose the listen language or multi",
        Command::SttLanguage,
        None,
    ),
    (
        "Update think configuration",
        "Send the current LLM model and system prompt",
        Command::UpdateThink,
        None,
    ),
    (
        "Toggle message timestamps",
        "Show or hide timestamps for all user and agent messages",
        Command::ToggleTimestamps,
        None,
    ),
    (
        "Toggle verbose JSON logging",
        "Write all client and server JSON messages to a request-ID log file",
        Command::ToggleJsonLogging,
        None,
    ),
    (
        "Copy conversation",
        "Copy the complete user and agent conversation to the clipboard",
        Command::CopyConversation,
        None,
    ),
    (
        "Copy request ID",
        "Copy the current Deepgram Voice Agent request ID to the clipboard",
        Command::CopyRequestId,
        None,
    ),
    (
        "Inject agent message",
        "Send InjectAgentMessage with a behavior",
        Command::InjectAgent,
        None,
    ),
    (
        "Inject user message",
        "Send InjectUserMessage text to the agent",
        Command::InjectUser,
        None,
    ),
    (
        "Force end turn",
        "Send ForceEndTurn (Flux STT only)",
        Command::ForceEnd,
        None,
    ),
    (
        "Send keep alive",
        "Send AgentKeepAlive to prevent a timeout",
        Command::KeepAlive,
        None,
    ),
    (
        "Show help",
        "Show keyboard and command-palette controls",
        Command::Help,
        Some("?"),
    ),
    ("Quit", "Exit the terminal UI", Command::Quit, Some("q")),
];

struct Ui {
    history: Vec<HistoryEntry>,
    status: String,
    request_id: Option<String>,
    connected: bool,
    palette: bool,
    query: String,
    selected: usize,
    editor: bool,
    prompt: String,
    cursor: usize,
    chooser: Option<usize>,
    chooser_query: String,
    voice_index: usize,
    stt_index: usize,
    stt_language: String,
    history_index: usize,
    input_mode: Option<InputMode>,
    input_buffer: String,
    input_cursor: usize,
    input_behavior: usize,
    timestamps: bool,
    json_logging: bool,
    user_config: UserConfig,
    selected_message: Option<usize>,
}
impl Ui {
    fn new(args: &LaunchOptions, user_config: UserConfig) -> Self {
        let voice_index = user_config
            .last_tts
            .voice
            .as_deref()
            .and_then(|voice| TTS_VOICES.iter().position(|candidate| *candidate == voice))
            .unwrap_or(0);
        let stt_index = user_config
            .listen
            .model
            .as_deref()
            .and_then(|model| {
                STT_MODELS
                    .iter()
                    .position(|(_, candidate, _)| *candidate == model)
            })
            .unwrap_or(2);
        let stt_language = user_config
            .listen
            .language
            .clone()
            .unwrap_or_else(|| "en".into());
        let prompt = user_config
            .historical_prompts
            .first()
            .cloned()
            .or_else(|| args.prompt.clone())
            .unwrap_or_else(|| DEFAULT_SYSTEM_PROMPT.into());
        Self {
            history: vec![HistoryEntry::event(
                "Welcome — press Ctrl+P for the command palette",
            )],
            status: "Disconnected".into(),
            request_id: None,
            connected: false,
            palette: false,
            query: String::new(),
            selected: 0,
            editor: false,
            prompt,
            cursor: 0,
            chooser: None,
            chooser_query: String::new(),
            voice_index,
            stt_index,
            stt_language,
            history_index: 0,
            input_mode: None,
            input_buffer: String::new(),
            input_cursor: 0,
            input_behavior: 0,
            timestamps: user_config.show_timestamps,
            json_logging: user_config.verbose_json_logging,
            user_config,
            selected_message: None,
        }
    }
    fn filtered(&self) -> Vec<usize> {
        let q = self.query.to_lowercase();
        COMMANDS
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                q.is_empty() || c.0.to_lowercase().contains(&q) || c.1.to_lowercase().contains(&q)
            })
            .map(|(i, _)| i)
            .collect()
    }
    fn filtered_tts_indices(&self) -> Vec<usize> {
        let query = self.chooser_query.to_ascii_lowercase();
        TTS_VOICES
            .iter()
            .enumerate()
            .filter(|(_, voice)| query.is_empty() || voice.to_ascii_lowercase().contains(&query))
            .map(|(index, _)| index)
            .collect()
    }
    fn push(&mut self, s: impl Into<String>) {
        self.history.push(HistoryEntry::event(s));
        if self.history.len() > 300 {
            self.history.remove(0);
        }
    }
    fn push_message(&mut self, role: &str, text: String) {
        self.history.push(HistoryEntry::message(role, text));
        if self.history.len() > 300 {
            self.history.remove(0);
        }
    }
    fn push_client(&mut self, s: impl Into<String>) {
        self.history.push(HistoryEntry::client(s));
        if self.history.len() > 300 {
            self.history.remove(0);
        }
    }
    fn push_warning(&mut self, s: impl Into<String>) {
        self.history.push(HistoryEntry::warning(s));
        if self.history.len() > 300 {
            self.history.remove(0);
        }
    }
    fn push_log_path(&mut self, path: String) {
        self.history.push(HistoryEntry::log_path(path));
        if self.history.len() > 300 {
            self.history.remove(0);
        }
    }
    fn save_config(&mut self) {
        if let Err(error) = config::save(&self.user_config) {
            self.push(format!("Configuration save error: {error}"));
        }
    }
    fn remember_prompt(&mut self) {
        if !self.prompt.trim().is_empty() {
            self.user_config
                .historical_prompts
                .retain(|prompt| prompt != &self.prompt);
            self.user_config
                .historical_prompts
                .insert(0, self.prompt.clone());
            self.user_config.historical_prompts.truncate(50);
        }
    }
    fn save_preferences(&mut self, options: &LaunchOptions) {
        self.user_config.last_tts.voice = Some(TTS_VOICES[self.voice_index].into());
        self.user_config.last_tts.model = Some(
            if TTS_VOICES[self.voice_index].starts_with("flux-") {
                "flux"
            } else {
                "aura-2"
            }
            .into(),
        );
        self.user_config.listen.provider = Some("deepgram".into());
        self.user_config.listen.model = Some(STT_MODELS[self.stt_index].1.into());
        self.user_config.listen.version = Some(STT_MODELS[self.stt_index].2.into());
        self.user_config.listen.language = Some(self.stt_language.clone());
        self.user_config.listen.language_hints = options.language_hints.clone();
        self.user_config.listen.keyterms = options.listen_keyterms.clone();
        self.user_config.listen.eot_threshold = options.listen_eot_threshold;
        self.user_config.listen.eager_eot_threshold = options.listen_eager_eot_threshold;
        self.user_config.listen.smart_format = options.listen_smart_format;
        self.save_config();
    }
}

struct HistoryEntry {
    timestamp: String,
    text: String,
    kind: HistoryKind,
    copy_text: Option<String>,
}
#[derive(Clone, Copy)]
enum HistoryKind {
    Event,
    Warning,
    LogPath,
    User,
    Agent,
    Client,
}
impl HistoryEntry {
    fn event(text: impl Into<String>) -> Self {
        Self {
            timestamp: String::new(),
            text: text.into(),
            kind: HistoryKind::Event,
            copy_text: None,
        }
    }
    fn message(role: &str, text: String) -> Self {
        Self {
            timestamp: now_timestamp(),
            text: format!("{role}: {text}"),
            kind: if role == "You" {
                HistoryKind::User
            } else {
                HistoryKind::Agent
            },
            copy_text: None,
        }
    }
    fn client(text: impl Into<String>) -> Self {
        Self {
            timestamp: String::new(),
            text: text.into(),
            kind: HistoryKind::Client,
            copy_text: None,
        }
    }
    fn warning(text: impl Into<String>) -> Self {
        Self {
            timestamp: String::new(),
            text: text.into(),
            kind: HistoryKind::Warning,
            copy_text: None,
        }
    }
    fn log_path(path: String) -> Self {
        Self {
            timestamp: String::new(),
            text: format!("JSON log: {path} (click to copy path)"),
            kind: HistoryKind::LogPath,
            copy_text: Some(path),
        }
    }
}
fn now_timestamp() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        % 86_400;
    format!(
        "{:02}:{:02}:{:02}",
        seconds / 3_600,
        (seconds / 60) % 60,
        seconds % 60
    )
}

struct JsonLogger {
    writer: BufWriter<File>,
    path: std::path::PathBuf,
}
fn open_json_log(request_id: &str) -> Result<JsonLogger, Box<dyn std::error::Error>> {
    let safe_id: String = request_id
        .chars()
        .filter(|character| {
            character.is_ascii_alphanumeric() || *character == '-' || *character == '_'
        })
        .collect();
    let path = std::env::temp_dir().join(format!("{safe_id}.txt"));
    let file = File::create(&path)?;
    Ok(JsonLogger {
        writer: BufWriter::new(file),
        path,
    })
}
fn log_json_message(
    logger: &mut Option<JsonLogger>,
    direction: &str,
    json: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(logger) = logger {
        writeln!(logger.writer, "{direction} {json}")?;
        logger.writer.flush()?;
    }
    Ok(())
}
fn record_json_message(
    history: &mut Vec<(String, String)>,
    logger: &mut Option<JsonLogger>,
    direction: &str,
    json: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    history.push((direction.to_string(), json.to_string()));
    log_json_message(logger, direction, json)
}

enum WorkerCommand {
    Stop,
    Json(String),
    SetJsonLogging(bool),
}

pub async fn run(args: LaunchOptions) -> Result<(), Box<dyn std::error::Error>> {
    let (worker_tx, mut events) = mpsc::unbounded_channel::<String>();
    let mut worker: Option<mpsc::UnboundedSender<WorkerCommand>> = None;
    let saved_config = config::load().unwrap_or_else(|error| {
        eprintln!("Unable to load voice-agent configuration: {error}");
        UserConfig::default()
    });
    let mut ui = Ui::new(&args, saved_config);
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let result = run_loop(&mut ui, &args, &mut worker, &worker_tx, &mut events).await;
    if let Some(tx) = worker {
        let _ = tx.send(WorkerCommand::Stop);
    }
    disable_raw_mode()?;
    execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen)?;
    result
}

async fn run_loop(
    ui: &mut Ui,
    args: &LaunchOptions,
    worker: &mut Option<mpsc::UnboundedSender<WorkerCommand>>,
    event_tx: &mpsc::UnboundedSender<String>,
    events: &mut mpsc::UnboundedReceiver<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    loop {
        while let Ok(line) = events.try_recv() {
            if line == "__CONNECTED__" {
                ui.history.clear();
                ui.connected = true;
                ui.status = "Connected".into();
            } else if let Some(error) = line.strip_prefix("__CONNECTION_ERROR__|") {
                ui.connected = false;
                ui.request_id = None;
                ui.status = "Connection error".into();
                ui.push(format!("Connection error: {error}"));
            } else if let Some(request_id) = line.strip_prefix("__REQUEST_ID__|") {
                ui.request_id = Some(request_id.to_string());
            } else if line == "__DISCONNECTED__" {
                ui.connected = false;
                ui.request_id = None;
                ui.status = "Disconnected".into();
            } else if let Some((role, text)) = line
                .strip_prefix("__MESSAGE__|")
                .and_then(|v| v.split_once('|'))
            {
                ui.push_message(role, text.to_string());
            } else if let Some(path) = line.strip_prefix("__JSON_LOG__|") {
                ui.push_log_path(path.to_string());
            } else if let Some(warning) = line.strip_prefix("__WARNING__|") {
                ui.push_warning(warning.to_string());
            } else {
                ui.push(line);
            }
        }
        terminal.draw(|f| draw(f, ui))?;
        if event::poll(Duration::from_millis(80))? {
            match event::read()? {
                Event::Key(key) => {
                    if handle_key(ui, key, args, worker, event_tx).await? {
                        break;
                    }
                }
                Event::Mouse(mouse) => {
                    handle_mouse(ui, mouse, conversation_area(terminal.size()?.into()))
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn conversation_area(area: ratatui::layout::Rect) -> ratatui::layout::Rect {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(2),
            Constraint::Length(2),
        ])
        .split(area)[1]
}

fn selected_style(style: Style, selected: bool) -> Style {
    if selected {
        style
            .bg(Color::Rgb(45, 25, 55))
            .add_modifier(Modifier::BOLD)
    } else {
        style
    }
}

fn handle_mouse(ui: &mut Ui, mouse: crossterm::event::MouseEvent, area: ratatui::layout::Rect) {
    if ui.palette || ui.editor || ui.chooser.is_some() || ui.input_mode.is_some() {
        return;
    }
    if !matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left))
        || mouse.column < area.x
        || mouse.column >= area.x + area.width
        || mouse.row < area.y + 1
        || mouse.row >= area.y + area.height - 1
    {
        return;
    }
    let clicked_line = (mouse.row - area.y - 1) as usize;
    let mut line = 0;
    let mut selected = None;
    for (index, entry) in ui.history.iter().enumerate() {
        let height = if ui.timestamps && !entry.timestamp.is_empty() {
            2
        } else {
            1
        };
        if clicked_line < line + height {
            selected = Some((index, entry.kind, entry.copy_text.clone()));
            break;
        }
        line += height;
    }
    let Some((index, kind, copy_text)) = selected else {
        return;
    };
    if let Some(path) = copy_text {
        match arboard::Clipboard::new().and_then(|mut clipboard| clipboard.set_text(path)) {
            Ok(()) => ui.push_client("→ Log path copied to clipboard"),
            Err(error) => ui.push(format!("Clipboard error: {error}")),
        }
    } else if matches!(kind, HistoryKind::User | HistoryKind::Agent) {
        ui.selected_message = Some(index);
    }
}

fn draw(f: &mut ratatui::Frame, ui: &Ui) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(2),
            Constraint::Length(2),
        ])
        .split(f.area());
    let title = Paragraph::new(Line::from(vec![
        Span::styled(
            " DEEPGRAM VOICE AGENT ",
            Style::default().fg(COLOR_TEAL).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  {}  ", ui.status),
            Style::default()
                .fg(if ui.connected {
                    Color::Green
                } else {
                    COLOR_PURPLE
                })
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "|  CTRL+P PALETTE  |  ? HELP  |  Q QUIT",
            Style::default().fg(COLOR_DARK_GRAY),
        ),
    ]))
    .style(Style::default().bg(COLOR_BG))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(COLOR_TEAL))
            .style(Style::default().bg(COLOR_BG)),
    );
    f.render_widget(title, chunks[0]);
    let body = ui
        .history
        .iter()
        .enumerate()
        .flat_map(|(index, entry)| {
            let style = match entry.kind {
                HistoryKind::User => Style::default().fg(COLOR_TEAL),
                HistoryKind::Agent => Style::default().fg(COLOR_PURPLE),
                HistoryKind::Client => Style::default().fg(Color::Yellow),
                HistoryKind::Warning | HistoryKind::LogPath => Style::default().fg(Color::Yellow),
                HistoryKind::Event => Style::default().fg(COLOR_DARK_GRAY),
            };
            let prefix = if ui.timestamps && !entry.timestamp.is_empty() {
                format!("[{}] ", entry.timestamp)
            } else {
                String::new()
            };
            if ui.timestamps && !entry.timestamp.is_empty() {
                let available_width = chunks[1].width.saturating_sub(2) as usize;
                let timestamp = format!("[{}]", entry.timestamp);
                let text_width = entry.text.chars().count();
                let timestamp_width = timestamp.chars().count();
                let entry_style = selected_style(style, ui.selected_message == Some(index));
                if text_width + timestamp_width < available_width {
                    vec![Line::from(vec![
                        Span::styled(&entry.text, entry_style),
                        Span::raw(" ".repeat(available_width - text_width - timestamp_width)),
                        Span::styled(timestamp, entry_style.add_modifier(Modifier::DIM)),
                    ])]
                } else {
                    vec![
                        Line::from(Span::styled(&entry.text, entry_style)),
                        Line::from(vec![
                            Span::raw(" ".repeat(available_width.saturating_sub(timestamp_width))),
                            Span::styled(timestamp, entry_style.add_modifier(Modifier::DIM)),
                        ]),
                    ]
                }
            } else {
                vec![Line::from(Span::styled(
                    format!("{prefix}{}", entry.text),
                    selected_style(style, ui.selected_message == Some(index)),
                ))]
            }
        })
        .collect::<Vec<_>>();
    f.render_widget(
        Paragraph::new(body).wrap(Wrap { trim: false }).block(
            Block::default()
                .borders(Borders::ALL)
                .title(Span::styled(
                    " CONVERSATION / EVENTS ",
                    Style::default().fg(COLOR_TEAL).add_modifier(Modifier::BOLD),
                ))
                .border_style(Style::default().fg(COLOR_DARK_GRAY))
                .style(Style::default().bg(COLOR_BG)),
        ),
        chunks[1],
    );
    f.render_widget(
        Paragraph::new(format!(
            " LISTEN: {}   |   TTS: {} ",
            concise_listen_model(STT_MODELS[ui.stt_index].1),
            concise_tts_voice(TTS_VOICES[ui.voice_index]),
        ))
        .style(Style::default().fg(COLOR_TEAL).bg(COLOR_BG))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(COLOR_DARK_GRAY))
                .style(Style::default().bg(COLOR_BG)),
        ),
        chunks[2],
    );
    if ui.editor {
        draw_editor(f, ui);
    } else if ui.input_mode.is_some() {
        draw_input(f, ui);
    } else if let Some(kind) = ui.chooser {
        draw_chooser(f, ui, kind);
    } else if ui.palette {
        draw_palette(f, ui);
    }
}

fn concise_listen_model(model: &str) -> &str {
    match model {
        "flux-general-en" => "Flux English",
        "flux-general-multi" => "Flux Multilingual",
        "nova-3" => "Nova-3",
        other => other,
    }
}

fn concise_tts_voice(model: &str) -> String {
    let (family, voice) = if let Some(value) = model.strip_prefix("aura-2-") {
        ("Aura-2", value)
    } else if let Some(value) = model.strip_prefix("flux-") {
        ("Flux", value)
    } else {
        return model.to_string();
    };
    let name = voice
        .rsplit_once('-')
        .map(|(name, _)| name)
        .unwrap_or(voice);
    format!("{name} ({family})")
}
fn draw_input(f: &mut ratatui::Frame, ui: &Ui) {
    let area = centered(f.area(), 78, 35);
    f.render_widget(Clear, area);
    let (title, hint) = match ui.input_mode {
        Some(InputMode::AgentMessage) => (
            " INJECT AGENT MESSAGE ",
            "TAB changes behavior: default / queue / interrupt",
        ),
        Some(InputMode::UserMessage) => (
            " INJECT USER MESSAGE ",
            "Enter sends the text as if the user spoke it",
        ),
        Some(InputMode::ThinkModel) => (
            " UPDATE THINK MODEL ",
            "Enter sends this model with the current system prompt",
        ),
        None => (" INPUT ", ""),
    };
    let behavior = if matches!(ui.input_mode, Some(InputMode::AgentMessage)) {
        format!(
            "\nBehavior: {}",
            ["default", "queue", "interrupt"][ui.input_behavior]
        )
    } else {
        String::new()
    };
    let before = &ui.input_buffer[..ui.input_cursor.min(ui.input_buffer.len())];
    let value = format!(
        "{}▌{}{}",
        before,
        &ui.input_buffer[ui.input_cursor.min(ui.input_buffer.len())..],
        behavior
    );
    let content = vec![
        Line::from(value),
        Line::from(""),
        Line::from(Span::styled(
            format!("{}  |  Enter send  |  Esc cancel", hint),
            Style::default().fg(COLOR_DARK_GRAY),
        )),
    ];
    f.render_widget(
        Paragraph::new(content).wrap(Wrap { trim: false }).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(COLOR_PURPLE))
                .style(Style::default().bg(COLOR_BG))
                .title(Span::styled(
                    title,
                    Style::default()
                        .fg(COLOR_PURPLE)
                        .add_modifier(Modifier::BOLD),
                )),
        ),
        area,
    );
}
fn draw_palette(f: &mut ratatui::Frame, ui: &Ui) {
    let area = centered(f.area(), 70, 75);
    f.render_widget(Clear, area);
    let filtered = ui.filtered();
    let items = filtered
        .iter()
        .map(|i| {
            let shortcut = COMMANDS[*i]
                .3
                .map(|key| format!("  [{key}]"))
                .unwrap_or_default();
            let available_width = area.width.saturating_sub(4) as usize;
            let name = COMMANDS[*i].0;
            let padding =
                available_width.saturating_sub(name.chars().count() + shortcut.chars().count());
            ListItem::new(vec![
                Line::from(vec![
                    Span::styled(name, Style::default().fg(COLOR_TEAL)),
                    Span::raw(" ".repeat(padding)),
                    Span::styled(shortcut, Style::default().fg(COLOR_PURPLE)),
                ]),
                Line::from(Span::styled(
                    format!("  {}", COMMANDS[*i].1),
                    Style::default().fg(COLOR_DARK_GRAY),
                )),
            ])
        })
        .collect::<Vec<_>>();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(
            Style::default()
                .fg(COLOR_PURPLE)
                .add_modifier(Modifier::BOLD),
        )
        .style(Style::default().bg(COLOR_BG))
        .title(Line::from(vec![
            Span::styled(
                " COMMAND PALETTE ",
                Style::default()
                    .fg(COLOR_PURPLE)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" > {}█", ui.query),
                Style::default().fg(COLOR_TEAL).add_modifier(Modifier::BOLD),
            ),
        ]));
    let list = List::new(items)
        .block(block)
        .style(Style::default().fg(COLOR_TEAL).bg(COLOR_BG))
        .highlight_style(
            Style::default()
                .fg(Color::White)
                .bg(COLOR_PURPLE)
                .add_modifier(Modifier::BOLD),
        );
    let mut state = ratatui::widgets::ListState::default();
    state.select(Some(ui.selected.min(filtered.len().saturating_sub(1))));
    f.render_stateful_widget(list, area, &mut state);
}
fn draw_chooser(f: &mut ratatui::Frame, ui: &Ui, kind: usize) {
    let area = centered(f.area(), 65, 65);
    f.render_widget(Clear, area);
    let tts_indices = ui.filtered_tts_indices();
    let values: Vec<String> = if kind == 0 {
        tts_indices
            .iter()
            .map(|i| TTS_VOICES[*i].to_string())
            .collect()
    } else if kind == 1 {
        STT_MODELS
            .iter()
            .map(|(n, m, v)| format!("{n} ({m}, {v})"))
            .collect()
    } else if kind == 2 {
        STT_LANGUAGES.iter().map(|s| s.to_string()).collect()
    } else {
        ui.user_config.historical_prompts.clone()
    };
    let chooser_title = if kind == 0 {
        Line::from(vec![
            Span::styled(
                " TTS VOICE SELECTOR ",
                Style::default()
                    .fg(COLOR_PURPLE)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" > {}█", ui.chooser_query),
                Style::default().fg(COLOR_TEAL).add_modifier(Modifier::BOLD),
            ),
        ])
    } else {
        Line::from(Span::styled(
            " CHOOSE WITH UP/DN, ENTER, ESC ",
            Style::default()
                .fg(COLOR_PURPLE)
                .add_modifier(Modifier::BOLD),
        ))
    };
    let list = List::new(values.into_iter().map(ListItem::new).collect::<Vec<_>>())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(
                    Style::default()
                        .fg(COLOR_PURPLE)
                        .add_modifier(Modifier::BOLD),
                )
                .style(Style::default().bg(COLOR_BG))
                .title(chooser_title),
        )
        .style(Style::default().fg(COLOR_TEAL).bg(COLOR_BG))
        .highlight_style(
            Style::default()
                .fg(Color::White)
                .bg(COLOR_PURPLE)
                .add_modifier(Modifier::BOLD),
        );
    let mut state = ratatui::widgets::ListState::default();
    state.select(Some(if kind == 0 {
        tts_indices
            .iter()
            .position(|i| *i == ui.voice_index)
            .unwrap_or(0)
    } else if kind == 1 {
        ui.stt_index
    } else if kind == 2 {
        STT_LANGUAGES
            .iter()
            .position(|v| *v == ui.stt_language)
            .unwrap_or(0)
    } else {
        ui.history_index
            .min(ui.user_config.historical_prompts.len().saturating_sub(1))
    }));
    f.render_stateful_widget(list, area, &mut state);
}
fn draw_editor(f: &mut ratatui::Frame, ui: &Ui) {
    let area = centered(f.area(), 85, 80);
    f.render_widget(Clear, area);
    let before = &ui.prompt[..ui.cursor.min(ui.prompt.len())];
    let text = format!(
        "{}▌{}",
        before,
        &ui.prompt[ui.cursor.min(ui.prompt.len())..]
    );
    f.render_widget(
        Paragraph::new(text).wrap(Wrap { trim: false }).block(
            Block::default().borders(Borders::ALL).title(
                " System prompt editor — arrows/Ctrl+arrows/Home/End, Ctrl+S apply, Esc cancel ",
            ),
        ),
        area,
    );
}
fn centered(r: ratatui::layout::Rect, x: u16, y: u16) -> ratatui::layout::Rect {
    let v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - y) / 2),
            Constraint::Percentage(y),
            Constraint::Percentage((100 - y) / 2),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - x) / 2),
            Constraint::Percentage(x),
            Constraint::Percentage((100 - x) / 2),
        ])
        .split(v[1])[1]
}

fn previous_word_boundary(text: &str, cursor: usize) -> usize {
    let mut index = cursor.min(text.len());
    while index > 0 {
        let (start, character) = text[..index].char_indices().next_back().unwrap();
        if !character.is_whitespace() {
            break;
        }
        index = start;
    }
    while index > 0 {
        let (start, character) = text[..index].char_indices().next_back().unwrap();
        if character.is_whitespace() {
            break;
        }
        index = start;
    }
    index
}

fn next_word_boundary(text: &str, cursor: usize) -> usize {
    let mut index = cursor.min(text.len());
    let remaining = &text[index..];
    let mut in_word = false;
    for (offset, character) in remaining.char_indices() {
        if character.is_whitespace() {
            if in_word {
                index += offset;
                break;
            }
        } else {
            in_word = true;
        }
        index = cursor + offset + character.len_utf8();
    }
    while index < text.len() {
        let character = text[index..].chars().next().unwrap();
        if !character.is_whitespace() {
            break;
        }
        index += character.len_utf8();
    }
    index
}

async fn handle_key(
    ui: &mut Ui,
    key: KeyEvent,
    args: &LaunchOptions,
    worker: &mut Option<mpsc::UnboundedSender<WorkerCommand>>,
    event_tx: &mpsc::UnboundedSender<String>,
) -> Result<bool, Box<dyn std::error::Error>> {
    if ui.editor {
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => ui.editor = false,
            (KeyCode::Char('s'), KeyModifiers::CONTROL) => {
                if let Some(tx) = worker {
                    let _ = tx.send(WorkerCommand::Json(
                        serde_json::json!({"type":"UpdatePrompt","prompt":ui.prompt}).to_string(),
                    ));
                    ui.push_client("→ UpdatePrompt sent");
                }
                ui.remember_prompt();
                ui.save_config();
                ui.editor = false;
            }
            (KeyCode::Left, modifiers) if modifiers.contains(KeyModifiers::CONTROL) => {
                ui.cursor = previous_word_boundary(&ui.prompt, ui.cursor);
            }
            (KeyCode::Right, modifiers) if modifiers.contains(KeyModifiers::CONTROL) => {
                ui.cursor = next_word_boundary(&ui.prompt, ui.cursor);
            }
            (KeyCode::Left, _) => ui.cursor = ui.cursor.saturating_sub(1),
            (KeyCode::Right, _) => ui.cursor = (ui.cursor + 1).min(ui.prompt.len()),
            (KeyCode::Home, _) => ui.cursor = 0,
            (KeyCode::End, _) => ui.cursor = ui.prompt.len(),
            (KeyCode::Backspace, _) => {
                if ui.cursor > 0 {
                    ui.prompt.remove(ui.cursor - 1);
                    ui.cursor -= 1;
                }
            }
            (KeyCode::Delete, _) => {
                if ui.cursor < ui.prompt.len() {
                    ui.prompt.remove(ui.cursor);
                }
            }
            (KeyCode::Char(c), _) => {
                ui.prompt.insert(ui.cursor, c);
                ui.cursor += 1;
            }
            _ => {}
        }
        return Ok(false);
    }
    if let Some(mode) = ui.input_mode {
        match key.code {
            KeyCode::Esc => ui.input_mode = None,
            KeyCode::Tab if matches!(mode, InputMode::AgentMessage) => {
                ui.input_behavior = (ui.input_behavior + 1) % 3;
            }
            KeyCode::Enter => {
                if let Some(tx) = worker {
                    let json = match mode {
                        InputMode::AgentMessage => {
                            let behavior = ["default", "queue", "interrupt"][ui.input_behavior];
                            serde_json::json!({
                                "type": "InjectAgentMessage",
                                "behavior": behavior,
                                "message": ui.input_buffer,
                            })
                        }
                        InputMode::UserMessage => serde_json::json!({
                            "type": "InjectUserMessage",
                            "content": ui.input_buffer,
                        }),
                        InputMode::ThinkModel => serde_json::json!({
                            "type": "UpdateThink",
                            "think": {"provider": {"type": args.think_type, "model": ui.input_buffer}, "prompt": ui.prompt}
                        }),
                    };
                    let _ = tx.send(WorkerCommand::Json(json.to_string()));
                    ui.push_client(match mode {
                        InputMode::AgentMessage => "→ InjectAgentMessage sent",
                        InputMode::UserMessage => "→ InjectUserMessage sent",
                        InputMode::ThinkModel => "→ UpdateThink sent",
                    });
                }
                ui.input_mode = None;
            }
            KeyCode::Left => ui.input_cursor = ui.input_cursor.saturating_sub(1),
            KeyCode::Right => ui.input_cursor = (ui.input_cursor + 1).min(ui.input_buffer.len()),
            KeyCode::Home => ui.input_cursor = 0,
            KeyCode::End => ui.input_cursor = ui.input_buffer.len(),
            KeyCode::Backspace => {
                if ui.input_cursor > 0 {
                    ui.input_buffer.remove(ui.input_cursor - 1);
                    ui.input_cursor -= 1;
                }
            }
            KeyCode::Delete => {
                if ui.input_cursor < ui.input_buffer.len() {
                    ui.input_buffer.remove(ui.input_cursor);
                }
            }
            KeyCode::Char(c) => {
                ui.input_buffer.insert(ui.input_cursor, c);
                ui.input_cursor += 1;
            }
            _ => {}
        }
        return Ok(false);
    }
    if let Some(kind) = ui.chooser {
        match key.code {
            KeyCode::Esc => ui.chooser = None,
            KeyCode::Up => {
                if kind == 0 {
                    let indices = ui.filtered_tts_indices();
                    if !indices.is_empty() {
                        let position = indices
                            .iter()
                            .position(|i| *i == ui.voice_index)
                            .unwrap_or(0);
                        ui.voice_index =
                            indices[position.checked_sub(1).unwrap_or(indices.len() - 1)];
                    }
                } else if kind == 1 {
                    ui.stt_index = ui.stt_index.saturating_sub(1)
                } else if kind == 2 {
                    let i = STT_LANGUAGES
                        .iter()
                        .position(|v| *v == ui.stt_language)
                        .unwrap_or(0);
                    ui.stt_language = STT_LANGUAGES[i.saturating_sub(1)].into();
                } else {
                    ui.history_index = ui.history_index.saturating_sub(1);
                }
            }
            KeyCode::Down => {
                if kind == 0 {
                    let indices = ui.filtered_tts_indices();
                    if !indices.is_empty() {
                        let position = indices
                            .iter()
                            .position(|i| *i == ui.voice_index)
                            .unwrap_or(0);
                        ui.voice_index = indices[(position + 1) % indices.len()];
                    }
                } else if kind == 1 {
                    ui.stt_index = (ui.stt_index + 1) % STT_MODELS.len()
                } else if kind == 2 {
                    let i = STT_LANGUAGES
                        .iter()
                        .position(|v| *v == ui.stt_language)
                        .unwrap_or(0);
                    ui.stt_language = STT_LANGUAGES[(i + 1) % STT_LANGUAGES.len()].into();
                } else if !ui.user_config.historical_prompts.is_empty() {
                    ui.history_index =
                        (ui.history_index + 1) % ui.user_config.historical_prompts.len();
                }
            }
            KeyCode::Enter => {
                if kind == 3 {
                    if let Some(prompt) = ui
                        .user_config
                        .historical_prompts
                        .get(ui.history_index)
                        .cloned()
                    {
                        ui.prompt = prompt;
                        ui.remember_prompt();
                        ui.save_config();
                        if let Some(tx) = worker {
                            let _ = tx.send(WorkerCommand::Json(
                                serde_json::json!({"type":"UpdatePrompt","prompt":ui.prompt})
                                    .to_string(),
                            ));
                            ui.push_client("→ Historical UpdatePrompt sent");
                        }
                    } else {
                        ui.push("No historical system prompts saved");
                    }
                    ui.chooser = None;
                    return Ok(false);
                }
                if let Some(tx) = worker {
                    let json = if kind == 0 {
                        update_speak_message(TTS_VOICES[ui.voice_index])
                    } else if kind == 1 {
                        let (_, model, version) = STT_MODELS[ui.stt_index];
                        update_listen_message(
                            model,
                            version,
                            &ui.stt_language,
                            &ui.user_config.listen,
                        )
                    } else if kind == 2 {
                        update_listen_message(
                            STT_MODELS[ui.stt_index].1,
                            STT_MODELS[ui.stt_index].2,
                            &ui.stt_language,
                            &ui.user_config.listen,
                        )
                    } else {
                        serde_json::Value::Null
                    };
                    let _ = tx.send(WorkerCommand::Json(json.to_string()));
                    ui.push_client("→ Voice configuration update sent");
                }
                if kind <= 2 {
                    ui.save_preferences(args);
                }
                ui.chooser = None
            }
            KeyCode::Backspace if kind == 0 => {
                ui.chooser_query.pop();
                if !ui.filtered_tts_indices().contains(&ui.voice_index) {
                    if let Some(index) = ui.filtered_tts_indices().first() {
                        ui.voice_index = *index;
                    }
                }
            }
            KeyCode::Char(character) if kind == 0 => {
                ui.chooser_query.push(character);
                if let Some(index) = ui.filtered_tts_indices().first() {
                    ui.voice_index = *index;
                }
            }
            _ => {}
        }
        return Ok(false);
    }
    if ui.palette {
        match key.code {
            KeyCode::Esc => ui.palette = false,
            KeyCode::Up => {
                let len = ui.filtered().len();
                if len > 0 {
                    ui.selected = ui.selected.checked_sub(1).unwrap_or(len - 1);
                }
            }
            KeyCode::Down => {
                let len = ui.filtered().len();
                if len > 0 {
                    ui.selected = (ui.selected + 1) % len;
                }
            }
            KeyCode::Backspace => {
                ui.query.pop();
                ui.selected = 0
            }
            KeyCode::Char(c) => {
                ui.query.push(c);
                ui.selected = 0
            }
            KeyCode::Enter => {
                let ids = ui.filtered();
                if let Some(i) = ids.get(ui.selected) {
                    let cmd = COMMANDS[*i].2.clone();
                    ui.palette = false;
                    return execute_command(ui, cmd, args, worker, event_tx).await;
                }
            }
            _ => {}
        }
        return Ok(false);
    }
    match (key.code,key.modifiers) {
        (KeyCode::Char('q'), KeyModifiers::NONE) | (KeyCode::Esc, _) => return Ok(true),
        (KeyCode::Char('p') | KeyCode::Char('P'), KeyModifiers::CONTROL) => { ui.palette=true; ui.query.clear(); ui.selected=0; }
        (KeyCode::Char(' '), KeyModifiers::NONE) => {
            let command = if ui.connected || worker.is_some() { Command::Disconnect } else { Command::Connect };
            return execute_command(ui, command, args, worker, event_tx).await;
        }
        (KeyCode::Char('?'), _) => ui.push("SPACE connect/disconnect | CTRL+P palette | UP/DN navigate | ENTER execute | ESC cancel | CTRL+S apply prompt | Q quit"),
        _ => {}
    }
    Ok(false)
}

async fn execute_command(
    ui: &mut Ui,
    cmd: Command,
    args: &LaunchOptions,
    worker: &mut Option<mpsc::UnboundedSender<WorkerCommand>>,
    event_tx: &mpsc::UnboundedSender<String>,
) -> Result<bool, Box<dyn std::error::Error>> {
    match cmd {
        Command::Connect => {
            if worker.is_none() {
                let (tx, rx) = mpsc::unbounded_channel();
                *worker = Some(tx);
                let mut a = args.clone();
                a.prompt = Some(ui.prompt.clone());
                a.speak_provider = "deepgram".into();
                a.speak_model = TTS_VOICES[ui.voice_index].into();
                a.listen_model = STT_MODELS[ui.stt_index].1.into();
                a.listen_version = Some(STT_MODELS[ui.stt_index].2.into());
                a.listen_language = ui.stt_language.clone();
                a.listen_provider = ui
                    .user_config
                    .listen
                    .provider
                    .clone()
                    .unwrap_or_else(|| "deepgram".into());
                a.language_hints = ui.user_config.listen.language_hints.clone();
                a.listen_keyterms = ui.user_config.listen.keyterms.clone();
                a.listen_eot_threshold = ui.user_config.listen.eot_threshold;
                a.listen_eager_eot_threshold = ui.user_config.listen.eager_eot_threshold;
                a.listen_smart_format = ui.user_config.listen.smart_format;
                ui.remember_prompt();
                ui.save_preferences(args);
                let out = event_tx.clone();
                let json_logging = ui.json_logging;
                tokio::spawn(async move {
                    if let Err(e) = worker_loop(a, rx, out.clone(), json_logging).await {
                        let _ = out.send(format!("__CONNECTION_ERROR__|{e}"));
                    }
                });
                ui.status = "Connecting…".into();
            }
        }
        Command::Disconnect => {
            if let Some(tx) = worker {
                let _ = tx.send(WorkerCommand::Stop);
                *worker = None;
                ui.connected = false;
                ui.status = "Disconnected".into();
            }
        }
        Command::Prompt => {
            ui.editor = true;
            ui.cursor = ui.prompt.len()
        }
        Command::HistoricalPrompt => {
            ui.chooser = Some(3);
            ui.history_index = 0;
        }
        Command::Tts => {
            ui.chooser = Some(0);
            ui.chooser_query.clear();
        }
        Command::Stt => ui.chooser = Some(1),
        Command::SttLanguage => ui.chooser = Some(2),
        Command::UpdateThink => {
            ui.input_mode = Some(InputMode::ThinkModel);
            ui.input_buffer = args.think_model.clone();
            ui.input_cursor = ui.input_buffer.len();
        }
        Command::ToggleTimestamps => {
            ui.timestamps = !ui.timestamps;
            ui.user_config.show_timestamps = ui.timestamps;
            ui.save_config();
            ui.push(format!(
                "Message timestamps {}",
                if ui.timestamps { "enabled" } else { "disabled" }
            ));
        }
        Command::ToggleJsonLogging => {
            ui.json_logging = !ui.json_logging;
            ui.user_config.verbose_json_logging = ui.json_logging;
            ui.save_config();
            if let Some(tx) = worker {
                let _ = tx.send(WorkerCommand::SetJsonLogging(ui.json_logging));
            }
            ui.push(format!(
                "Verbose JSON logging {}",
                if ui.json_logging {
                    "enabled"
                } else {
                    "disabled"
                }
            ));
        }
        Command::CopyConversation => {
            let conversation = ui
                .history
                .iter()
                .filter(|entry| matches!(entry.kind, HistoryKind::User | HistoryKind::Agent))
                .map(|entry| {
                    if ui.timestamps && !entry.timestamp.is_empty() {
                        format!("[{}] {}", entry.timestamp, entry.text)
                    } else {
                        entry.text.clone()
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            match arboard::Clipboard::new()
                .and_then(|mut clipboard| clipboard.set_text(conversation))
            {
                Ok(()) => ui.push_client("→ Conversation copied to clipboard"),
                Err(error) => ui.push(format!("Clipboard error: {error}")),
            }
        }
        Command::CopyRequestId => match ui.request_id.clone() {
            Some(request_id) => match arboard::Clipboard::new()
                .and_then(|mut clipboard| clipboard.set_text(request_id))
            {
                Ok(()) => ui.push_client("→ Request ID copied to clipboard"),
                Err(error) => ui.push(format!("Clipboard error: {error}")),
            },
            None => ui.push("No active Voice Agent request ID"),
        },
        Command::InjectAgent => {
            ui.input_mode = Some(InputMode::AgentMessage);
            ui.input_buffer.clear();
            ui.input_cursor = 0;
            ui.input_behavior = 0;
        }
        Command::InjectUser => {
            ui.input_mode = Some(InputMode::UserMessage);
            ui.input_buffer.clear();
            ui.input_cursor = 0;
        }
        Command::ForceEnd => {
            if let Some(tx) = worker {
                let _ = tx.send(WorkerCommand::Json(
                    serde_json::json!({"type":"ForceEndTurn"}).to_string(),
                ));
                ui.push_client("→ ForceEndTurn sent");
            }
        }
        Command::KeepAlive => {
            if let Some(tx) = worker {
                let _ = tx.send(WorkerCommand::Json(
                    serde_json::json!({"type":"AgentKeepAlive"}).to_string(),
                ));
                ui.push_client("→ AgentKeepAlive sent");
            }
        }
        Command::Help => ui.push(
            "Commands are searchable with Ctrl+P; connect first, then use update/inject controls.",
        ),
        Command::Quit => return Ok(true),
    }
    Ok(false)
}

async fn worker_loop(
    args: LaunchOptions,
    mut commands: mpsc::UnboundedReceiver<WorkerCommand>,
    out: mpsc::UnboundedSender<String>,
    json_logging: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    validate_speak_options(&args)?;
    let key = env::var("DEEPGRAM_API_KEY").map_err(|_| "DEEPGRAM_API_KEY is not set")?;
    let capture = AudioCapture::new(args.audio_sample_rate)?;
    let rate = capture.config.sample_rate.0;
    let channels = capture.config.channels;
    let mic = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let (audio_tx, mut audio_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let (play_tx, play_rx) = std_mpsc::channel::<Vec<u8>>();
    let mute = !args.no_mic_mute;
    let mic_for_player = mic.clone();
    std::thread::spawn(move || {
        if let Ok(player) = AudioPlayer::new(mic_for_player, mute, true) {
            for audio in play_rx {
                let _ = player.play_audio(audio);
            }
        }
    });
    let mut ws =
        connect_to_voice_agent(&key, &args.endpoint, rate, channels, args.verbose, false).await?;
    let welcome = ws
        .next()
        .await
        .ok_or("Voice Agent connection closed before Welcome message")??;
    let welcome_text = match welcome {
        Message::Text(text) => text.to_string(),
        message => {
            return Err(
                format!("expected Voice Agent Welcome message, received {message:?}").into(),
            )
        }
    };
    let welcome_response = serde_json::from_str::<VoiceAgentResponse>(&welcome_text)?;
    let request_id = welcome_response
        .data
        .get("request_id")
        .and_then(|value| value.as_str())
        .ok_or("Voice Agent Welcome message did not include request_id")?
        .to_string();
    let _ = out.send(format!("__REQUEST_ID__|{request_id}"));
    let mut json_history = Vec::<(String, String)>::new();
    let mut json_log = if json_logging {
        Some(open_json_log(&request_id)?)
    } else {
        None
    };
    record_json_message(&mut json_history, &mut json_log, "SERVER", &welcome_text)?;
    let config = serde_json::to_string(&config_from_options(
        &args,
        rate,
        load_eleven_labs_api_key(&args)?,
    ))?;
    let (mut sink, mut stream) = ws.split();
    record_json_message(&mut json_history, &mut json_log, "CLIENT", &config)?;
    sink.send(Message::Text(config.into())).await?;
    let _ = out.send("__CONNECTED__".into());
    if let Some(log) = &json_log {
        let _ = out.send(format!("__JSON_LOG__|{}", log.path.display()));
    }
    // CPAL streams are not Send on some platforms (notably macOS). Keep the
    // stream on its own OS thread while the WebSocket worker remains Send.
    let encoding = args.audio_encoding.clone();
    std::thread::spawn(move || {
        if let Ok(_stream) = capture.start_capture(audio_tx, mic, &encoding) {
            loop {
                std::thread::sleep(Duration::from_secs(60));
            }
        }
    });
    loop {
        tokio::select! {
            Some(command) = commands.recv() => match command {
                WorkerCommand::Stop => break,
                WorkerCommand::Json(json) => {
                    record_json_message(&mut json_history, &mut json_log, "CLIENT", &json)?;
                    sink.send(Message::Text(json.into())).await?
                }
                WorkerCommand::SetJsonLogging(enabled) => {
                    if enabled && json_log.is_none() {
                        let log = open_json_log(&request_id)?;
                        let mut log_option = Some(log);
                        for (direction, json) in &json_history {
                            log_json_message(&mut log_option, direction, json)?;
                        }
                        json_log = log_option;
                        if let Some(log) = &json_log {
                            let _ = out.send(format!("__JSON_LOG__|{}", log.path.display()));
                        }
                    } else if !enabled {
                        json_log = None;
                    }
                }
            },
            Some(message) = stream.next() => match message? {
                Message::Text(text) => {
                    let text = text.to_string();
                    record_json_message(&mut json_history, &mut json_log, "SERVER", &text)?;
                    if let Ok(response) = serde_json::from_str::<VoiceAgentResponse>(&text) {
                        match response.message_type.as_str() {
                            "ConversationText" => {
                                let role = response.data.get("role").and_then(|v| v.as_str()).unwrap_or("?");
                                let content = response.data.get("content").and_then(|v| v.as_str()).unwrap_or("");
                                let _ = out.send(format!("__MESSAGE__|{}|{}", if role == "user" { "You" } else { "Agent" }, content));
                            }
                            "Warning" => { let _ = out.send(format!("__WARNING__|Warning: {}", response.data)); }
                            "Error" => { let _ = out.send(format!("Error: {}", response.data)); }
                            _ => {}
                        }
                    }
                }
                Message::Binary(audio) => { let _ = play_tx.send(audio.to_vec()); }
                _ => {}
            },
            Some(audio) = audio_rx.recv() => sink.send(Message::Binary(audio.into())).await?,
            else => break,
        }
    }
    let _ = out.send("__DISCONNECTED__".into());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_listen_for_nova_includes_only_v1_language_configuration() {
        let message = update_listen_message(
            "nova-3",
            "v1",
            "es",
            &crate::config::ListenConfig::default(),
        );
        let provider = &message["listen"]["provider"];
        assert_eq!(provider["type"], "deepgram");
        assert_eq!(provider["model"], "nova-3");
        assert_eq!(provider["version"], "v1");
        assert_eq!(provider["language"], "es");
        assert!(provider.get("keyterms").is_none());
    }

    #[test]
    fn update_listen_for_flux_omits_language_and_preserves_supported_fields() {
        let preferences = crate::config::ListenConfig {
            eot_threshold: Some(0.7),
            eager_eot_threshold: Some(0.5),
            keyterms: vec!["Deepgram".into()],
            language_hints: vec!["en-US".into(), "es".into()],
            ..Default::default()
        };
        let message = update_listen_message("flux-general-multi", "v2", "en", &preferences);
        let provider = &message["listen"]["provider"];
        assert_eq!(provider["version"], "v2");
        assert!(provider.get("language").is_none());
        assert_eq!(provider["keyterms"], serde_json::json!(["Deepgram"]));
        assert_eq!(
            provider["language_hints"],
            serde_json::json!(["en-US", "es"])
        );
    }

    #[test]
    fn update_speak_uses_deepgram_model_and_version_fields() {
        let flux = update_speak_message("flux-alexis-en");
        let aura = update_speak_message("aura-2-thalia-en");
        assert_eq!(
            flux["speak"]["provider"],
            serde_json::json!({"type":"deepgram","version":"v2","model":"flux-alexis-en"})
        );
        assert_eq!(
            aura["speak"]["provider"],
            serde_json::json!({"type":"deepgram","version":"v1","model":"aura-2-thalia-en"})
        );
        assert!(flux["speak"]["provider"].get("voice").is_none());
    }

    #[test]
    fn excludes_the_invalid_perseo_aura_two_voice() {
        assert!(!TTS_VOICES.contains(&"aura-2-perseo-it"));
    }
}
