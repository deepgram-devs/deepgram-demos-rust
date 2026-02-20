use ratatui::widgets::TableState;
use ratatui::widgets::ListState;
use ratatui::layout::Rect;
use directories::ProjectDirs;
use anyhow::Result;
use lazy_static::lazy_static;
use std::collections::HashMap;
use rust_decimal::Decimal;
use std::str::FromStr;
use std::sync::Arc;
use rodio::OutputStream;
use tokio::sync::mpsc;
use crate::config::{self, AppConfig, ExperimentalFlags};
use crate::persistence;

#[derive(Clone, Debug, PartialEq)]
pub enum Gender {
    Male,
    Female,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Voice {
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub model: String,
    pub language: String,
    pub gender: Gender,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CurrentScreen {
    Main,
    Editing,
    Help,
    ApiKeyInput,
    VoiceFilter,
    SampleRateSelect,
    AudioFormatSelect,
}

/// Describes a Deepgram TTS audio encoding format with its constraints.
pub struct AudioFormat {
    pub encoding: &'static str,          // Deepgram API `encoding` parameter value
    pub display_name: &'static str,      // Human-readable name shown in the UI
    pub extension: &'static str,         // Cache file extension
    pub valid_sample_rates: &'static [u32],
    pub default_sample_rate: u32,
}

static MP3_RATES: [u32; 1]      = [22050];
static LINEAR16_RATES: [u32; 5] = [8000, 16000, 24000, 32000, 48000];
static MULAW_RATES: [u32; 2]    = [8000, 16000];
static ALAW_RATES: [u32; 2]     = [8000, 16000];
static FLAC_RATES: [u32; 5]     = [8000, 16000, 22050, 32000, 48000];
static AAC_RATES: [u32; 1]      = [22050];

pub static AUDIO_FORMATS: [AudioFormat; 6] = [
    AudioFormat { encoding: "mp3",      display_name: "MP3",           extension: "mp3",   valid_sample_rates: &MP3_RATES,      default_sample_rate: 22050 },
    AudioFormat { encoding: "linear16", display_name: "Linear16 (WAV)",extension: "wav",   valid_sample_rates: &LINEAR16_RATES, default_sample_rate: 24000 },
    AudioFormat { encoding: "mulaw",    display_name: "μ-law",         extension: "mulaw", valid_sample_rates: &MULAW_RATES,    default_sample_rate: 8000  },
    AudioFormat { encoding: "alaw",     display_name: "A-law",         extension: "alaw",  valid_sample_rates: &ALAW_RATES,     default_sample_rate: 8000  },
    AudioFormat { encoding: "flac",     display_name: "FLAC",          extension: "flac",  valid_sample_rates: &FLAC_RATES,     default_sample_rate: 8000  },
    AudioFormat { encoding: "aac",      display_name: "AAC",           extension: "aac",   valid_sample_rates: &AAC_RATES,      default_sample_rate: 22050 },
];

pub const DEFAULT_FORMAT_INDEX: usize = 0; // MP3

#[derive(Clone, Debug, PartialEq)]
pub enum CurrentlyEditing {
    Text,
    Voice,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LogLevel {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Clone, Debug)]
pub struct LogEntry {
    pub level: LogLevel,
    pub message: String,
    pub timestamp: chrono::DateTime<chrono::Local>,
}

pub struct App {
    pub config: AppConfig,
    pub current_screen: CurrentScreen,
    pub currently_editing: Option<CurrentlyEditing>,
    pub text_table_state: TableState,
    pub voice_menu_state: ListState,
    pub saved_texts: Vec<String>,
    pub voices: Vec<Voice>,
    pub audio_cache_dir: String,
    pub deepgram_endpoint: String,
    pub status_message: String,
    pub focused_panel: Panel,
    pub logs: Vec<LogEntry>,
    pub log_scroll_offset: usize,
    pub log_panel_bounds: Rect,
    pub input_buffer: String,
    pub api_key_override: Option<String>,
    pub api_key_input_buffer: String,
    pub voice_filter: String,
    pub voice_filter_buffer: String,
    pub audio_format_index: usize,
    pub audio_format_menu_state: ListState,
    pub sample_rate: u32,
    pub sample_rate_menu_state: ListState,
    pub playback_speed: Decimal,  // Range: 0.7 to 1.5
    pub is_loading: bool,
    pub loading_text: String,
    pub spinner_index: usize,
    pub audio_sink: Option<Arc<rodio::Sink>>,
    pub audio_stream: Option<Arc<OutputStream>>,
    pub tts_receiver: Option<mpsc::UnboundedReceiver<TtsResult>>,
    pub audio_duration_ms: u64,
    pub playback_start_time: std::time::Instant,
    pub audio_cache_info: std::collections::HashMap<String, bool>, // text -> is_cached
    pub help_scroll_offset: usize,
    pub text_panel_bounds: Rect,
    pub voice_panel_bounds: Rect,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Panel {
    TextList,
    VoiceMenu,
}

const SPINNER_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub enum TtsResult {
    Success {
        message: String,
        audio_data: Vec<u8>,
        is_cached: bool,
    },
    Error(String),
}

lazy_static! {
    static ref DEEPGRAM_VOICES: HashMap<&'static str, Vec<Voice>> = {
        let mut map = HashMap::new();
        // Deepgram Voices
        map.insert("Deepgram", vec![
            Voice { id: "aura-angus-en".to_string(), name: "Angus (aura)".to_string(), vendor: "Deepgram".to_string(), model: "aura".to_string(), language: "English".to_string(), gender: Gender::Male },
            Voice { id: "aura-asteria-en".to_string(), name: "Asteria (aura)".to_string(), vendor: "Deepgram".to_string(), model: "aura".to_string(), language: "English".to_string(), gender: Gender::Female },
            Voice { id: "aura-athena-en".to_string(), name: "Athena (aura)".to_string(), vendor: "Deepgram".to_string(), model: "aura".to_string(), language: "English".to_string(), gender: Gender::Female },
            Voice { id: "aura-helios-en".to_string(), name: "Helios (aura)".to_string(), vendor: "Deepgram".to_string(), model: "aura".to_string(), language: "English".to_string(), gender: Gender::Male },
            Voice { id: "aura-hera-en".to_string(), name: "Hera (aura)".to_string(), vendor: "Deepgram".to_string(), model: "aura".to_string(), language: "English".to_string(), gender: Gender::Female },
            Voice { id: "aura-luna-en".to_string(), name: "Luna (aura)".to_string(), vendor: "Deepgram".to_string(), model: "aura".to_string(), language: "English".to_string(), gender: Gender::Female },
            Voice { id: "aura-orion-en".to_string(), name: "Orion (aura)".to_string(), vendor: "Deepgram".to_string(), model: "aura".to_string(), language: "English".to_string(), gender: Gender::Male },
            Voice { id: "aura-orpheus-en".to_string(), name: "Orpheus (aura)".to_string(), vendor: "Deepgram".to_string(), model: "aura".to_string(), language: "English".to_string(), gender: Gender::Male },
            Voice { id: "aura-perseus-en".to_string(), name: "Perseus (aura)".to_string(), vendor: "Deepgram".to_string(), model: "aura".to_string(), language: "English".to_string(), gender: Gender::Male },
            Voice { id: "aura-stella-en".to_string(), name: "Stella (aura)".to_string(), vendor: "Deepgram".to_string(), model: "aura".to_string(), language: "English".to_string(), gender: Gender::Female },
            Voice { id: "aura-zeus-en".to_string(), name: "Zeus (aura)".to_string(), vendor: "Deepgram".to_string(), model: "aura".to_string(), language: "English".to_string(), gender: Gender::Male },
            Voice { id: "aura-2-amalthea-en".to_string(), name: "Amalthea (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-andromeda-en".to_string(), name: "Andromeda (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-apollo-en".to_string(), name: "Apollo (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string(), gender: Gender::Male },
            Voice { id: "aura-2-arcas-en".to_string(), name: "Arcas (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string(), gender: Gender::Male },
            Voice { id: "aura-2-aries-en".to_string(), name: "Aries (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-asteria-en".to_string(), name: "Asteria (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-athena-en".to_string(), name: "Athena (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-atlas-en".to_string(), name: "Atlas (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string(), gender: Gender::Male },
            Voice { id: "aura-2-aurora-en".to_string(), name: "Aurora (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-callista-en".to_string(), name: "Callista (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-cora-en".to_string(), name: "Cora (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-cordelia-en".to_string(), name: "Cordelia (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-delia-en".to_string(), name: "Delia (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-draco-en".to_string(), name: "Draco (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string(), gender: Gender::Male },
            Voice { id: "aura-2-electra-en".to_string(), name: "Electra (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-harmonia-en".to_string(), name: "Harmonia (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-helena-en".to_string(), name: "Helena (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-hera-en".to_string(), name: "Hera (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-hermes-en".to_string(), name: "Hermes (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-hyperion-en".to_string(), name: "Hyperion (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-iris-en".to_string(), name: "Iris (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-janus-en".to_string(), name: "Janus (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string(), gender: Gender::Male },
            Voice { id: "aura-2-juno-en".to_string(), name: "Juno (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-jupiter-en".to_string(), name: "Jupiter (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-luna-en".to_string(), name: "Luna (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-mars-en".to_string(), name: "Mars (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-minerva-en".to_string(), name: "Minerva (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-neptune-en".to_string(), name: "Neptune (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-odysseus-en".to_string(), name: "Odysseus (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-ophelia-en".to_string(), name: "Ophelia (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-orion-en".to_string(), name: "Orion (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string(), gender: Gender::Male },
            Voice { id: "aura-2-orpheus-en".to_string(), name: "Orpheus (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string(), gender: Gender::Male },
            Voice { id: "aura-2-pandora-en".to_string(), name: "Pandora (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-phoebe-en".to_string(), name: "Phoebe (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-pluto-en".to_string(), name: "Pluto (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-saturn-en".to_string(), name: "Saturn (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-selene-en".to_string(), name: "Selene (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-thalia-en".to_string(), name: "Thalia (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-theia-en".to_string(), name: "Theia (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-vesta-en".to_string(), name: "Vesta (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-zeus-en".to_string(), name: "Zeus (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string(), gender: Gender::Male },
            Voice { id: "aura-2-alvaro-es".to_string(), name: "Alvaro (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Spanish".to_string(), gender: Gender::Male },
            Voice { id: "aura-2-aquila-es".to_string(), name: "Aquila (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Spanish".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-carina-es".to_string(), name: "Carina (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Spanish".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-celeste-es".to_string(), name: "Celeste (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Spanish".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-diana-es".to_string(), name: "Diana (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Spanish".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-estrella-es".to_string(), name: "Estrella (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Spanish".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-javier-es".to_string(), name: "Javier (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Spanish".to_string(), gender: Gender::Male },
            Voice { id: "aura-2-nestor-es".to_string(), name: "Nestor (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Spanish".to_string(), gender: Gender::Male },
            Voice { id: "aura-2-selena-es".to_string(), name: "Selena (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Spanish".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-sirio-es".to_string(), name: "Sirio (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Spanish".to_string(), gender: Gender::Male },
            Voice { id: "aura-2-luciano-es".to_string(), name: "Luciano (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Spanish".to_string(), gender: Gender::Male },
            Voice { id: "aura-2-olivia-es".to_string(), name: "Olivia (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Spanish".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-valerio-es".to_string(), name: "Valerio (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Spanish".to_string(), gender: Gender::Male },
            Voice { id: "aura-2-agustina-es".to_string(), name: "Agustina (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Spanish".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-silvia-es".to_string(), name: "Silvia (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Spanish".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-gloria-es".to_string(), name: "Gloria (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Spanish".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-antonia-es".to_string(), name: "Antonia (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Spanish".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-beatrix-nl".to_string(), name: "Beatrix (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Dutch".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-daphne-nl".to_string(), name: "Daphne (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Dutch".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-cornelia-nl".to_string(), name: "Cornelia (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Dutch".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-sander-nl".to_string(), name: "Sander (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Dutch".to_string(), gender: Gender::Male },
            Voice { id: "aura-2-hestia-nl".to_string(), name: "Hestia (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Dutch".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-lars-nl".to_string(), name: "Lars (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Dutch".to_string(), gender: Gender::Male },
            Voice { id: "aura-2-roman-nl".to_string(), name: "Roman (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Dutch".to_string(), gender: Gender::Male },
            Voice { id: "aura-2-rhea-nl".to_string(), name: "Rhea (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Dutch".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-leda-nl".to_string(), name: "Leda (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Dutch".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-agathe-fr".to_string(), name: "Agathe (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "French".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-hector-fr".to_string(), name: "Hector (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "French".to_string(), gender: Gender::Male },
            Voice { id: "aura-2-elara-de".to_string(), name: "Elara (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "German".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-aurelia-de".to_string(), name: "Aurelia (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "German".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-lara-de".to_string(), name: "Lara (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "German".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-julius-de".to_string(), name: "Julius (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "German".to_string(), gender: Gender::Male },
            Voice { id: "aura-2-fabian-de".to_string(), name: "Fabian (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "German".to_string(), gender: Gender::Male },
            Voice { id: "aura-2-kara-de".to_string(), name: "Kara (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "German".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-viktoria-de".to_string(), name: "Viktoria (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "German".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-melia-it".to_string(), name: "Melia (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Italian".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-elio-it".to_string(), name: "Elio (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Italian".to_string(), gender: Gender::Male },
            Voice { id: "aura-2-flavio-it".to_string(), name: "Flavio (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Italian".to_string(), gender: Gender::Male },
            Voice { id: "aura-2-maia-it".to_string(), name: "Maia (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Italian".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-cinzia-it".to_string(), name: "Cinzia (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Italian".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-cesare-it".to_string(), name: "Cesare (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Italian".to_string(), gender: Gender::Male },
            Voice { id: "aura-2-livia-it".to_string(), name: "Livia (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Italian".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-perseo-it".to_string(), name: "Perseo (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Italian".to_string(), gender: Gender::Male },
            Voice { id: "aura-2-dionisio-it".to_string(), name: "Dionisio (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Italian".to_string(), gender: Gender::Male },
            Voice { id: "aura-2-demetra-it".to_string(), name: "Demetra (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Italian".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-uzume-ja".to_string(), name: "Uzume (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Japanese".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-ebisu-ja".to_string(), name: "Ebisu (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Japanese".to_string(), gender: Gender::Male },
            Voice { id: "aura-2-fujin-ja".to_string(), name: "Fujin (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Japanese".to_string(), gender: Gender::Male },
            Voice { id: "aura-2-izanami-ja".to_string(), name: "Izanami (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Japanese".to_string(), gender: Gender::Female },
            Voice { id: "aura-2-ama-ja".to_string(), name: "Ama (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Japanese".to_string(), gender: Gender::Female },
        ]);
        map
    };
}


impl App {
    pub fn new(deepgram_endpoint: String, format_index: usize, sample_rate: u32, config: AppConfig) -> App {
        let mut text_table_state = TableState::default();
        text_table_state.select(Some(0)); // Select the first item by default

        let mut voice_menu_state = ListState::default();
        voice_menu_state.select(Some(0)); // Select the first voice by default

        let voices: Vec<Voice> = DEEPGRAM_VOICES.values().flatten().cloned().collect();

        // Resolve API key: env var > config file key
        dotenvy::dotenv().ok();
        let resolved_api_key = std::env::var("DEEPGRAM_API_KEY").ok()
            .or_else(|| config.api.key.clone().filter(|k| !k.is_empty()));

        let mut initial_logs: Vec<LogEntry> = Vec::new();
        let make_entry = |level: LogLevel, message: String| LogEntry {
            level,
            message,
            timestamp: chrono::Local::now(),
        };

        initial_logs.push(make_entry(LogLevel::Info, format!("Config: {}", config::config_path_display())));

        if resolved_api_key.is_none() {
            initial_logs.push(make_entry(
                LogLevel::Warning,
                "No DEEPGRAM_API_KEY set. Press 'k' to enter your API key interactively.".to_string(),
            ));
        }

        let flags = &config.experimental;
        if flags.streaming_playback {
            initial_logs.push(make_entry(LogLevel::Info, "[experimental] streaming_playback enabled".to_string()));
        }
        if flags.ssml_support {
            initial_logs.push(make_entry(LogLevel::Info, "[experimental] ssml_support enabled".to_string()));
        }

        // Validate format index
        let format_index = format_index.min(AUDIO_FORMATS.len() - 1);
        let fmt = &AUDIO_FORMATS[format_index];

        // Snap sample_rate to a valid value for the chosen format
        let sample_rate = if fmt.valid_sample_rates.contains(&sample_rate) {
            sample_rate
        } else {
            fmt.default_sample_rate
        };

        if format_index != DEFAULT_FORMAT_INDEX {
            initial_logs.push(make_entry(LogLevel::Info, format!("Audio format: {} | {} Hz", fmt.display_name, sample_rate)));
        } else if sample_rate != fmt.default_sample_rate {
            initial_logs.push(make_entry(LogLevel::Info, format!("Sample rate: {} Hz", sample_rate)));
        }

        // Pre-select format in format menu
        let mut audio_format_menu_state = ListState::default();
        audio_format_menu_state.select(Some(format_index));

        // Pre-select sample rate within the format's valid rates
        let rate_index = fmt.valid_sample_rates.iter().position(|&r| r == sample_rate).unwrap_or(0);
        let mut sample_rate_menu_state = ListState::default();
        sample_rate_menu_state.select(Some(rate_index));

        App {
            config,
            current_screen: CurrentScreen::Main,
            currently_editing: None,
            text_table_state,
            voice_menu_state,
            saved_texts: persistence::load_saved_texts(),
            voices,
            audio_cache_dir: Self::get_audio_cache_dir().expect("Failed to get audio cache directory"),
            deepgram_endpoint,
            status_message: "Press 'n' to add new text, 'd' to delete, 'Enter' to play.".to_string(),
            focused_panel: Panel::TextList,
            logs: initial_logs,
            input_buffer: String::new(),
            api_key_override: resolved_api_key,
            api_key_input_buffer: String::new(),
            voice_filter: String::new(),
            voice_filter_buffer: String::new(),
            audio_format_index: format_index,
            audio_format_menu_state,
            sample_rate,
            sample_rate_menu_state,
            playback_speed: Decimal::from_str("1.0").unwrap(),
            is_loading: false,
            loading_text: String::new(),
            spinner_index: 0,
            audio_sink: None,
            audio_stream: None,
            tts_receiver: None,
            audio_duration_ms: 0,
            playback_start_time: std::time::Instant::now(),
            audio_cache_info: std::collections::HashMap::new(),
            help_scroll_offset: 0,
            log_scroll_offset: 0,
            text_panel_bounds: Rect::default(),
            voice_panel_bounds: Rect::default(),
            log_panel_bounds: Rect::default(),
        }
    }

    pub fn enter_input_mode(&mut self) {
        self.current_screen = CurrentScreen::Editing;
        self.currently_editing = Some(CurrentlyEditing::Text);
        self.input_buffer.clear();
        self.status_message = "Editing... Press 'Enter' to save, 'Esc' to cancel.".to_string();
    }

    pub fn exit_input_mode(&mut self) {
        self.current_screen = CurrentScreen::Main;
        self.currently_editing = None;
        self.input_buffer.clear();
        self.status_message = "Press 'n' to add new text, 'd' to delete, 'Enter' to play.".to_string();
    }

    #[allow(dead_code)]
    pub fn experimental(&self) -> &ExperimentalFlags {
        &self.config.experimental
    }

    pub fn open_audio_cache_in_finder(&mut self) {
        let dir = self.audio_cache_dir.clone();
        match std::process::Command::new("open").arg(&dir).spawn() {
            Ok(_) => self.add_log(format!("Opened cache folder in Finder: {}", dir)),
            Err(e) => self.add_log_with_level(LogLevel::Error, format!("Failed to open cache folder: {}", e)),
        }
    }

    pub fn enter_api_key_mode(&mut self) {
        self.current_screen = CurrentScreen::ApiKeyInput;
        self.api_key_input_buffer.clear();
        self.status_message = "Enter your Deepgram API key. Press 'Enter' to save, 'Esc' to cancel.".to_string();
    }

    pub fn save_api_key(&mut self) {
        let key = self.api_key_input_buffer.trim().to_string();
        if !key.is_empty() {
            self.api_key_override = Some(key);
            self.add_log_with_level(LogLevel::Success, "API key set successfully.".to_string());
        }
        self.api_key_input_buffer.clear();
        self.current_screen = CurrentScreen::Main;
        self.status_message = "Press 'n' to add new text, 'd' to delete, 'Enter' to play.".to_string();
    }

    pub fn exit_api_key_mode(&mut self) {
        self.api_key_input_buffer.clear();
        self.current_screen = CurrentScreen::Main;
        self.status_message = "Press 'n' to add new text, 'd' to delete, 'Enter' to play.".to_string();
    }

    pub fn show_help_screen(&mut self) {
        self.current_screen = CurrentScreen::Help;
    }

    pub fn exit_help_screen(&mut self) {
        self.current_screen = CurrentScreen::Main;
        self.help_scroll_offset = 0;
    }

    pub fn scroll_help(&mut self, direction: i32, max_lines: usize) {
        let offset = self.help_scroll_offset as i32 + direction;
        self.help_scroll_offset = offset.max(0) as usize;

        // Clamp to prevent scrolling past the end
        let visible_lines = 15; // Approximate visible lines in help box
        if self.help_scroll_offset > max_lines.saturating_sub(visible_lines) {
            self.help_scroll_offset = max_lines.saturating_sub(visible_lines);
        }
    }

    pub fn handle_mouse_click(&mut self, x: u16, y: u16) {
        // Check if click is within text panel bounds
        if x >= self.text_panel_bounds.x
            && x < self.text_panel_bounds.x + self.text_panel_bounds.width
            && y >= self.text_panel_bounds.y
            && y < self.text_panel_bounds.y + self.text_panel_bounds.height
        {
            self.focused_panel = Panel::TextList;
            // +1 for top border, +1 for header row
            let first_row = self.text_panel_bounds.y + 2;
            if y >= first_row {
                let idx = self.text_table_state.offset() + (y - first_row) as usize;
                if idx < self.saved_texts.len() {
                    self.text_table_state.select(Some(idx));
                }
            }
        }
        // Check if click is within voice panel bounds
        else if x >= self.voice_panel_bounds.x
            && x < self.voice_panel_bounds.x + self.voice_panel_bounds.width
            && y >= self.voice_panel_bounds.y
            && y < self.voice_panel_bounds.y + self.voice_panel_bounds.height
        {
            self.focused_panel = Panel::VoiceMenu;
            // +1 for top border only (no header row)
            let first_row = self.voice_panel_bounds.y + 1;
            if y >= first_row {
                let idx = self.voice_menu_state.offset() + (y - first_row) as usize;
                // Ignore clicks on language separator rows
                if !self.is_language_separator_index(idx) {
                    let total_items = self.voice_display_item_count();
                    if idx < total_items {
                        self.voice_menu_state.select(Some(idx));
                    }
                }
            }
        }
    }

    fn voice_display_item_count(&self) -> usize {
        let mut count = 0;
        let mut current_language: Option<String> = None;
        for voice in self.get_filtered_voices() {
            if current_language.as_ref() != Some(&voice.language) {
                current_language = Some(voice.language.clone());
                count += 1; // separator
            }
            count += 1;
        }
        count
    }

    pub fn save_input_as_text(&mut self) {
        if !self.input_buffer.trim().is_empty() {
            self.saved_texts.push(self.input_buffer.clone());
            self.add_log(format!("Added new text: {}", self.input_buffer));

            // Persist to disk
            if let Err(e) = persistence::save_saved_texts(&self.saved_texts) {
                self.add_log(format!("Warning: Failed to save texts to disk: {}", e));
            }
        }
        self.exit_input_mode();
    }

    pub fn paste_from_clipboard(&mut self) {
        match arboard::Clipboard::new() {
            Ok(mut clipboard) => {
                match clipboard.get_text() {
                    Ok(text) => {
                        self.input_buffer.push_str(&text);
                        self.add_log("Pasted from clipboard".to_string());
                    }
                    Err(e) => {
                        self.add_log(format!("Failed to paste from clipboard: {}", e));
                    }
                }
            }
            Err(e) => {
                self.add_log(format!("Failed to access clipboard: {}", e));
            }
        }
    }

    pub fn paste_from_clipboard_to_api_key(&mut self) {
        match arboard::Clipboard::new() {
            Ok(mut clipboard) => {
                match clipboard.get_text() {
                    Ok(text) => {
                        self.api_key_input_buffer.push_str(&text);
                        self.add_log("Pasted API key from clipboard".to_string());
                    }
                    Err(e) => {
                        self.add_log(format!("Failed to paste from clipboard: {}", e));
                    }
                }
            }
            Err(e) => {
                self.add_log(format!("Failed to access clipboard: {}", e));
            }
        }
    }

    pub fn delete_selected_text(&mut self) {
        if let Some(index) = self.text_table_state.selected() {
            if index < self.saved_texts.len() {
                let removed = self.saved_texts.remove(index);
                self.add_log(format!("Deleted text: {}", removed));

                // Adjust selection
                if self.saved_texts.is_empty() {
                    self.text_table_state.select(None);
                } else if index >= self.saved_texts.len() {
                    self.text_table_state.select(Some(self.saved_texts.len() - 1));
                }

                // Persist to disk
                if let Err(e) = persistence::save_saved_texts(&self.saved_texts) {
                    self.add_log(format!("Warning: Failed to save texts to disk: {}", e));
                }
            }
        }
    }


    pub fn add_log(&mut self, message: String) {
        self.add_log_with_level(LogLevel::Info, message);
    }

    pub fn add_log_with_level(&mut self, level: LogLevel, message: String) {
        self.logs.push(LogEntry { level, message, timestamp: chrono::Local::now() });
        if self.logs.len() > 500 {
            self.logs.remove(0);
        }
        // Reset scroll to show newest entry
        self.log_scroll_offset = 0;
    }

    /// Scroll the log panel. direction: -1 = scroll toward older, +1 = scroll toward newer.
    /// The log is rendered newest-first, so scrolling "up" (direction -1) means
    /// incrementing the offset to reveal older entries.
    pub fn scroll_logs(&mut self, direction: i32) {
        let offset = self.log_scroll_offset as i32 - direction;
        self.log_scroll_offset = offset.max(0) as usize;
        // Upper bound is clamped in the renderer where visible height is known
    }

    fn get_audio_cache_dir() -> Result<String> {
        if let Some(proj_dirs) = ProjectDirs::from("com", "deepgram", "tts-tui") {
            let cache_dir = proj_dirs.cache_dir().join("audio");
            std::fs::create_dir_all(&cache_dir)?;
            Ok(cache_dir.to_str().unwrap().to_string())
        } else {
            Err(anyhow::anyhow!("Could not determine project directories"))
        }
    }

    pub fn scroll_text_list(&mut self, direction: i32) {
        if self.focused_panel == Panel::TextList {
            let i = match self.text_table_state.selected() {
                Some(i) => {
                    let new_index = i as i32 + direction;
                    if new_index < 0 {
                        self.saved_texts.len() - 1
                    } else if new_index as usize >= self.saved_texts.len() {
                        0
                    } else {
                        new_index as usize
                    }
                }
                None => 0,
            };
            self.text_table_state.select(Some(i));
        } else if self.focused_panel == Panel::VoiceMenu {
            let filtered_voices = self.get_filtered_voices();
            // Calculate total items including separators
            let mut current_language: Option<String> = None;
            let mut total_items = 0;
            for voice in filtered_voices.iter() {
                if current_language.as_ref() != Some(&voice.language) {
                    current_language = Some(voice.language.clone());
                    total_items += 1;
                }
                total_items += 1;
            }

            let i = match self.voice_menu_state.selected() {
                Some(i) => {
                    let mut new_index = i as i32 + direction;
                    // Clamp and skip separators
                    if new_index < 0 {
                        new_index = (total_items - 1) as i32;
                    } else if new_index as usize >= total_items {
                        new_index = 0;
                    }

                    // Skip separators
                    while new_index >= 0 && new_index < total_items as i32 && self.is_language_separator_index(new_index as usize) {
                        new_index += direction;
                    }
                    // Wrap around if needed
                    if new_index < 0 {
                        new_index = (total_items - 1) as i32;
                    } else if new_index as usize >= total_items {
                        new_index = 0;
                    }

                    new_index as usize
                }
                None => 0,
            };
            self.voice_menu_state.select(Some(i));
        }
    }

    pub fn get_selected_text(&self) -> Option<String> {
        self.text_table_state.selected().map(|i| self.saved_texts[i].clone())
    }

    pub fn set_status_message(&mut self, message: String) {
        self.status_message = message;
    }

    pub fn focus_next_panel(&mut self) {
        self.focused_panel = match self.focused_panel {
            Panel::TextList => Panel::VoiceMenu,
            Panel::VoiceMenu => Panel::TextList,
        };
    }

    pub fn focus_prev_panel(&mut self) {
        self.focused_panel = match self.focused_panel {
            Panel::TextList => Panel::VoiceMenu,
            Panel::VoiceMenu => Panel::TextList,
        };
    }

    pub fn get_filtered_voices(&self) -> Vec<&Voice> {
        if self.voice_filter.is_empty() {
            self.voices.iter().collect()
        } else {
            let filter_lower = self.voice_filter.to_lowercase();
            self.voices
                .iter()
                .filter(|voice| {
                    voice.name.to_lowercase().contains(&filter_lower)
                        || voice.language.to_lowercase().contains(&filter_lower)
                        || voice.model.to_lowercase().contains(&filter_lower)
                })
                .collect()
        }
    }

    /// Returns voices matching `voice_filter_buffer` — used for live count in the filter popup.
    pub fn get_filtered_voices_for_buffer(&self) -> Vec<&Voice> {
        if self.voice_filter_buffer.is_empty() {
            self.voices.iter().collect()
        } else {
            let filter_lower = self.voice_filter_buffer.to_lowercase();
            self.voices
                .iter()
                .filter(|voice| {
                    voice.name.to_lowercase().contains(&filter_lower)
                        || voice.language.to_lowercase().contains(&filter_lower)
                        || voice.model.to_lowercase().contains(&filter_lower)
                })
                .collect()
        }
    }

    fn is_language_separator_index(&self, item_index: usize) -> bool {
        let filtered_voices = self.get_filtered_voices();
        let mut current_language: Option<String> = None;
        let mut item_counter = 0;

        for voice in filtered_voices.iter() {
            if current_language.as_ref() != Some(&voice.language) {
                if item_counter == item_index {
                    return true; // This is a separator
                }
                current_language = Some(voice.language.clone());
                item_counter += 1;
            }
            if item_counter == item_index {
                return false; // This is a voice
            }
            item_counter += 1;
        }
        false
    }

    fn get_voice_index_from_display_index(&self, display_index: usize) -> Option<usize> {
        let filtered_voices = self.get_filtered_voices();
        let mut current_language: Option<String> = None;
        let mut item_counter = 0;
        let mut voice_counter = 0;

        for voice in filtered_voices.iter() {
            if current_language.as_ref() != Some(&voice.language) {
                current_language = Some(voice.language.clone());
                item_counter += 1; // Skip separator
            }

            if item_counter == display_index {
                return Some(voice_counter);
            }

            item_counter += 1;
            voice_counter += 1;
        }
        None
    }

    pub fn get_selected_voice(&self) -> Option<&Voice> {
        let filtered_voices = self.get_filtered_voices();
        self.voice_menu_state.selected().and_then(|display_index| {
            self.get_voice_index_from_display_index(display_index)
                .and_then(|voice_index| {
                    if voice_index < filtered_voices.len() {
                        Some(filtered_voices[voice_index])
                    } else {
                        None
                    }
                })
        })
    }

    pub fn clear_voice_filter(&mut self) {
        self.voice_filter.clear();
        self.voice_menu_state.select(Some(0));
    }

    pub fn enter_voice_filter_mode(&mut self) {
        self.voice_filter_buffer = self.voice_filter.clone();
        self.current_screen = CurrentScreen::VoiceFilter;
        self.status_message = "Filter voices — Enter to apply, Esc to cancel, Ctrl+U to clear".to_string();
    }

    pub fn apply_voice_filter(&mut self) {
        self.voice_filter = self.voice_filter_buffer.clone();
        self.voice_filter_buffer.clear();
        self.voice_menu_state.select(Some(0));
        self.current_screen = CurrentScreen::Main;
        self.focused_panel = Panel::VoiceMenu;
        self.status_message = "Press 'n' to add new text, 'd' to delete, 'Enter' to play.".to_string();
    }

    pub fn cancel_voice_filter(&mut self) {
        self.voice_filter_buffer.clear();
        self.current_screen = CurrentScreen::Main;
        self.status_message = "Press 'n' to add new text, 'd' to delete, 'Enter' to play.".to_string();
    }

    pub fn clear_voice_filter_buffer(&mut self) {
        self.voice_filter_buffer.clear();
    }

    pub fn current_audio_format(&self) -> &'static AudioFormat {
        &AUDIO_FORMATS[self.audio_format_index]
    }

    pub fn enter_audio_format_mode(&mut self) {
        self.audio_format_menu_state.select(Some(self.audio_format_index));
        self.current_screen = CurrentScreen::AudioFormatSelect;
        self.status_message = "Select audio format — Enter to apply, Esc to cancel".to_string();
    }

    pub fn apply_audio_format(&mut self) {
        if let Some(idx) = self.audio_format_menu_state.selected() {
            if idx < AUDIO_FORMATS.len() {
                self.audio_format_index = idx;
                let fmt = &AUDIO_FORMATS[idx];
                // Snap sample rate to a valid value for the new format
                if !fmt.valid_sample_rates.contains(&self.sample_rate) {
                    let old_rate = self.sample_rate;
                    self.sample_rate = fmt.default_sample_rate;
                    self.add_log_with_level(LogLevel::Info,
                        format!("Sample rate adjusted from {} to {} Hz for {} encoding",
                            old_rate, self.sample_rate, fmt.display_name));
                }
                self.add_log_with_level(LogLevel::Info,
                    format!("Audio format: {} | {} Hz", fmt.display_name, self.sample_rate));
            }
        }
        self.current_screen = CurrentScreen::Main;
        self.status_message = "Press 'n' to add new text, 'd' to delete, 'Enter' to play.".to_string();
    }

    pub fn cancel_audio_format_mode(&mut self) {
        self.current_screen = CurrentScreen::Main;
        self.status_message = "Press 'n' to add new text, 'd' to delete, 'Enter' to play.".to_string();
    }

    /// Close whatever popup is currently open and return to Main.
    /// Called by the global Esc handler so every popup dismisses consistently.
    pub fn close_current_popup(&mut self) {
        match self.current_screen {
            CurrentScreen::Editing         => self.exit_input_mode(),
            CurrentScreen::Help            => self.exit_help_screen(),
            CurrentScreen::ApiKeyInput     => self.exit_api_key_mode(),
            CurrentScreen::VoiceFilter     => self.cancel_voice_filter(),
            CurrentScreen::SampleRateSelect  => self.cancel_sample_rate_mode(),
            CurrentScreen::AudioFormatSelect => self.cancel_audio_format_mode(),
            CurrentScreen::Main            => {}
        }
    }

    pub fn scroll_audio_format_menu(&mut self, direction: i32) {
        let len = AUDIO_FORMATS.len();
        let i = match self.audio_format_menu_state.selected() {
            Some(i) => {
                let new = i as i32 + direction;
                if new < 0 { len - 1 } else if new as usize >= len { 0 } else { new as usize }
            }
            None => 0,
        };
        self.audio_format_menu_state.select(Some(i));
    }

    pub fn enter_sample_rate_mode(&mut self) {
        let rates = self.current_audio_format().valid_sample_rates;
        let rate_index = rates.iter().position(|&r| r == self.sample_rate).unwrap_or(0);
        self.sample_rate_menu_state.select(Some(rate_index));
        self.current_screen = CurrentScreen::SampleRateSelect;
        self.status_message = "Select sample rate — Enter to apply, Esc to cancel".to_string();
    }

    pub fn apply_sample_rate(&mut self) {
        if let Some(idx) = self.sample_rate_menu_state.selected() {
            let rates = self.current_audio_format().valid_sample_rates;
            if idx < rates.len() {
                self.sample_rate = rates[idx];
                self.add_log_with_level(LogLevel::Info, format!("Sample rate set to {} Hz", self.sample_rate));
            }
        }
        self.current_screen = CurrentScreen::Main;
        self.status_message = "Press 'n' to add new text, 'd' to delete, 'Enter' to play.".to_string();
    }

    pub fn cancel_sample_rate_mode(&mut self) {
        self.current_screen = CurrentScreen::Main;
        self.status_message = "Press 'n' to add new text, 'd' to delete, 'Enter' to play.".to_string();
    }

    pub fn scroll_sample_rate_menu(&mut self, direction: i32) {
        let len = self.current_audio_format().valid_sample_rates.len();
        let i = match self.sample_rate_menu_state.selected() {
            Some(i) => {
                let new = i as i32 + direction;
                if new < 0 { len - 1 } else if new as usize >= len { 0 } else { new as usize }
            }
            None => 0,
        };
        self.sample_rate_menu_state.select(Some(i));
    }

    pub fn increase_speed(&mut self) {
        let increment = Decimal::from_str("0.05").unwrap();
        let max_speed = Decimal::from_str("1.5").unwrap();
        self.playback_speed = (self.playback_speed + increment).min(max_speed);
        self.set_status_message(format!("Speed: {:.2}x", self.playback_speed));
    }

    pub fn decrease_speed(&mut self) {
        let decrement = Decimal::from_str("0.05").unwrap();
        let min_speed = Decimal::from_str("0.7").unwrap();
        self.playback_speed = (self.playback_speed - decrement).max(min_speed);
        self.set_status_message(format!("Speed: {:.2}x", self.playback_speed));
    }

    pub fn reset_speed(&mut self) {
        self.playback_speed = Decimal::from_str("1.0").unwrap();
        self.set_status_message("Speed: 1.00x (default)".to_string());
    }

    pub fn start_loading(&mut self, text: String) {
        self.is_loading = true;
        self.loading_text = text;
        self.spinner_index = 0;
        self.audio_duration_ms = 0;
        self.playback_start_time = std::time::Instant::now();
    }

    pub fn stop_loading(&mut self) {
        self.is_loading = false;
        self.loading_text.clear();
    }

    pub fn update_spinner(&mut self) {
        if self.is_loading {
            self.spinner_index = (self.spinner_index + 1) % SPINNER_FRAMES.len();
        }
    }

    pub fn get_spinner_char(&self) -> &str {
        SPINNER_FRAMES[self.spinner_index]
    }

    pub fn check_audio_playback(&mut self) {
        if self.is_loading {
            if let Some(sink) = &self.audio_sink {
                if sink.empty() {
                    // Playback finished - clean up
                    self.stop_loading();
                    self.audio_sink = None;
                    self.audio_stream = None;
                    self.set_status_message("Playback complete".to_string());
                }
            }
        }
    }

    pub fn stop_audio_playback(&mut self) {
        if let Some(sink) = self.audio_sink.take() {
            sink.stop();
        }
        self.audio_stream = None;
        self.stop_loading();
        self.set_status_message("Playback stopped".to_string());
    }

    pub fn get_playback_progress(&self) -> (u64, u64) {
        if self.is_loading && !self.audio_sink.is_none() {
            let elapsed = self.playback_start_time.elapsed().as_millis() as u64;
            (elapsed, self.audio_duration_ms)
        } else {
            (0, 0)
        }
    }

    pub fn check_tts_result(&mut self) -> Option<Vec<u8>> {
        if let Some(receiver) = &mut self.tts_receiver {
            // Non-blocking check for TTS result
            if let Ok(result) = receiver.try_recv() {
                self.tts_receiver = None;
                match result {
                    TtsResult::Success { message, audio_data, is_cached } => {
                        let log_level = if is_cached { LogLevel::Success } else { LogLevel::Info };
                        self.add_log_with_level(log_level, message.clone());
                        if let Some(text) = self.get_selected_text() {
                            self.audio_cache_info.insert(text, is_cached);
                        }
                        return Some(audio_data);
                    }
                    TtsResult::Error(error) => {
                        self.stop_loading();
                        self.add_log_with_level(LogLevel::Error, format!("Error: {}", error));
                        self.set_status_message("Error occurred during TTS".to_string());
                    }
                }
            }
        }
        None
    }
}
