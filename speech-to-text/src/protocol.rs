use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) struct DeepgramResponse {
    #[serde(rename = "type")]
    pub(crate) message_type: String,
    // Deepgram uses different channel shapes for Results and control events:
    // Results uses an object, while SpeechStarted/UtteranceEnd may use a scalar
    // or an array. Keep this raw until the message type is known.
    #[serde(default)]
    pub(crate) channel: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Channel {
    pub(crate) alternatives: Vec<Alternative>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Alternative {
    pub(crate) transcript: String,
    pub(crate) confidence: Option<f64>,
    #[serde(default)]
    pub(crate) words: Vec<Word>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Word {
    pub(crate) word: String,
    pub(crate) speaker: Option<u32>,
}

pub(crate) type StreamError = Box<dyn std::error::Error + Send + Sync>;
pub(crate) type StreamResult = Result<(), StreamError>;

#[derive(Clone)]
pub(crate) struct DeepgramClientConfig {
    pub(crate) api_key: Option<String>,
    pub(crate) callback: Option<String>,
    pub(crate) silent: bool,
    pub(crate) endpoint: Option<String>,
    pub(crate) encoding: Option<String>,
    pub(crate) sample_rate_override: Option<u32>,
    pub(crate) channels_override: Option<u16>,
    pub(crate) multichannel: bool,
    pub(crate) diarize: bool,
    pub(crate) detect_entities: bool,
    pub(crate) interim_results: bool,
    pub(crate) vad_events: bool,
    pub(crate) punctuate: bool,
    pub(crate) smart_format: bool,
    pub(crate) sentiment: bool,
    pub(crate) intents: bool,
    pub(crate) topics: bool,
    pub(crate) model: Option<String>,
    pub(crate) version: Option<String>,
    pub(crate) redact: Option<String>,
    pub(crate) language: Option<String>,
    pub(crate) endpointing: Option<u32>,
    pub(crate) utterance_end: Option<u32>,
    pub(crate) keyterm: Option<String>,
    pub(crate) keywords: Option<String>,
}
