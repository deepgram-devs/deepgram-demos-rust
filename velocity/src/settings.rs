use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

use iced::widget::{
    button, checkbox, column, container, pick_list, progress_bar, row, scrollable, text, text_input,
};
use iced::{
    Background, Border, Color, Element, Font, Length, Shadow, Size, Subscription, Task, Theme,
    font, keyboard, overlay::menu, window,
};

use crate::audio::{self, AudioMeter};
use crate::config::{self, Config, OutputMode};
use crate::deepgram;
use crate::hotkey;
use crate::logger;
use crate::state;

static UI_COMMAND_SENDER: OnceLock<mpsc::Sender<UiCommand>> = OnceLock::new();

const DEFAULT_AUDIO_INPUT_LABEL: &str = "Default system input";
const SETTINGS_TITLE: &str = "Velocity Settings";
const API_KEY_TITLE: &str = "Velocity API Key";
const SECTION_SPACING: f32 = 14.0;
const PANEL_RADIUS: f32 = 4.0;
const UI_TICK_INTERVAL: Duration = Duration::from_millis(50);
const CONFIG_CHECK_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LaunchMode {
    Settings,
    ApiKey,
}

impl LaunchMode {
    fn title(self) -> &'static str {
        match self {
            LaunchMode::Settings => SETTINGS_TITLE,
            LaunchMode::ApiKey => API_KEY_TITLE,
        }
    }

    fn subtitle(self) -> &'static str {
        match self {
            LaunchMode::Settings => "Application settings",
            LaunchMode::ApiKey => "API key onboarding",
        }
    }
}

enum UiCommand {
    ShowSettings(Arc<state::AppState>),
    PromptForApiKey(mpsc::Sender<Option<String>>),
}

#[derive(Debug, Clone)]
enum Message {
    ApiKeyChanged(String),
    ModelSelected(String),
    LanguageSelected(String),
    SmartFormatToggled(bool),
    KeyTermsChanged(String),
    PushToTalkChanged(String),
    KeepTalkingChanged(String),
    StreamingChanged(String),
    ResendChanged(String),
    AudioInputSelected(String),
    HistoryLimitChanged(String),
    OutputModeSelected(String),
    AppendNewlineToggled(bool),
    SavePressed,
    ReloadPressed,
    KeyboardEvent(keyboard::Event),
    CloseRequested(window::Id),
    WindowOpened(window::Id),
    WindowClosed(window::Id),
    Tick,
}

pub fn prompt_for_api_key() -> Option<String> {
    let sender = ui_command_sender();
    let (tx, rx) = mpsc::channel();
    let _ = sender.send(UiCommand::PromptForApiKey(tx));
    rx.recv().ok().flatten()
}

pub fn show_settings_window() {
    let app = state::global();
    let _ = ui_command_sender().send(UiCommand::ShowSettings(app));
}

fn ui_command_sender() -> &'static mpsc::Sender<UiCommand> {
    UI_COMMAND_SENDER.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            run_ui_daemon(rx);
        });
        tx
    })
}

fn run_ui_daemon(commands: mpsc::Receiver<UiCommand>) {
    let commands = Arc::new(Mutex::new(Some(commands)));
    let result = iced::daemon(
        {
            let commands = Arc::clone(&commands);
            move || {
                let receiver = commands
                    .lock()
                    .unwrap()
                    .take()
                    .expect("settings UI booted more than once");
                SettingsWindow::new(receiver)
            }
        },
        update,
        view,
    )
    .title(window_title)
    .theme(app_theme)
    .subscription(subscription)
    .run();

    if let Err(error) = result {
        logger::verbose(&format!("Failed to run settings UI: {error}"));
    }
}

fn window_title(state: &SettingsWindow, _window: window::Id) -> String {
    state.window_title()
}

fn app_theme(_state: &SettingsWindow, _window: window::Id) -> Theme {
    Theme::Dark
}

fn page_background() -> Color {
    Color::from_rgb8(0x0B, 0x0B, 0x0C)
}

fn panel_background() -> Color {
    Color::from_rgb8(0x14, 0x14, 0x16)
}

fn panel_border() -> Color {
    Color::from_rgb8(0x28, 0x28, 0x2D)
}

fn secondary_panel_background() -> Color {
    Color::from_rgb8(0x10, 0x10, 0x12)
}

fn primary_text() -> Color {
    Color::from_rgb8(0xFF, 0xFF, 0xFF)
}

fn secondary_text() -> Color {
    Color::from_rgb8(0xA9, 0xA9, 0xAD)
}

fn muted_text() -> Color {
    Color::from_rgb8(0x78, 0x78, 0x80)
}

fn success_text() -> Color {
    Color::from_rgb8(0xD7, 0xF9, 0xE0)
}

fn error_text() -> Color {
    Color::from_rgb8(0xFF, 0xC1, 0xC1)
}

fn body_font() -> Font {
    Font::DEFAULT
}

fn heading_font() -> Font {
    Font {
        family: Font::DEFAULT.family,
        weight: font::Weight::Bold,
        ..Font::DEFAULT
    }
}

fn label_font() -> Font {
    Font {
        family: body_font().family,
        weight: font::Weight::Semibold,
        ..Font::DEFAULT
    }
}

fn subtle_status_color(status: &str) -> Color {
    let lower = status.to_ascii_lowercase();
    if lower.contains("fail")
        || lower.contains("error")
        || lower.contains("required")
        || lower.contains("reject")
        || lower.contains("invalid")
    {
        error_text()
    } else if lower.contains("saved") || lower.contains("loaded") {
        success_text()
    } else {
        secondary_text()
    }
}

fn panel_style() -> impl Fn(&Theme) -> container::Style {
    |_theme| {
        container::Style::default()
            .background(panel_background())
            .color(primary_text())
            .border(Border {
                color: panel_border(),
                width: 1.0,
                radius: PANEL_RADIUS.into(),
            })
    }
}

fn status_panel_style(has_warning: bool) -> impl Fn(&Theme) -> container::Style {
    move |_theme| {
        container::Style::default()
            .background(secondary_panel_background())
            .color(primary_text())
            .border(Border {
                color: if has_warning {
                    primary_text()
                } else {
                    panel_border()
                },
                width: 1.0,
                radius: PANEL_RADIUS.into(),
            })
    }
}

fn page_style() -> impl Fn(&Theme) -> container::Style {
    |_theme| {
        container::Style::default()
            .background(page_background())
            .color(primary_text())
    }
}

fn primary_button_style(_theme: &Theme, status: button::Status) -> button::Style {
    let base = button::Style {
        background: Some(Background::Color(primary_text())),
        text_color: page_background(),
        border: Border {
            color: primary_text(),
            width: 1.0,
            radius: PANEL_RADIUS.into(),
        },
        ..Default::default()
    };

    match status {
        button::Status::Hovered => button::Style {
            background: Some(Background::Color(Color::from_rgb8(0xE7, 0xE7, 0xEA))),
            ..base
        },
        button::Status::Pressed => button::Style {
            background: Some(Background::Color(Color::from_rgb8(0xD7, 0xD7, 0xDB))),
            ..base
        },
        button::Status::Disabled => button::Style {
            background: Some(Background::Color(Color::from_rgb8(0x66, 0x66, 0x6C))),
            text_color: Color::from_rgb8(0x1B, 0x1B, 0x1E),
            border: Border {
                color: Color::from_rgb8(0x66, 0x66, 0x6C),
                ..base.border
            },
            ..base
        },
        _ => base,
    }
}

fn secondary_button_style(_theme: &Theme, status: button::Status) -> button::Style {
    let base = button::Style {
        background: Some(Background::Color(panel_background())),
        text_color: primary_text(),
        border: Border {
            color: panel_border(),
            width: 1.0,
            radius: PANEL_RADIUS.into(),
        },
        ..Default::default()
    };

    match status {
        button::Status::Hovered => button::Style {
            background: Some(Background::Color(Color::from_rgb8(0x1B, 0x1B, 0x1E))),
            border: Border {
                color: primary_text(),
                ..base.border
            },
            ..base
        },
        button::Status::Pressed => button::Style {
            background: Some(Background::Color(Color::from_rgb8(0x20, 0x20, 0x24))),
            ..base
        },
        button::Status::Disabled => button::Style {
            text_color: muted_text(),
            ..base
        },
        _ => base,
    }
}

fn input_style(_theme: &Theme, status: text_input::Status) -> text_input::Style {
    let active = text_input::Style {
        background: Background::Color(secondary_panel_background()),
        border: Border {
            color: panel_border(),
            width: 1.0,
            radius: PANEL_RADIUS.into(),
        },
        icon: secondary_text(),
        placeholder: muted_text(),
        value: primary_text(),
        selection: Color::from_rgba8(0xFF, 0xFF, 0xFF, 0.18),
    };

    match status {
        text_input::Status::Hovered => text_input::Style {
            border: Border {
                color: secondary_text(),
                ..active.border
            },
            ..active
        },
        text_input::Status::Focused { .. } => text_input::Style {
            border: Border {
                color: primary_text(),
                ..active.border
            },
            ..active
        },
        text_input::Status::Disabled => text_input::Style {
            background: Background::Color(panel_background()),
            value: muted_text(),
            ..active
        },
        _ => active,
    }
}

fn pick_list_style(_theme: &Theme, status: pick_list::Status) -> pick_list::Style {
    let active = pick_list::Style {
        text_color: primary_text(),
        placeholder_color: muted_text(),
        handle_color: secondary_text(),
        background: Background::Color(secondary_panel_background()),
        border: Border {
            color: panel_border(),
            width: 1.0,
            radius: PANEL_RADIUS.into(),
        },
    };

    match status {
        pick_list::Status::Hovered | pick_list::Status::Opened { .. } => pick_list::Style {
            border: Border {
                color: primary_text(),
                ..active.border
            },
            ..active
        },
        _ => active,
    }
}

fn pick_list_menu_style(_theme: &Theme) -> menu::Style {
    menu::Style {
        background: panel_background().into(),
        border: Border {
            color: panel_border(),
            width: 1.0,
            radius: PANEL_RADIUS.into(),
        },
        text_color: primary_text(),
        selected_text_color: page_background(),
        selected_background: primary_text().into(),
        shadow: Shadow::default(),
    }
}

fn checkbox_style(_theme: &Theme, status: checkbox::Status) -> checkbox::Style {
    let (is_checked, hovered) = match status {
        checkbox::Status::Active { is_checked } => (is_checked, false),
        checkbox::Status::Hovered { is_checked } => (is_checked, true),
        checkbox::Status::Disabled { is_checked } => (is_checked, false),
    };

    checkbox::Style {
        background: if is_checked {
            Background::Color(primary_text())
        } else if hovered {
            Background::Color(Color::from_rgb8(0x1A, 0x1A, 0x1D))
        } else {
            Background::Color(secondary_panel_background())
        },
        icon_color: page_background(),
        border: Border {
            color: if hovered {
                primary_text()
            } else {
                panel_border()
            },
            width: 1.0,
            radius: PANEL_RADIUS.into(),
        },
        text_color: Some(primary_text()),
    }
}

fn meter_style(_theme: &Theme) -> progress_bar::Style {
    progress_bar::Style {
        background: Background::Color(secondary_panel_background()),
        bar: Background::Color(primary_text()),
        border: Border {
            color: panel_border(),
            width: 1.0,
            radius: PANEL_RADIUS.into(),
        },
    }
}

struct SettingsWindow {
    launch_mode: LaunchMode,
    current_window: Option<window::Id>,
    command_rx: mpsc::Receiver<UiCommand>,
    app: Option<Arc<state::AppState>>,
    completion: Option<mpsc::Sender<Option<String>>>,
    api_key: String,
    model: String,
    language: String,
    smart_format: bool,
    key_terms: String,
    push_to_talk: String,
    keep_talking: String,
    streaming: String,
    resend_selected: String,
    audio_input: String,
    history_limit: String,
    output_mode: String,
    append_newline: bool,
    meter: Option<AudioMeter>,
    meter_level: u8,
    meter_label: String,
    audio_inputs: Vec<String>,
    language_options: Vec<String>,
    status: String,
    config_changed_externally: bool,
    last_loaded_config_write_time: Option<SystemTime>,
    last_config_change_check: Instant,
    window_visible: bool,
}

impl SettingsWindow {
    fn new(command_rx: mpsc::Receiver<UiCommand>) -> (Self, Task<Message>) {
        let mut state = Self {
            launch_mode: LaunchMode::Settings,
            current_window: None,
            command_rx,
            app: None,
            completion: None,
            api_key: String::new(),
            model: deepgram::DEFAULT_MODEL.to_string(),
            language: deepgram::DO_NOT_SPECIFY_LANGUAGE_LABEL.to_string(),
            smart_format: false,
            key_terms: String::new(),
            push_to_talk: Config::default().hotkeys.push_to_talk,
            keep_talking: Config::default().hotkeys.keep_talking,
            streaming: Config::default().hotkeys.streaming,
            resend_selected: Config::default().hotkeys.resend_selected,
            audio_input: DEFAULT_AUDIO_INPUT_LABEL.to_string(),
            history_limit: config::DEFAULT_HISTORY_LIMIT.to_string(),
            output_mode: OutputMode::DirectInput.as_label().to_string(),
            append_newline: false,
            meter: None,
            meter_level: 0,
            meter_label: "Mic activity: unavailable".to_string(),
            audio_inputs: vec![DEFAULT_AUDIO_INPUT_LABEL.to_string()],
            language_options: vec![deepgram::DO_NOT_SPECIFY_LANGUAGE_LABEL.to_string()],
            status: String::new(),
            config_changed_externally: false,
            last_loaded_config_write_time: None,
            last_config_change_check: Instant::now(),
            window_visible: false,
        };

        state.refresh_audio_inputs();
        state.reload_from_disk();
        (state, Task::none())
    }

    fn window_title(&self) -> String {
        self.launch_mode.title().to_string()
    }

    fn runtime_status(&self) -> String {
        if let Some(app) = &self.app {
            if let Some(error) = app.last_error() {
                return error;
            }

            let config = app.config();
            return format!(
                "Current mode: recording={} keep-talking={} streaming={} selected mic={}",
                app.is_recording(),
                app.is_keep_talking(),
                app.is_streaming(),
                config
                    .audio_input
                    .as_deref()
                    .unwrap_or(DEFAULT_AUDIO_INPUT_LABEL),
            );
        }

        "Configuration required before Velocity can start recording".to_string()
    }

    fn selected_audio_input(&self) -> Option<&str> {
        (self.audio_input != DEFAULT_AUDIO_INPUT_LABEL).then_some(self.audio_input.as_str())
    }

    fn refresh_audio_inputs(&mut self) {
        let mut inputs = audio::list_input_devices()
            .into_iter()
            .map(|device| device.name)
            .collect::<Vec<_>>();

        if !inputs
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(DEFAULT_AUDIO_INPUT_LABEL))
        {
            inputs.insert(0, DEFAULT_AUDIO_INPUT_LABEL.to_string());
        }

        if !self.audio_input.trim().is_empty()
            && !inputs
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(&self.audio_input))
        {
            inputs.push(self.audio_input.clone());
        }

        self.audio_inputs = inputs;
    }

    fn reload_from_disk(&mut self) {
        self.refresh_audio_inputs();

        match config::load_state() {
            Ok(loaded) => {
                self.apply_config(&loaded.config);
                self.last_loaded_config_write_time = loaded.modified_at;
                self.config_changed_externally = false;
                self.status = format!("Loaded {}", config::config_path().display());
            }
            Err(error) => {
                self.status = error;
            }
        }
    }

    fn apply_config(&mut self, config: &Config) {
        self.api_key = config.api_key.clone().unwrap_or_default();
        self.model = config.model.clone();
        self.language_options = language_options_for_model(&self.model);
        self.language = config
            .language
            .as_deref()
            .and_then(|value| {
                deepgram::normalize_language(&self.model, Some(value))
                    .ok()
                    .flatten()
            })
            .and_then(|language| {
                deepgram::languages_for_model(&self.model)
                    .iter()
                    .find(|option| option.code.eq_ignore_ascii_case(&language))
                    .map(deepgram::language_display)
            })
            .unwrap_or_else(|| deepgram::DO_NOT_SPECIFY_LANGUAGE_LABEL.to_string());
        self.smart_format = config.smart_format;
        self.key_terms = format_key_terms_display(&config.key_terms);
        self.push_to_talk = config.hotkeys.push_to_talk.clone();
        self.keep_talking = config.hotkeys.keep_talking.clone();
        self.streaming = config.hotkeys.streaming.clone();
        self.resend_selected = config.hotkeys.resend_selected.clone();
        self.audio_input = config
            .audio_input
            .clone()
            .unwrap_or_else(|| DEFAULT_AUDIO_INPUT_LABEL.to_string());
        self.history_limit = config.history_limit.to_string();
        self.output_mode = config.output_mode.as_label().to_string();
        self.append_newline = config.append_newline;
        self.refresh_audio_inputs();
        self.restart_meter();
    }

    fn restart_meter(&mut self) {
        if self.launch_mode == LaunchMode::Settings && self.window_visible {
            self.meter = AudioMeter::new(self.selected_audio_input());
        } else {
            self.meter = None;
            self.meter_level = 0;
        }
        self.update_meter_label();
    }

    fn update_meter_label(&mut self) {
        self.meter_label = format_meter_text(self.meter_level);
    }

    fn handle_command(&mut self, command: UiCommand) -> Task<Message> {
        match command {
            UiCommand::ShowSettings(app) => {
                self.launch_mode = LaunchMode::Settings;
                self.app = Some(app);
                self.completion = None;
                self.reload_from_disk();
                self.show_or_open_window()
            }
            UiCommand::PromptForApiKey(completion) => {
                if let Some(previous) = self.completion.replace(completion) {
                    let _ = previous.send(None);
                }
                self.launch_mode = LaunchMode::ApiKey;
                self.app = None;
                self.reload_from_disk();
                self.show_or_open_window()
            }
        }
    }

    fn show_or_open_window(&mut self) -> Task<Message> {
        self.window_visible = true;
        self.restart_meter();

        if let Some(id) = self.current_window {
            return Task::batch([
                window::set_mode(id, window::Mode::Windowed),
                window::minimize(id, false),
                window::gain_focus(id),
            ]);
        }

        let settings = window::Settings {
            size: Size::new(760.0, 920.0),
            min_size: Some(Size::new(680.0, 780.0)),
            position: window::Position::Centered,
            exit_on_close_request: false,
            ..Default::default()
        };
        let (id, open) = window::open(settings);
        self.current_window = Some(id);
        open.map(Message::WindowOpened)
    }

    fn tick(&mut self) -> Task<Message> {
        let mut tasks = Vec::new();

        loop {
            match self.command_rx.try_recv() {
                Ok(command) => tasks.push(self.handle_command(command)),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => break,
            }
        }

        self.meter_level = self
            .meter
            .as_mut()
            .map(|meter| meter.sample_level())
            .unwrap_or(0);
        self.update_meter_label();

        if self.window_visible
            && self.launch_mode == LaunchMode::Settings
            && self.last_config_change_check.elapsed() >= CONFIG_CHECK_INTERVAL
        {
            let current_write_time = std::fs::metadata(config::config_path())
                .ok()
                .and_then(|metadata| metadata.modified().ok());

            self.config_changed_externally = current_write_time.is_some()
                && current_write_time != self.last_loaded_config_write_time;
            self.last_config_change_check = Instant::now();
        }

        Task::batch(tasks)
    }

    fn save(&mut self) -> Task<Message> {
        let Some(mut updated) = self.build_config() else {
            return Task::none();
        };

        if let Err(error) = updated.normalize() {
            self.status = error;
            return Task::none();
        }

        if let Err(error) = validate_hotkeys(&updated) {
            self.status = error;
            return Task::none();
        }

        if self.launch_mode == LaunchMode::ApiKey
            && updated
                .api_key
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
        {
            self.status = "API key is required to continue".to_string();
            return Task::none();
        }

        if let Some(app) = &self.app {
            match app.request_config_save(updated.clone()) {
                Ok(modified_at) => {
                    self.last_loaded_config_write_time = modified_at;
                    self.config_changed_externally = false;
                }
                Err(error) => {
                    self.status = error;
                    return Task::none();
                }
            }
        } else {
            if let Err(error) = config::save(&updated) {
                self.status = error;
                return Task::none();
            }
            if let Err(error) = config::ensure_backup(&updated) {
                self.status = error;
                return Task::none();
            }

            self.last_loaded_config_write_time = std::fs::metadata(config::config_path())
                .ok()
                .and_then(|meta| meta.modified().ok());
            self.config_changed_externally = false;
        }

        self.status = "Settings saved".to_string();

        if self.launch_mode == LaunchMode::ApiKey {
            if let Some(tx) = self.completion.take() {
                let _ = tx.send(updated.api_key.clone());
            }
            if let Some(id) = self.current_window {
                return window::close(id);
            }
        }

        Task::none()
    }

    fn build_config(&mut self) -> Option<Config> {
        let history_limit = match self.history_limit.trim().parse::<usize>() {
            Ok(value) if value > 0 => value,
            _ => {
                self.status = "History limit must be a positive number".to_string();
                return None;
            }
        };

        let output_mode = match self.output_mode.as_str() {
            "Copy to clipboard" => OutputMode::Clipboard,
            "Paste clipboard" => OutputMode::Paste,
            _ => OutputMode::DirectInput,
        };

        Some(Config {
            api_key: (!self.api_key.trim().is_empty()).then(|| self.api_key.trim().to_string()),
            smart_format: self.smart_format,
            model: self.model.clone(),
            language: deepgram::language_code_from_display(&self.model, &self.language),
            key_terms: parse_key_terms_text(&self.key_terms),
            hotkeys: config::HotkeyConfig {
                push_to_talk: self.push_to_talk.trim().to_string(),
                keep_talking: self.keep_talking.trim().to_string(),
                streaming: self.streaming.trim().to_string(),
                resend_selected: self.resend_selected.trim().to_string(),
            },
            audio_input: self.selected_audio_input().map(str::to_string),
            history_limit,
            output_mode,
            append_newline: self.append_newline,
            vad_silence_ms: self
                .app
                .as_ref()
                .map(|app| app.config().vad_silence_ms)
                .unwrap_or_else(|| config::Config::default().vad_silence_ms),
        })
    }
}

fn update(state: &mut SettingsWindow, message: Message) -> Task<Message> {
    match message {
        Message::ApiKeyChanged(value) => state.api_key = value,
        Message::ModelSelected(value) => {
            state.model = value;
            state.language_options = language_options_for_model(&state.model);
            let selected = deepgram::language_code_from_display(&state.model, &state.language);
            state.language = selected
                .and_then(|language| {
                    deepgram::languages_for_model(&state.model)
                        .iter()
                        .find(|option| option.code.eq_ignore_ascii_case(&language))
                        .map(deepgram::language_display)
                })
                .unwrap_or_else(|| deepgram::DO_NOT_SPECIFY_LANGUAGE_LABEL.to_string());
        }
        Message::LanguageSelected(value) => state.language = value,
        Message::SmartFormatToggled(value) => state.smart_format = value,
        Message::KeyTermsChanged(value) => state.key_terms = value,
        Message::PushToTalkChanged(value) => state.push_to_talk = value,
        Message::KeepTalkingChanged(value) => state.keep_talking = value,
        Message::StreamingChanged(value) => state.streaming = value,
        Message::ResendChanged(value) => state.resend_selected = value,
        Message::AudioInputSelected(value) => {
            state.audio_input = value;
            state.restart_meter();
        }
        Message::HistoryLimitChanged(value) => state.history_limit = value,
        Message::OutputModeSelected(value) => state.output_mode = value,
        Message::AppendNewlineToggled(value) => state.append_newline = value,
        Message::ReloadPressed => state.reload_from_disk(),
        Message::SavePressed => return state.save(),
        Message::KeyboardEvent(event) => {
            if is_save_shortcut(&event) {
                return state.save();
            }
        }
        Message::CloseRequested(id) => {
            if state.launch_mode == LaunchMode::ApiKey {
                if let Some(tx) = state.completion.take() {
                    let _ = tx.send(None);
                }
                state.window_visible = false;
                state.restart_meter();
                state.current_window = None;
                return window::close(id);
            }

            state.window_visible = false;
            state.restart_meter();
            return Task::batch([
                window::set_mode(id, window::Mode::Hidden),
                window::minimize(id, true),
            ]);
        }
        Message::WindowOpened(id) => {
            state.current_window = Some(id);
            state.window_visible = true;
            state.restart_meter();
            return Task::batch([
                window::set_mode(id, window::Mode::Windowed),
                window::minimize(id, false),
                window::gain_focus(id),
            ]);
        }
        Message::WindowClosed(id) => {
            if state.current_window == Some(id) {
                state.current_window = None;
            }
            state.window_visible = false;
            state.restart_meter();
        }
        Message::Tick => return state.tick(),
    }

    Task::none()
}

fn subscription(_state: &SettingsWindow) -> Subscription<Message> {
    Subscription::batch([
        iced::time::every(UI_TICK_INTERVAL).map(|_| Message::Tick),
        keyboard::listen().map(Message::KeyboardEvent),
        window::close_requests().map(Message::CloseRequested),
        window::close_events().map(Message::WindowClosed),
    ])
}

fn view(state: &SettingsWindow, _window: window::Id) -> Element<'_, Message> {
    let transcription = section(
        "Transcription",
        column![
            text_input("Deepgram API key", &state.api_key)
                .on_input(Message::ApiKeyChanged)
                .secure(true)
                .font(body_font())
                .padding(14)
                .size(15)
                .style(input_style),
            pick_list(
                model_options(),
                Some(state.model.clone()),
                Message::ModelSelected
            )
            .font(body_font())
            .padding(14)
            .text_size(15)
            .style(pick_list_style)
            .menu_style(pick_list_menu_style),
            pick_list(
                state.language_options.clone(),
                Some(state.language.clone()),
                Message::LanguageSelected
            )
            .font(body_font())
            .padding(14)
            .text_size(15)
            .style(pick_list_style)
            .menu_style(pick_list_menu_style),
            checkbox(state.smart_format)
                .label("Enable smart formatting")
                .on_toggle(Message::SmartFormatToggled)
                .spacing(12)
                .size(20)
                .text_size(15)
                .font(body_font())
                .style(checkbox_style),
            text_input("Recognition key terms, comma-separated", &state.key_terms)
                .on_input(Message::KeyTermsChanged)
                .font(body_font())
                .padding(14)
                .size(15)
                .style(input_style),
        ]
        .spacing(SECTION_SPACING),
    );

    let hotkeys = section(
        "Hotkeys",
        column![
            text_input("Push to talk", &state.push_to_talk)
                .on_input(Message::PushToTalkChanged)
                .font(body_font())
                .padding(14)
                .size(15)
                .style(input_style),
            text_input("Keep talking", &state.keep_talking)
                .on_input(Message::KeepTalkingChanged)
                .font(body_font())
                .padding(14)
                .size(15)
                .style(input_style),
            text_input("Streaming", &state.streaming)
                .on_input(Message::StreamingChanged)
                .font(body_font())
                .padding(14)
                .size(15)
                .style(input_style),
            text_input("Resend selected transcript", &state.resend_selected)
                .on_input(Message::ResendChanged)
                .font(body_font())
                .padding(14)
                .size(15)
                .style(input_style),
        ]
        .spacing(SECTION_SPACING),
    );

    let audio_output = section(
        "Audio And Output",
        column![
            pick_list(
                state.audio_inputs.clone(),
                Some(state.audio_input.clone()),
                Message::AudioInputSelected
            )
            .font(body_font())
            .padding(14)
            .text_size(15)
            .style(pick_list_style)
            .menu_style(pick_list_menu_style),
            progress_bar(0.0..=100.0, state.meter_level as f32).style(meter_style),
            text(&state.meter_label)
                .size(14)
                .font(body_font())
                .color(secondary_text()),
            pick_list(
                output_mode_options(),
                Some(state.output_mode.clone()),
                Message::OutputModeSelected
            )
            .font(body_font())
            .padding(14)
            .text_size(15)
            .style(pick_list_style)
            .menu_style(pick_list_menu_style),
            text_input("Recent history limit", &state.history_limit)
                .on_input(Message::HistoryLimitChanged)
                .font(body_font())
                .padding(14)
                .size(15)
                .style(input_style),
            checkbox(state.append_newline)
                .label("Append newline after transcript")
                .on_toggle(Message::AppendNewlineToggled)
                .spacing(12)
                .size(20)
                .text_size(15)
                .font(body_font())
                .style(checkbox_style),
        ]
        .spacing(SECTION_SPACING),
    );

    let mut status_column = column![
        text(state.launch_mode.subtitle())
            .size(13)
            .font(label_font())
            .color(muted_text()),
        text(state.runtime_status())
            .size(15)
            .font(body_font())
            .color(secondary_text()),
    ]
    .spacing(10);

    if state.config_changed_externally {
        status_column = status_column.push(
            text(
                "The configuration file changed on disk. Reload before saving to avoid overwriting newer values."
            )
            .size(14)
            .font(body_font())
            .color(primary_text()),
        );
    }

    if !state.status.is_empty() {
        status_column = status_column.push(
            text(&state.status)
                .size(14)
                .font(body_font())
                .color(subtle_status_color(&state.status)),
        );
    }

    let status = container(status_column.spacing(10))
        .padding(18)
        .style(status_panel_style(state.config_changed_externally));

    let actions = row![
        button(text("Reload").font(label_font()).size(15))
            .padding(14)
            .style(secondary_button_style)
            .on_press(Message::ReloadPressed),
        button(text("Save").font(label_font()).size(15))
            .padding(14)
            .style(primary_button_style)
            .on_press(Message::SavePressed),
    ]
    .spacing(12);

    let content = column![
        text("DEEPGRAM // VELOCITY")
            .size(12)
            .font(label_font())
            .color(muted_text()),
        text(state.window_title())
            .size(40)
            .font(heading_font())
            .color(primary_text()),
        text(state.launch_mode.subtitle())
            .size(16)
            .font(body_font())
            .color(secondary_text()),
        transcription,
        hotkeys,
        audio_output,
        section("Status", status),
        actions,
    ]
    .spacing(24)
    .padding(28)
    .max_width(720);

    container(
        container(scrollable(content))
            .max_width(760)
            .style(panel_style()),
    )
    .padding(18)
    .style(page_style())
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn section<'a>(title: &'a str, content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(
        column![
            text(title).size(13).font(label_font()).color(muted_text()),
            content.into()
        ]
        .spacing(16),
    )
    .padding(20)
    .style(panel_style())
    .width(Length::Fill)
    .into()
}

fn validate_hotkeys(config: &Config) -> Result<(), String> {
    hotkey::parse_hotkey(&config.hotkeys.push_to_talk)?;
    hotkey::parse_hotkey(&config.hotkeys.keep_talking)?;
    hotkey::parse_hotkey(&config.hotkeys.streaming)?;
    hotkey::parse_hotkey(&config.hotkeys.resend_selected)?;
    Ok(())
}

fn is_save_shortcut(event: &keyboard::Event) -> bool {
    match event {
        keyboard::Event::KeyPressed { key, modifiers, .. } if modifiers.command() => {
            matches!(key.as_ref(), keyboard::Key::Character("s" | "S"))
        }
        _ => false,
    }
}

fn format_meter_text(level: u8) -> String {
    let filled = (level / 10).min(10);
    let empty = 10 - filled;
    format!(
        "Mic activity: [{}{}] {}%",
        "|".repeat(filled as usize),
        " ".repeat(empty as usize),
        level
    )
}

fn format_key_terms_display(key_terms: &[String]) -> String {
    key_terms
        .iter()
        .map(|term| term.trim())
        .filter(|term| !term.is_empty())
        .collect::<Vec<_>>()
        .join(", ")
}

fn parse_key_terms_text(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|term| term.trim())
        .filter(|term| !term.is_empty())
        .map(|term| term.to_string())
        .collect()
}

fn language_options_for_model(model: &str) -> Vec<String> {
    let mut options = vec![deepgram::DO_NOT_SPECIFY_LANGUAGE_LABEL.to_string()];
    options.extend(
        deepgram::languages_for_model(model)
            .iter()
            .map(deepgram::language_display),
    );
    options
}

fn output_mode_options() -> Vec<String> {
    OutputMode::all()
        .into_iter()
        .map(|mode| mode.as_label().to_string())
        .collect()
}

fn model_options() -> Vec<String> {
    deepgram::supported_models()
        .iter()
        .map(|model| (*model).to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_key_terms_display_uses_commas() {
        let formatted = format_key_terms_display(&[
            "PowerShell".to_string(),
            " Deepgram ".to_string(),
            String::new(),
        ]);

        assert_eq!(formatted, "PowerShell, Deepgram");
    }

    #[test]
    fn parse_key_terms_text_splits_on_commas() {
        let parsed = parse_key_terms_text("PowerShell, Deepgram , Kubernetes,,");

        assert_eq!(parsed, vec!["PowerShell", "Deepgram", "Kubernetes"]);
    }
}
