use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};
use std::{panic, sync::mpsc};

use anyhow::Result;
use gpui::{
    App, Bounds, Context, Entity, IntoElement, KeyBinding, ParentElement as _, Render,
    Subscription, Window, WindowBounds, WindowOptions, actions, div, prelude::*, px, size,
};
use gpui_component::{
    ActiveTheme, Root, Sizable, Size,
    button::{Button, ButtonVariants as _},
    form::{field, v_form},
    group_box::{GroupBox, GroupBoxVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    progress::Progress,
    scroll::ScrollableElement as _,
    select::{Select, SelectEvent, SelectState},
    switch::Switch,
    v_flex,
};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, SW_RESTORE, SetForegroundWindow, ShowWindow,
};
use windows::core::PCWSTR;

use crate::audio::{self, AudioMeter};
use crate::config::{self, Config, OutputMode};
use crate::deepgram;
use crate::hotkey;
use crate::logger;
use crate::state;

actions!(velocity_settings, [SaveSettings]);

const DEFAULT_AUDIO_INPUT_LABEL: &str = "Default system input";
const SETTINGS_TITLE: &str = "Velocity Settings";
const API_KEY_TITLE: &str = "Velocity API Key";
const UI_TICK_INTERVAL: Duration = Duration::from_millis(50);
const CONFIG_CHECK_INTERVAL: Duration = Duration::from_secs(1);
const KEY_CONTEXT: &str = "VelocitySettings";

static SETTINGS_WINDOW_OPEN: AtomicBool = AtomicBool::new(false);
static API_KEY_WINDOW_OPEN: AtomicBool = AtomicBool::new(false);

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

struct CompletionHandle(Arc<Mutex<Option<mpsc::Sender<Option<String>>>>>);

impl CompletionHandle {
    fn new(sender: mpsc::Sender<Option<String>>) -> Self {
        Self(Arc::new(Mutex::new(Some(sender))))
    }

    fn send(&self, value: Option<String>) {
        if let Some(tx) = self.0.lock().unwrap().take() {
            let _ = tx.send(value);
        }
    }
}

impl Clone for CompletionHandle {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

pub fn prompt_for_api_key() -> Option<String> {
    if API_KEY_WINDOW_OPEN.swap(true, Ordering::SeqCst) {
        focus_existing_window(API_KEY_TITLE);
        return None;
    }

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let completion = CompletionHandle::new(tx);
        run_window(LaunchMode::ApiKey, None, Some(completion.clone()));
        completion.send(None);
        API_KEY_WINDOW_OPEN.store(false, Ordering::SeqCst);
    });

    rx.recv().ok().flatten()
}

pub fn show_settings_window() {
    if SETTINGS_WINDOW_OPEN.swap(true, Ordering::SeqCst) {
        focus_existing_window(SETTINGS_TITLE);
        return;
    }

    let app = state::global();
    std::thread::spawn(move || {
        run_window(LaunchMode::Settings, Some(app), None);
        SETTINGS_WINDOW_OPEN.store(false, Ordering::SeqCst);
    });
}

fn run_window(
    launch_mode: LaunchMode,
    app: Option<Arc<state::AppState>>,
    completion: Option<CompletionHandle>,
) {
    let result = panic::catch_unwind(move || {
        let app_instance = gpui_platform::application();
        app_instance.run(move |cx| {
            gpui_component::init(cx);
            cx.bind_keys([KeyBinding::new("ctrl-s", SaveSettings, Some(KEY_CONTEXT))]);
            cx.on_window_closed(|cx, _window_id| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            let window_bounds = WindowBounds::Windowed(Bounds::centered(
                None,
                size(
                    px(if launch_mode == LaunchMode::Settings {
                        920.
                    } else {
                        760.
                    }),
                    px(if launch_mode == LaunchMode::Settings {
                        980.
                    } else {
                        380.
                    }),
                ),
                cx,
            ));

            cx.spawn(async move |cx| {
                let completion_for_window = completion.clone();
                let completion_for_error = completion.clone();
                let app_for_window = app.clone();
                let open_result = cx.open_window(
                    WindowOptions {
                        window_bounds: Some(window_bounds),
                        ..Default::default()
                    },
                    move |window, cx| {
                        let view = cx.new(|cx| {
                            SettingsView::new(
                                launch_mode,
                                app_for_window.clone(),
                                completion_for_window.clone(),
                                window,
                                cx,
                            )
                        });
                        cx.new(|cx| Root::new(view, window, cx).bg(cx.theme().background))
                    },
                );

                if let Err(error) = open_result {
                    logger::verbose(&format!("Failed to open GPUI settings window: {error}"));
                    if let Some(completion) = completion_for_error {
                        completion.send(None);
                    }
                    cx.update(|cx| cx.quit());
                }
            })
            .detach();
        });
    });

    if let Err(error) = result {
        logger::verbose(&format!("GPUI settings thread panicked: {error:?}"));
    }
}

fn focus_existing_window(title: &str) {
    let title = to_wide(title);
    unsafe {
        let Ok(hwnd) = FindWindowW(None, PCWSTR(title.as_ptr())) else {
            return;
        };
        let _ = ShowWindow(hwnd, SW_RESTORE);
        let _ = SetForegroundWindow(hwnd);
    }
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

struct SettingsView {
    launch_mode: LaunchMode,
    app: Option<Arc<state::AppState>>,
    completion: Option<CompletionHandle>,
    api_key_input: Entity<InputState>,
    model_select: Entity<SelectState<Vec<String>>>,
    language_select: Entity<SelectState<Vec<String>>>,
    key_terms_input: Entity<InputState>,
    push_to_talk_input: Entity<InputState>,
    keep_talking_input: Entity<InputState>,
    streaming_input: Entity<InputState>,
    resend_selected_input: Entity<InputState>,
    audio_input_select: Entity<SelectState<Vec<String>>>,
    history_limit_input: Entity<InputState>,
    output_mode_select: Entity<SelectState<Vec<String>>>,
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
    audio_inputs: Vec<String>,
    language_options: Vec<String>,
    meter: Option<AudioMeter>,
    meter_level: u8,
    meter_label: String,
    status: String,
    config_changed_externally: bool,
    last_loaded_config_write_time: Option<SystemTime>,
    last_config_change_check: Instant,
    _subscriptions: Vec<Subscription>,
}

impl SettingsView {
    fn new(
        launch_mode: LaunchMode,
        app: Option<Arc<state::AppState>>,
        completion: Option<CompletionHandle>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        window.set_window_title(launch_mode.title());

        let model_items = model_options();
        let language_items = vec![deepgram::DO_NOT_SPECIFY_LANGUAGE_LABEL.to_string()];
        let audio_inputs = vec![DEFAULT_AUDIO_INPUT_LABEL.to_string()];
        let output_modes = output_mode_options();

        let api_key_input = cx.new(|cx| {
            InputState::new(window, cx)
                .masked(true)
                .placeholder("Deepgram API key")
        });
        let key_terms_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Recognition key terms, comma-separated")
        });
        let push_to_talk_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Push to talk"));
        let keep_talking_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Keep talking"));
        let streaming_input = cx.new(|cx| InputState::new(window, cx).placeholder("Streaming"));
        let resend_selected_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Resend selected transcript"));
        let history_limit_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Recent history limit"));

        let model_select = cx.new(|cx| {
            SelectState::new(
                model_items.clone(),
                index_path_for(&deepgram::DEFAULT_MODEL.to_string(), &model_items),
                window,
                cx,
            )
        });
        let language_select = cx.new(|cx| {
            SelectState::new(
                language_items.clone(),
                index_path_for(
                    &deepgram::DO_NOT_SPECIFY_LANGUAGE_LABEL.to_string(),
                    &language_items,
                ),
                window,
                cx,
            )
        });
        let audio_input_select = cx.new(|cx| {
            SelectState::new(
                audio_inputs.clone(),
                index_path_for(&DEFAULT_AUDIO_INPUT_LABEL.to_string(), &audio_inputs),
                window,
                cx,
            )
        });
        let output_mode_select = cx.new(|cx| {
            SelectState::new(
                output_modes.clone(),
                index_path_for(
                    &OutputMode::DirectInput.as_label().to_string(),
                    &output_modes,
                ),
                window,
                cx,
            )
        });

        let mut view = Self {
            launch_mode,
            app,
            completion,
            api_key_input,
            model_select,
            language_select,
            key_terms_input,
            push_to_talk_input,
            keep_talking_input,
            streaming_input,
            resend_selected_input,
            audio_input_select,
            history_limit_input,
            output_mode_select,
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
            audio_inputs,
            language_options: language_items,
            meter: None,
            meter_level: 0,
            meter_label: "Mic activity: unavailable".to_string(),
            status: String::new(),
            config_changed_externally: false,
            last_loaded_config_write_time: None,
            last_config_change_check: Instant::now(),
            _subscriptions: Vec::new(),
        };

        view.install_subscriptions(window, cx);
        view.reload_from_disk(window, cx);
        view.start_tick_loop(cx);
        view
    }

    fn install_subscriptions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self._subscriptions = vec![
            subscribe_input_string(cx, window, &self.api_key_input, |this, value, _, cx| {
                this.api_key = value;
                cx.notify();
            }),
            subscribe_input_string(cx, window, &self.key_terms_input, |this, value, _, cx| {
                this.key_terms = value;
                cx.notify();
            }),
            subscribe_input_string(
                cx,
                window,
                &self.push_to_talk_input,
                |this, value, _, cx| {
                    this.push_to_talk = value;
                    cx.notify();
                },
            ),
            subscribe_input_string(
                cx,
                window,
                &self.keep_talking_input,
                |this, value, _, cx| {
                    this.keep_talking = value;
                    cx.notify();
                },
            ),
            subscribe_input_string(cx, window, &self.streaming_input, |this, value, _, cx| {
                this.streaming = value;
                cx.notify();
            }),
            subscribe_input_string(
                cx,
                window,
                &self.resend_selected_input,
                |this, value, _, cx| {
                    this.resend_selected = value;
                    cx.notify();
                },
            ),
            subscribe_input_string(
                cx,
                window,
                &self.history_limit_input,
                |this, value, _, cx| {
                    this.history_limit = value;
                    cx.notify();
                },
            ),
            subscribe_select_string(cx, window, &self.model_select, |this, value, window, cx| {
                this.model = value;
                this.refresh_language_options(window, cx);
                cx.notify();
            }),
            subscribe_select_string(cx, window, &self.language_select, |this, value, _, cx| {
                this.language = value;
                cx.notify();
            }),
            subscribe_select_string(
                cx,
                window,
                &self.audio_input_select,
                |this, value, _, cx| {
                    this.audio_input = value;
                    this.restart_meter();
                    cx.notify();
                },
            ),
            subscribe_select_string(
                cx,
                window,
                &self.output_mode_select,
                |this, value, _, cx| {
                    this.output_mode = value;
                    cx.notify();
                },
            ),
        ];
    }

    fn start_tick_loop(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(UI_TICK_INTERVAL).await;
                let result = this.update(cx, |this, cx| {
                    this.tick(cx);
                    cx.notify();
                });
                if result.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        self.meter_level = self
            .meter
            .as_mut()
            .map(|meter| meter.sample_level())
            .unwrap_or(0);
        self.meter_label = format_meter_text(self.meter_level);

        if self.launch_mode == LaunchMode::Settings
            && self.last_config_change_check.elapsed() >= CONFIG_CHECK_INTERVAL
        {
            let current_write_time = std::fs::metadata(config::config_path())
                .ok()
                .and_then(|metadata| metadata.modified().ok());

            self.config_changed_externally = current_write_time.is_some()
                && current_write_time != self.last_loaded_config_write_time;
            self.last_config_change_check = Instant::now();
        }

        let _ = cx;
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

    fn refresh_audio_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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

        self.audio_inputs = inputs.clone();
        self.audio_input_select.update(cx, |select, cx| {
            select.set_items(inputs, window, cx);
            let current = self.audio_input.clone();
            select.set_selected_value(&current, window, cx);
        });
    }

    fn reload_from_disk(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match config::load_state() {
            Ok(loaded) => {
                self.apply_config(&loaded.config, loaded.modified_at, window, cx);
                self.status = format!("Loaded {}", config::config_path().display());
            }
            Err(error) => {
                self.status = error;
            }
        }
        cx.notify();
    }

    fn apply_config(
        &mut self,
        config: &Config,
        modified_at: Option<SystemTime>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.api_key = config.api_key.clone().unwrap_or_default();
        self.model = config.model.clone();
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
        self.last_loaded_config_write_time = modified_at;
        self.config_changed_externally = false;
        self.last_config_change_check = Instant::now();

        self.api_key_input.update(cx, |input, cx| {
            input.set_value(self.api_key.clone(), window, cx)
        });
        self.key_terms_input.update(cx, |input, cx| {
            input.set_value(self.key_terms.clone(), window, cx)
        });
        self.push_to_talk_input.update(cx, |input, cx| {
            input.set_value(self.push_to_talk.clone(), window, cx)
        });
        self.keep_talking_input.update(cx, |input, cx| {
            input.set_value(self.keep_talking.clone(), window, cx)
        });
        self.streaming_input.update(cx, |input, cx| {
            input.set_value(self.streaming.clone(), window, cx)
        });
        self.resend_selected_input.update(cx, |input, cx| {
            input.set_value(self.resend_selected.clone(), window, cx)
        });
        self.history_limit_input.update(cx, |input, cx| {
            input.set_value(self.history_limit.clone(), window, cx)
        });

        let models = model_options();
        self.model_select.update(cx, |select, cx| {
            select.set_items(models, window, cx);
            let model = self.model.clone();
            select.set_selected_value(&model, window, cx);
        });

        self.refresh_language_options(window, cx);
        self.refresh_audio_inputs(window, cx);

        let output_modes = output_mode_options();
        self.output_mode_select.update(cx, |select, cx| {
            select.set_items(output_modes, window, cx);
            let output_mode = self.output_mode.clone();
            select.set_selected_value(&output_mode, window, cx);
        });

        self.restart_meter();
        self.meter_label = format_meter_text(self.meter_level);
    }

    fn refresh_language_options(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.language_options = language_options_for_model(&self.model);

        let selected = deepgram::language_code_from_display(&self.model, &self.language);
        self.language = selected
            .and_then(|language| {
                deepgram::languages_for_model(&self.model)
                    .iter()
                    .find(|option| option.code.eq_ignore_ascii_case(&language))
                    .map(deepgram::language_display)
            })
            .unwrap_or_else(|| deepgram::DO_NOT_SPECIFY_LANGUAGE_LABEL.to_string());

        let language_options = self.language_options.clone();
        self.language_select.update(cx, |select, cx| {
            select.set_items(language_options, window, cx);
            let selected_language = self.language.clone();
            select.set_selected_value(&selected_language, window, cx);
        });
    }

    fn restart_meter(&mut self) {
        if self.launch_mode == LaunchMode::Settings {
            self.meter = AudioMeter::new(self.selected_audio_input());
        } else {
            self.meter = None;
            self.meter_level = 0;
        }
        self.meter_label = format_meter_text(self.meter_level);
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(mut updated) = self.build_config() else {
            cx.notify();
            return;
        };

        if let Err(error) = updated.normalize() {
            self.status = error;
            cx.notify();
            return;
        }

        if let Err(error) = validate_hotkeys(&updated) {
            self.status = error;
            cx.notify();
            return;
        }

        if self.launch_mode == LaunchMode::ApiKey
            && updated
                .api_key
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
        {
            self.status = "API key is required to continue".to_string();
            cx.notify();
            return;
        }

        if let Some(app) = &self.app {
            match app.request_config_save(updated.clone()) {
                Ok(modified_at) => {
                    self.last_loaded_config_write_time = modified_at;
                    self.config_changed_externally = false;
                }
                Err(error) => {
                    self.status = error;
                    cx.notify();
                    return;
                }
            }
        } else {
            if let Err(error) = config::save(&updated) {
                self.status = error;
                cx.notify();
                return;
            }
            if let Err(error) = config::ensure_backup(&updated) {
                self.status = error;
                cx.notify();
                return;
            }

            self.last_loaded_config_write_time = std::fs::metadata(config::config_path())
                .ok()
                .and_then(|meta| meta.modified().ok());
            self.config_changed_externally = false;
        }

        self.status = "Settings saved".to_string();
        if self.launch_mode == LaunchMode::ApiKey {
            if let Some(completion) = &self.completion {
                completion.send(updated.api_key.clone());
            }
            window.remove_window();
        }
        cx.notify();
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

    fn on_save_action(&mut self, _: &SaveSettings, window: &mut Window, cx: &mut Context<Self>) {
        self.save(window, cx);
    }

    fn on_reload_click(
        &mut self,
        _: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.reload_from_disk(window, cx);
    }

    fn on_save_click(&mut self, _: &gpui::ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.save(window, cx);
    }

    fn on_smart_format_click(&mut self, checked: &bool, _: &mut Window, cx: &mut Context<Self>) {
        self.smart_format = *checked;
        cx.notify();
    }

    fn on_append_newline_click(&mut self, checked: &bool, _: &mut Window, cx: &mut Context<Self>) {
        self.append_newline = *checked;
        cx.notify();
    }

    fn render_api_key_prompt(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .gap_4()
            .w_full()
            .child(
                GroupBox::new().outline().title("Configuration").child(
                    v_form().with_size(Size::Large).child(
                        field()
                            .label("Deepgram API key")
                            .child(Input::new(&self.api_key_input).w_full()),
                    ),
                ),
            )
            .child(self.render_status_box(cx))
            .child(
                h_flex().justify_end().gap_3().child(
                    Button::new("save-api-key")
                        .primary()
                        .label("Save")
                        .on_click(cx.listener(Self::on_save_click)),
                ),
            )
            .map(move |this| {
                this.on_action(cx.listener(Self::on_save_action))
                    .key_context(KEY_CONTEXT)
                    .size_full()
                    .p_6()
                    .bg(cx.theme().background)
                    .text_color(cx.theme().foreground)
            })
    }

    fn render_settings(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let transcription = GroupBox::new().outline().title("Transcription").child(
            v_form()
                .with_size(Size::Large)
                .child(
                    field()
                        .label("Deepgram API key")
                        .child(Input::new(&self.api_key_input).w_full()),
                )
                .child(
                    field()
                        .label("Model")
                        .child(Select::new(&self.model_select).w_full()),
                )
                .child(
                    field()
                        .label("Language")
                        .child(Select::new(&self.language_select).w_full()),
                )
                .child(
                    field().label("Smart format").child(
                        Switch::new("smart-format")
                            .checked(self.smart_format)
                            .label("Enable smart formatting")
                            .on_click(cx.listener(Self::on_smart_format_click)),
                    ),
                )
                .child(
                    field()
                        .label("Key terms")
                        .child(Input::new(&self.key_terms_input).w_full()),
                ),
        );

        let hotkeys = GroupBox::new().outline().title("Hotkeys").child(
            v_form()
                .with_size(Size::Large)
                .child(
                    field()
                        .label("Push to talk")
                        .child(Input::new(&self.push_to_talk_input).w_full()),
                )
                .child(
                    field()
                        .label("Keep talking")
                        .child(Input::new(&self.keep_talking_input).w_full()),
                )
                .child(
                    field()
                        .label("Streaming")
                        .child(Input::new(&self.streaming_input).w_full()),
                )
                .child(
                    field()
                        .label("Resend selected")
                        .child(Input::new(&self.resend_selected_input).w_full()),
                ),
        );

        let audio_output = GroupBox::new().outline().title("Audio and output").child(
            v_form()
                .with_size(Size::Large)
                .child(
                    field()
                        .label("Microphone")
                        .child(Select::new(&self.audio_input_select).w_full()),
                )
                .child(
                    field().label("Mic activity").child(
                        v_flex()
                            .w_full()
                            .gap_2()
                            .child(Progress::new("mic-meter").value(self.meter_level as f32))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(self.meter_label.clone()),
                            ),
                    ),
                )
                .child(
                    field()
                        .label("Output mode")
                        .child(Select::new(&self.output_mode_select).w_full()),
                )
                .child(
                    field()
                        .label("History limit")
                        .child(Input::new(&self.history_limit_input).w_full()),
                )
                .child(
                    field().label("Append newline").child(
                        Switch::new("append-newline")
                            .checked(self.append_newline)
                            .label("Append newline after transcript")
                            .on_click(cx.listener(Self::on_append_newline_click)),
                    ),
                ),
        );

        let actions = h_flex()
            .justify_end()
            .gap_3()
            .child(
                Button::new("reload")
                    .outline()
                    .label("Reload")
                    .on_click(cx.listener(Self::on_reload_click)),
            )
            .child(
                Button::new("save")
                    .primary()
                    .label("Save")
                    .on_click(cx.listener(Self::on_save_click)),
            );

        div()
            .on_action(cx.listener(Self::on_save_action))
            .key_context(KEY_CONTEXT)
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .overflow_y_scrollbar()
            .child(
                v_flex()
                    .gap_6()
                    .p_6()
                    .w_full()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("DEEPGRAM // VELOCITY"),
                            )
                            .child(
                                div()
                                    .text_3xl()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .child(self.launch_mode.title()),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(self.launch_mode.subtitle()),
                            ),
                    )
                    .child(transcription)
                    .child(hotkeys)
                    .child(audio_output)
                    .child(self.render_status_box(cx))
                    .child(actions),
            )
    }

    fn render_status_box(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut status = v_flex()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.launch_mode.subtitle()),
            )
            .child(div().text_sm().child(self.runtime_status()));

        if self.config_changed_externally {
            status = status.child(
                div()
                    .text_sm()
                    .text_color(cx.theme().warning)
                    .child(
                        "The configuration file changed on disk. Reload before saving to avoid overwriting newer values.",
                    ),
            );
        }

        if !self.status.is_empty() {
            status = status.child(
                div()
                    .text_sm()
                    .text_color(status_color(&self.status, cx))
                    .child(self.status.clone()),
            );
        }

        GroupBox::new().outline().title("Status").child(status)
    }
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.launch_mode == LaunchMode::ApiKey {
            self.render_api_key_prompt(window, cx).into_any_element()
        } else {
            self.render_settings(window, cx).into_any_element()
        }
    }
}

fn subscribe_input_string(
    cx: &mut Context<SettingsView>,
    window: &mut Window,
    input: &Entity<InputState>,
    on_change: impl Fn(&mut SettingsView, String, &mut Window, &mut Context<SettingsView>) + 'static,
) -> Subscription {
    let input = input.clone();
    let observed_input = input.clone();
    cx.subscribe_in(
        &input,
        window,
        move |this, _, event: &InputEvent, window, cx| {
            if matches!(event, InputEvent::Change) {
                on_change(this, observed_input.read(cx).value().to_string(), window, cx);
            }
        },
    )
}

fn subscribe_select_string(
    cx: &mut Context<SettingsView>,
    window: &mut Window,
    select: &Entity<SelectState<Vec<String>>>,
    on_change: impl Fn(&mut SettingsView, String, &mut Window, &mut Context<SettingsView>) + 'static,
) -> Subscription {
    let select = select.clone();
    cx.subscribe_in(
        &select,
        window,
        move |this, _, event: &SelectEvent<Vec<String>>, window, cx| {
            let SelectEvent::Confirm(Some(value)) = event else {
                return;
            };
            on_change(this, value.to_string(), window, cx);
        },
    )
}

fn status_color(status: &str, cx: &App) -> gpui::Hsla {
    let lower = status.to_ascii_lowercase();
    if lower.contains("fail")
        || lower.contains("error")
        || lower.contains("required")
        || lower.contains("reject")
        || lower.contains("invalid")
    {
        cx.theme().danger
    } else if lower.contains("saved") || lower.contains("loaded") {
        cx.theme().success
    } else {
        cx.theme().muted_foreground
    }
}

fn index_path_for(value: &String, items: &[String]) -> Option<gpui_component::IndexPath> {
    items
        .iter()
        .position(|candidate| candidate == value)
        .map(|row| gpui_component::IndexPath::default().row(row))
}

fn validate_hotkeys(config: &Config) -> Result<(), String> {
    hotkey::parse_hotkey(&config.hotkeys.push_to_talk)?;
    hotkey::parse_hotkey(&config.hotkeys.keep_talking)?;
    hotkey::parse_hotkey(&config.hotkeys.streaming)?;
    hotkey::parse_hotkey(&config.hotkeys.resend_selected)?;
    Ok(())
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
