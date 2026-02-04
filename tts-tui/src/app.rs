use ratatui::widgets::TableState;
use ratatui::widgets::ListState;
use directories::ProjectDirs;
use anyhow::Result;
use lazy_static::lazy_static;
use std::collections::HashMap;
use rust_decimal::Decimal;
use std::str::FromStr;

#[derive(Clone, Debug, PartialEq)]
pub struct Voice {
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub model: String,
    pub language: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CurrentScreen {
    Main,
    Editing,
    Help,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CurrentlyEditing {
    Text,
    Voice,
}

pub struct App {
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
    pub logs: Vec<String>,
    pub input_buffer: String,
    pub voice_filter: String,
    pub playback_speed: Decimal,  // Range: 0.7 to 1.5
}

#[derive(Clone, Debug, PartialEq)]
pub enum Panel {
    TextList,
    VoiceMenu,
}

lazy_static! {
    static ref DEEPGRAM_VOICES: HashMap<&'static str, Vec<Voice>> = {
        let mut map = HashMap::new();
        // Deepgram Voices
        map.insert("Deepgram", vec![
            Voice { id: "aura-angus-en".to_string(), name: "Angus (aura)".to_string(), vendor: "Deepgram".to_string(), model: "aura".to_string(), language: "English".to_string() },
            Voice { id: "aura-asteria-en".to_string(), name: "Asteria (aura)".to_string(), vendor: "Deepgram".to_string(), model: "aura".to_string(), language: "English".to_string() },
            Voice { id: "aura-athena-en".to_string(), name: "Athena (aura)".to_string(), vendor: "Deepgram".to_string(), model: "aura".to_string(), language: "English".to_string() },
            Voice { id: "aura-helios-en".to_string(), name: "Helios (aura)".to_string(), vendor: "Deepgram".to_string(), model: "aura".to_string(), language: "English".to_string() },
            Voice { id: "aura-hera-en".to_string(), name: "Hera (aura)".to_string(), vendor: "Deepgram".to_string(), model: "aura".to_string(), language: "English".to_string() },
            Voice { id: "aura-luna-en".to_string(), name: "Luna (aura)".to_string(), vendor: "Deepgram".to_string(), model: "aura".to_string(), language: "English".to_string() },
            Voice { id: "aura-orion-en".to_string(), name: "Orion (aura)".to_string(), vendor: "Deepgram".to_string(), model: "aura".to_string(), language: "English".to_string() },
            Voice { id: "aura-orpheus-en".to_string(), name: "Orpheus (aura)".to_string(), vendor: "Deepgram".to_string(), model: "aura".to_string(), language: "English".to_string() },
            Voice { id: "aura-perseus-en".to_string(), name: "Perseus (aura)".to_string(), vendor: "Deepgram".to_string(), model: "aura".to_string(), language: "English".to_string() },
            Voice { id: "aura-stella-en".to_string(), name: "Stella (aura)".to_string(), vendor: "Deepgram".to_string(), model: "aura".to_string(), language: "English".to_string() },
            Voice { id: "aura-zeus-en".to_string(), name: "Zeus (aura)".to_string(), vendor: "Deepgram".to_string(), model: "aura".to_string(), language: "English".to_string() },
            Voice { id: "aura-2-amalthea-en".to_string(), name: "Amalthea (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string() },
            Voice { id: "aura-2-andromeda-en".to_string(), name: "Andromeda (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string() },
            Voice { id: "aura-2-apollo-en".to_string(), name: "Apollo (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string() },
            Voice { id: "aura-2-arcas-en".to_string(), name: "Arcas (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string() },
            Voice { id: "aura-2-aries-en".to_string(), name: "Aries (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string() },
            Voice { id: "aura-2-asteria-en".to_string(), name: "Asteria (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string() },
            Voice { id: "aura-2-athena-en".to_string(), name: "Athena (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string() },
            Voice { id: "aura-2-atlas-en".to_string(), name: "Atlas (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string() },
            Voice { id: "aura-2-aurora-en".to_string(), name: "Aurora (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string() },
            Voice { id: "aura-2-callista-en".to_string(), name: "Callista (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string() },
            Voice { id: "aura-2-cora-en".to_string(), name: "Cora (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string() },
            Voice { id: "aura-2-cordelia-en".to_string(), name: "Cordelia (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string() },
            Voice { id: "aura-2-delia-en".to_string(), name: "Delia (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string() },
            Voice { id: "aura-2-draco-en".to_string(), name: "Draco (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string() },
            Voice { id: "aura-2-electra-en".to_string(), name: "Electra (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string() },
            Voice { id: "aura-2-harmonia-en".to_string(), name: "Harmonia (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string() },
            Voice { id: "aura-2-helena-en".to_string(), name: "Helena (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string() },
            Voice { id: "aura-2-hera-en".to_string(), name: "Hera (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string() },
            Voice { id: "aura-2-hermes-en".to_string(), name: "Hermes (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string() },
            Voice { id: "aura-2-hyperion-en".to_string(), name: "Hyperion (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string() },
            Voice { id: "aura-2-iris-en".to_string(), name: "Iris (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string() },
            Voice { id: "aura-2-janus-en".to_string(), name: "Janus (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string() },
            Voice { id: "aura-2-juno-en".to_string(), name: "Juno (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string() },
            Voice { id: "aura-2-jupiter-en".to_string(), name: "Jupiter (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string() },
            Voice { id: "aura-2-luna-en".to_string(), name: "Luna (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string() },
            Voice { id: "aura-2-mars-en".to_string(), name: "Mars (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string() },
            Voice { id: "aura-2-minerva-en".to_string(), name: "Minerva (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string() },
            Voice { id: "aura-2-neptune-en".to_string(), name: "Neptune (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string() },
            Voice { id: "aura-2-odysseus-en".to_string(), name: "Odysseus (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string() },
            Voice { id: "aura-2-ophelia-en".to_string(), name: "Ophelia (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string() },
            Voice { id: "aura-2-orion-en".to_string(), name: "Orion (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string() },
            Voice { id: "aura-2-orpheus-en".to_string(), name: "Orpheus (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string() },
            Voice { id: "aura-2-pandora-en".to_string(), name: "Pandora (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string() },
            Voice { id: "aura-2-phoebe-en".to_string(), name: "Phoebe (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string() },
            Voice { id: "aura-2-pluto-en".to_string(), name: "Pluto (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string() },
            Voice { id: "aura-2-saturn-en".to_string(), name: "Saturn (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string() },
            Voice { id: "aura-2-selene-en".to_string(), name: "Selene (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string() },
            Voice { id: "aura-2-thalia-en".to_string(), name: "Thalia (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string() },
            Voice { id: "aura-2-theia-en".to_string(), name: "Theia (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string() },
            Voice { id: "aura-2-vesta-en".to_string(), name: "Vesta (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string() },
            Voice { id: "aura-2-zeus-en".to_string(), name: "Zeus (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "English".to_string() },
            Voice { id: "aura-2-alvaro-es".to_string(), name: "Alvaro (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Spanish".to_string() },
            Voice { id: "aura-2-aquila-es".to_string(), name: "Aquila (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Spanish".to_string() },
            Voice { id: "aura-2-carina-es".to_string(), name: "Carina (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Spanish".to_string() },
            Voice { id: "aura-2-celeste-es".to_string(), name: "Celeste (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Spanish".to_string() },
            Voice { id: "aura-2-diana-es".to_string(), name: "Diana (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Spanish".to_string() },
            Voice { id: "aura-2-estrella-es".to_string(), name: "Estrella (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Spanish".to_string() },
            Voice { id: "aura-2-javier-es".to_string(), name: "Javier (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Spanish".to_string() },
            Voice { id: "aura-2-nestor-es".to_string(), name: "Nestor (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Spanish".to_string() },
            Voice { id: "aura-2-selena-es".to_string(), name: "Selena (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Spanish".to_string() },
            Voice { id: "aura-2-sirio-es".to_string(), name: "Sirio (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Spanish".to_string() },
            Voice { id: "aura-2-luciano-es".to_string(), name: "Luciano (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Spanish".to_string() },
            Voice { id: "aura-2-olivia-es".to_string(), name: "Olivia (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Spanish".to_string() },
            Voice { id: "aura-2-valerio-es".to_string(), name: "Valerio (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Spanish".to_string() },
            Voice { id: "aura-2-agustina-es".to_string(), name: "Agustina (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Spanish".to_string() },
            Voice { id: "aura-2-silvia-es".to_string(), name: "Silvia (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Spanish".to_string() },
            Voice { id: "aura-2-gloria-es".to_string(), name: "Gloria (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Spanish".to_string() },
            Voice { id: "aura-2-antonia-es".to_string(), name: "Antonia (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Spanish".to_string() },
            Voice { id: "aura-2-beatrix-nl".to_string(), name: "Beatrix (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Dutch".to_string() },
            Voice { id: "aura-2-daphne-nl".to_string(), name: "Daphne (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Dutch".to_string() },
            Voice { id: "aura-2-cornelia-nl".to_string(), name: "Cornelia (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Dutch".to_string() },
            Voice { id: "aura-2-sander-nl".to_string(), name: "Sander (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Dutch".to_string() },
            Voice { id: "aura-2-hestia-nl".to_string(), name: "Hestia (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Dutch".to_string() },
            Voice { id: "aura-2-lars-nl".to_string(), name: "Lars (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Dutch".to_string() },
            Voice { id: "aura-2-roman-nl".to_string(), name: "Roman (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Dutch".to_string() },
            Voice { id: "aura-2-rhea-nl".to_string(), name: "Rhea (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Dutch".to_string() },
            Voice { id: "aura-2-leda-nl".to_string(), name: "Leda (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Dutch".to_string() },
            Voice { id: "aura-2-agathe-fr".to_string(), name: "Agathe (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "French".to_string() },
            Voice { id: "aura-2-hector-fr".to_string(), name: "Hector (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "French".to_string() },
            Voice { id: "aura-2-elara-de".to_string(), name: "Elara (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "German".to_string() },
            Voice { id: "aura-2-aurelia-de".to_string(), name: "Aurelia (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "German".to_string() },
            Voice { id: "aura-2-lara-de".to_string(), name: "Lara (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "German".to_string() },
            Voice { id: "aura-2-julius-de".to_string(), name: "Julius (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "German".to_string() },
            Voice { id: "aura-2-fabian-de".to_string(), name: "Fabian (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "German".to_string() },
            Voice { id: "aura-2-kara-de".to_string(), name: "Kara (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "German".to_string() },
            Voice { id: "aura-2-viktoria-de".to_string(), name: "Viktoria (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "German".to_string() },
            Voice { id: "aura-2-melia-it".to_string(), name: "Melia (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Italian".to_string() },
            Voice { id: "aura-2-elio-it".to_string(), name: "Elio (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Italian".to_string() },
            Voice { id: "aura-2-flavio-it".to_string(), name: "Flavio (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Italian".to_string() },
            Voice { id: "aura-2-maia-it".to_string(), name: "Maia (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Italian".to_string() },
            Voice { id: "aura-2-cinzia-it".to_string(), name: "Cinzia (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Italian".to_string() },
            Voice { id: "aura-2-cesare-it".to_string(), name: "Cesare (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Italian".to_string() },
            Voice { id: "aura-2-livia-it".to_string(), name: "Livia (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Italian".to_string() },
            Voice { id: "aura-2-perseo-it".to_string(), name: "Perseo (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Italian".to_string() },
            Voice { id: "aura-2-dionisio-it".to_string(), name: "Dionisio (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Italian".to_string() },
            Voice { id: "aura-2-demetra-it".to_string(), name: "Demetra (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Italian".to_string() },
            Voice { id: "aura-2-uzume-ja".to_string(), name: "Uzume (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Japanese".to_string() },
            Voice { id: "aura-2-ebisu-ja".to_string(), name: "Ebisu (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Japanese".to_string() },
            Voice { id: "aura-2-fujin-ja".to_string(), name: "Fujin (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Japanese".to_string() },
            Voice { id: "aura-2-izanami-ja".to_string(), name: "Izanami (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Japanese".to_string() },
            Voice { id: "aura-2-ama-ja".to_string(), name: "Ama (aura-2)".to_string(), vendor: "Deepgram".to_string(), model: "aura-2".to_string(), language: "Japanese".to_string() },
        ]);
        map
    };
}


impl App {
    pub fn new(deepgram_endpoint: String) -> App {
        let mut text_table_state = TableState::default();
        text_table_state.select(Some(0)); // Select the first item by default

        let mut voice_menu_state = ListState::default();
        voice_menu_state.select(Some(0)); // Select the first voice by default

        let voices: Vec<Voice> = DEEPGRAM_VOICES.values().flatten().cloned().collect();

        App {
            current_screen: CurrentScreen::Main,
            currently_editing: None,
            text_table_state,
            voice_menu_state,
            saved_texts: vec![
                "Hello, this is a test of the Deepgram Text-to-Speech API.".to_string(),
                "The quick brown fox jumps over the lazy dog.".to_string(),
                "Rust is a systems programming language that focuses on safety, speed, and concurrency.".to_string(),
                "Gemini is a family of multimodal models developed by Google AI.".to_string(),
                "This is a longer text to demonstrate scrolling and playback features.".to_string(),
                "Another example sentence for testing purposes.".to_string(),
                "One more for good measure.".to_string(),
            ],
            voices,
            audio_cache_dir: Self::get_audio_cache_dir().expect("Failed to get audio cache directory"),
            deepgram_endpoint,
            status_message: "Press 'n' to add new text, 'd' to delete, 'Enter' to play.".to_string(),
            focused_panel: Panel::TextList,
            logs: Vec::new(),
            input_buffer: String::new(),
            voice_filter: String::new(),
            playback_speed: Decimal::from_str("1.0").unwrap(),
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

    pub fn show_help_screen(&mut self) {
        self.current_screen = CurrentScreen::Help;
    }

    pub fn exit_help_screen(&mut self) {
        self.current_screen = CurrentScreen::Main;
    }

    pub fn save_input_as_text(&mut self) {
        if !self.input_buffer.trim().is_empty() {
            self.saved_texts.push(self.input_buffer.clone());
            self.add_log(format!("Added new text: {}", self.input_buffer));
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
            }
        }
    }


    pub fn add_log(&mut self, message: String) {
        self.logs.push(message);
        // Optional: Limit the number of logs to prevent infinite growth
        if self.logs.len() > 100 {
            self.logs.remove(0);
        }
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
            let i = match self.voice_menu_state.selected() {
                Some(i) => {
                    let new_index = i as i32 + direction;
                    if new_index < 0 {
                        filtered_voices.len().saturating_sub(1)
                    } else if new_index as usize >= filtered_voices.len() {
                        0
                    } else {
                        new_index as usize
                    }
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

    pub fn get_selected_voice(&self) -> Option<&Voice> {
        let filtered_voices = self.get_filtered_voices();
        self.voice_menu_state.selected().and_then(|i| {
            if i < filtered_voices.len() {
                Some(filtered_voices[i])
            } else {
                None
            }
        })
    }

    pub fn clear_voice_filter(&mut self) {
        self.voice_filter.clear();
        self.voice_menu_state.select(Some(0));
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
}
