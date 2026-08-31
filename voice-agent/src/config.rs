use std::{collections::BTreeMap, fs, io, path::PathBuf};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct VoiceAgentConfig {
    #[serde(default)]
    pub historical_prompts: Vec<String>,
    #[serde(default)]
    pub show_timestamps: bool,
    #[serde(default)]
    pub verbose_json_logging: bool,
    #[serde(default)]
    pub last_tts: TtsConfig,
    #[serde(default)]
    pub listen: ListenConfig,
    /// Preserve unrelated YAML keys so this file can be shared with other tools.
    #[serde(flatten)]
    pub other: BTreeMap<String, serde_yaml::Value>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct TtsConfig {
    pub voice: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ListenConfig {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub version: Option<String>,
    pub language: Option<String>,
    pub language_hints: Vec<String>,
    pub keyterms: Vec<String>,
    pub eot_threshold: Option<f32>,
    pub eager_eot_threshold: Option<f32>,
    pub smart_format: Option<bool>,
}

pub fn path() -> Option<PathBuf> {
    ProjectDirs::from("", "", "deepgram").map(|dirs| dirs.config_dir().join("voice-agent.yml"))
}

pub fn load() -> io::Result<VoiceAgentConfig> {
    let Some(path) = path() else {
        return Ok(VoiceAgentConfig::default());
    };
    match fs::read_to_string(path) {
        Ok(contents) => serde_yaml::from_str(&contents)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(VoiceAgentConfig::default()),
        Err(error) => Err(error),
    }
}

pub fn save(config: &VoiceAgentConfig) -> io::Result<()> {
    let Some(path) = path() else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let contents = serde_yaml::to_string(config)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    fs::write(path, contents)
}
