use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

pub const DEFAULT_HISTORY_LIMIT: usize = 20;
pub const CONFIG_SUBDIRECTORY: &str = "deepgram";
pub const CONFIG_FILE_NAME: &str = "velocity.yml";
pub const CONFIG_BACKUP_FILE_NAME: &str = "velocity.backup.yml";
pub const HISTORY_FILE_NAME: &str = "velocity-history.yml";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum OutputMode {
    #[default]
    DirectInput,
    Clipboard,
    Paste,
}

impl OutputMode {
    pub fn as_label(self) -> &'static str {
        match self {
            OutputMode::DirectInput => "Type directly",
            OutputMode::Clipboard => "Copy to clipboard",
            OutputMode::Paste => "Paste clipboard",
        }
    }

    pub fn all() -> [OutputMode; 3] {
        [OutputMode::DirectInput, OutputMode::Clipboard, OutputMode::Paste]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HotkeyConfig {
    pub push_to_talk: String,
    pub keep_talking: String,
    pub streaming: String,
    pub resend_selected: String,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            push_to_talk: "Win+Ctrl+'".to_string(),
            keep_talking: "Win+Ctrl+Shift+'".to_string(),
            streaming: "Win+Ctrl+[".to_string(),
            resend_selected: "Win+Ctrl+]".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    pub api_key: Option<String>,
    #[serde(default)]
    pub smart_format: bool,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default, rename = "keyterms", alias = "key_terms")]
    pub key_terms: Vec<String>,
    #[serde(default)]
    pub hotkeys: HotkeyConfig,
    pub audio_input: Option<String>,
    #[serde(default = "default_history_limit")]
    pub history_limit: usize,
    #[serde(default)]
    pub output_mode: OutputMode,
    #[serde(default)]
    pub append_newline: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            api_key: None,
            smart_format: false,
            model: default_model(),
            language: None,
            key_terms: Vec::new(),
            hotkeys: HotkeyConfig::default(),
            audio_input: None,
            history_limit: default_history_limit(),
            output_mode: OutputMode::default(),
            append_newline: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConfigFileState {
    pub config: Config,
    pub modified_at: Option<SystemTime>,
}

fn default_model() -> String {
    crate::deepgram::DEFAULT_MODEL.to_string()
}

fn default_history_limit() -> usize {
    DEFAULT_HISTORY_LIMIT
}

pub fn app_data_dir() -> PathBuf {
    let home = std::env::var("USERPROFILE").expect("USERPROFILE not set");
    PathBuf::from(home).join(".config").join(CONFIG_SUBDIRECTORY)
}

pub fn config_path() -> PathBuf {
    app_data_dir().join(CONFIG_FILE_NAME)
}

pub fn backup_path() -> PathBuf {
    app_data_dir().join(CONFIG_BACKUP_FILE_NAME)
}

pub fn history_path() -> PathBuf {
    app_data_dir().join(HISTORY_FILE_NAME)
}

pub fn load() -> Config {
    load_state().map(|state| state.config).unwrap_or_default()
}

pub fn load_state() -> Result<ConfigFileState, String> {
    let path = config_path();
    load_from_path(&path)
}

pub fn load_from_path(path: &Path) -> Result<ConfigFileState, String> {
    if !path.exists() {
        return Ok(ConfigFileState {
            config: Config::default(),
            modified_at: None,
        });
    }

    let contents = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
    let mut config = serde_yaml::from_str::<Config>(&contents)
        .map_err(|e| format!("Failed to parse {}: {e}", path.display()))?;
    config.normalize()?;

    let modified_at = fs::metadata(path).ok().and_then(|m| m.modified().ok());
    Ok(ConfigFileState { config, modified_at })
}

pub fn save(config: &Config) -> Result<(), String> {
    let path = config_path();
    save_to_path(&path, config)
}

pub fn save_to_path(path: &Path, config: &Config) -> Result<(), String> {
    let mut normalized = config.clone();
    normalized.normalize()?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create {}: {e}", parent.display()))?;
    }

    let contents = serde_yaml::to_string(&normalized)
        .map_err(|e| format!("Failed to serialize config: {e}"))?;
    fs::write(path, contents)
        .map_err(|e| format!("Failed to write {}: {e}", path.display()))
}

pub fn ensure_backup(config: &Config) -> Result<(), String> {
    let path = backup_path();
    save_to_path(&path, config)
}

impl Config {
    pub fn normalize(&mut self) -> Result<(), String> {
        self.model = crate::deepgram::normalize_model(&self.model)
            .ok_or_else(|| format!("Unsupported model: {}", self.model.trim()))?
            .to_string();
        self.language = crate::deepgram::normalize_language(&self.model, self.language.as_deref())?;

        self.key_terms = self
            .key_terms
            .iter()
            .map(|term| term.trim())
            .filter(|term| !term.is_empty())
            .map(|term| term.to_string())
            .collect();

        if let Some(device) = &self.audio_input {
            let trimmed = device.trim();
            self.audio_input = if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            };
        }

        if self.history_limit == 0 {
            return Err("History limit must be greater than zero".to_string());
        }

        self.hotkeys.push_to_talk = normalize_hotkey_text(&self.hotkeys.push_to_talk)?;
        self.hotkeys.keep_talking = normalize_hotkey_text(&self.hotkeys.keep_talking)?;
        self.hotkeys.streaming = normalize_hotkey_text(&self.hotkeys.streaming)?;
        self.hotkeys.resend_selected = normalize_hotkey_text(&self.hotkeys.resend_selected)?;
        Ok(())
    }
}

fn normalize_hotkey_text(value: &str) -> Result<String, String> {
    let parts = value
        .split('+')
        .map(|segment| segment.trim())
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();

    if parts.len() < 2 {
        return Err(format!("Invalid hotkey: {value}"));
    }

    Ok(parts.join("+"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_contains_expected_values() {
        let config = Config::default();
        assert_eq!(config.model, crate::deepgram::DEFAULT_MODEL);
        assert_eq!(config.language, None);
        assert_eq!(config.history_limit, DEFAULT_HISTORY_LIMIT);
        assert_eq!(config.hotkeys.push_to_talk, "Win+Ctrl+'");
        assert_eq!(config.hotkeys.resend_selected, "Win+Ctrl+]");
        assert_eq!(config.output_mode, OutputMode::DirectInput);
    }

    #[test]
    fn normalize_rejects_zero_history_limit() {
        let mut config = Config {
            history_limit: 0,
            ..Config::default()
        };
        assert!(config.normalize().is_err());
    }

    #[test]
    fn normalize_trims_key_terms_and_device() {
        let mut config = Config {
            key_terms: vec![" alpha ".into(), "".into(), "beta".into()],
            audio_input: Some("  Headset Mic  ".into()),
            ..Config::default()
        };

        config.normalize().unwrap();

        assert_eq!(config.key_terms, vec!["alpha", "beta"]);
        assert_eq!(config.audio_input.as_deref(), Some("Headset Mic"));
    }

    #[test]
    fn normalize_rejects_language_not_supported_by_model() {
        let mut config = Config {
            model: "nova-2".to_string(),
            language: Some("ar".to_string()),
            ..Config::default()
        };

        assert!(config.normalize().is_err());
    }

    #[test]
    fn serialize_uses_keyterms_field_name() {
        let config = Config {
            key_terms: vec!["Velocity".into(), "Deepgram".into()],
            ..Config::default()
        };

        let yaml = serde_yaml::to_string(&config).unwrap();

        assert!(yaml.contains("keyterms:"));
        assert!(!yaml.contains("key_terms:"));
    }

    #[test]
    fn deserialize_accepts_legacy_key_terms_field_name() {
        let yaml = r#"
model: nova-3
key_terms:
  - Velocity
  - Deepgram
"#;

        let config = serde_yaml::from_str::<Config>(yaml).unwrap();

        assert_eq!(config.key_terms, vec!["Velocity", "Deepgram"]);
    }

    #[test]
    fn app_data_paths_live_under_deepgram_directory() {
        let config_dir = app_data_dir();

        assert!(config_dir.ends_with(Path::new(".config").join(CONFIG_SUBDIRECTORY)));
        assert_eq!(config_path(), config_dir.join(CONFIG_FILE_NAME));
        assert_eq!(backup_path(), config_dir.join(CONFIG_BACKUP_FILE_NAME));
        assert_eq!(history_path(), config_dir.join(HISTORY_FILE_NAME));
    }
}
