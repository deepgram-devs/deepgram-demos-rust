use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Top-level application configuration loaded from ~/.config/tts-tui.toml.
/// Priority order (highest to lowest): CLI args > env vars > TOML config > defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub api: ApiConfig,

    #[serde(default)]
    pub audio: AudioConfig,

    #[serde(default)]
    pub experimental: ExperimentalFlags,
}

/// API connection settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// Deepgram API key. Overridden by DEEPGRAM_API_KEY env var or the interactive 'k' command.
    /// Valid values: any valid Deepgram API key string.
    pub key: Option<String>,

    /// Deepgram TTS endpoint URL.
    /// Valid values: any valid HTTPS URL pointing to a Deepgram-compatible TTS endpoint.
    /// Default: "https://api.deepgram.com/v1/speak"
    pub endpoint: Option<String>,
}

/// Audio output settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    /// TTS audio encoding format.
    /// Valid values: mp3, linear16, mulaw, alaw, opus, flac, aac
    /// Overridden by --audio-format CLI flag or DEEPGRAM_AUDIO_FORMAT env var.
    /// Default: mp3
    pub format: Option<String>,

    /// TTS output sample rate in Hz.
    /// Valid values depend on the chosen format — see documentation.
    /// Overridden by --sample-rate CLI flag or DEEPGRAM_SAMPLE_RATE env var.
    /// Default: format-dependent (e.g. 22050 for mp3, 24000 for linear16)
    pub sample_rate: Option<u32>,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self { format: None, sample_rate: None }
    }
}

/// Feature flags for in-development functionality.
/// Set a flag to `true` to opt in. Flagged features may be incomplete or unstable.
/// Each flag can also be overridden by an environment variable:
///   TTS_TUI_FEATURE_<FLAG_NAME_UPPERCASE>=true|false
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentalFlags {
    /// Stream audio playback as bytes arrive instead of waiting for the full download.
    /// Env var override: TTS_TUI_FEATURE_STREAMING_PLAYBACK=true|false
    /// Default: false
    pub streaming_playback: bool,

    /// Allow SSML markup tags in text input for fine-grained speech control.
    /// Env var override: TTS_TUI_FEATURE_SSML_SUPPORT=true|false
    /// Default: false
    pub ssml_support: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            api: ApiConfig::default(),
            audio: AudioConfig::default(),
            experimental: ExperimentalFlags::default(),
        }
    }
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            key: None,
            endpoint: None,
        }
    }
}

impl Default for ExperimentalFlags {
    fn default() -> Self {
        Self {
            streaming_playback: false,
            ssml_support: false,
        }
    }
}

/// The default config file content, written on first run.
/// Written as a string so comments are preserved in the file.
const DEFAULT_CONFIG: &str = r#"# tts-tui configuration
# Located at ~/.config/tts-tui.toml
#
# Priority order for all settings (highest wins):
#   CLI arguments > environment variables > this file > built-in defaults

# [api] — Deepgram API connection settings
[api]
# Your Deepgram API key.
# Can also be set via the DEEPGRAM_API_KEY environment variable,
# or entered interactively at runtime with the 'k' key.
# key = "your-api-key-here"

# Custom TTS endpoint URL.
# Useful for self-hosted or proxy deployments.
# Can also be set via --endpoint CLI flag or DEEPGRAM_TTS_ENDPOINT env var.
# Default: "https://api.deepgram.com/v1/speak"
# endpoint = "https://api.deepgram.com/v1/speak"


# [audio] — Audio output settings
[audio]
# TTS audio encoding format.
# Valid values: mp3, linear16, mulaw, alaw, opus, flac, aac
# Can also be set via --audio-format CLI flag or DEEPGRAM_AUDIO_FORMAT env var.
# Default: mp3
# format = "mp3"

# TTS output sample rate in Hz.
# Valid values depend on the chosen format — see documentation.
# Can also be set via --sample-rate CLI flag or DEEPGRAM_SAMPLE_RATE env var.
# Default: format-dependent (e.g. 22050 for mp3, 24000 for linear16)
# sample_rate = 22050


# [experimental] — Feature flags for in-development functionality.
# Set a flag to true to opt in. Features may be incomplete or unstable.
# Each flag can also be overridden by an environment variable:
#   TTS_TUI_FEATURE_<FLAG_NAME_UPPERCASE>=true|false
[experimental]
# Stream audio playback as bytes arrive instead of waiting for the full download.
# Env var: TTS_TUI_FEATURE_STREAMING_PLAYBACK=true|false
streaming_playback = false

# Allow SSML markup tags in text input for fine-grained speech control.
# Env var: TTS_TUI_FEATURE_SSML_SUPPORT=true|false
ssml_support = false
"#;

fn get_config_path() -> Option<PathBuf> {
    // Use the system home directory directly; ~/.config is the conventional XDG location
    std::env::var("HOME").ok().map(|h| PathBuf::from(h).join(".config").join("tts-tui.toml"))
}

/// Load the application config from ~/.config/tts-tui.toml.
/// Creates the file with defaults and comments if it does not exist.
/// Applies environment variable overrides after loading.
pub fn load() -> AppConfig {
    let path = match get_config_path() {
        Some(p) => p,
        None => return AppConfig::default(),
    };

    // Create default config file on first run
    if !path.exists() {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(&path, DEFAULT_CONFIG);
    }

    let mut config = match fs::read_to_string(&path) {
        Ok(contents) => toml::from_str::<AppConfig>(&contents).unwrap_or_else(|e| {
            eprintln!("Warning: failed to parse {}: {}. Using defaults.", path.display(), e);
            AppConfig::default()
        }),
        Err(e) => {
            eprintln!("Warning: failed to read {}: {}. Using defaults.", path.display(), e);
            AppConfig::default()
        }
    };

    apply_env_overrides(&mut config);
    config
}

fn apply_env_overrides(config: &mut AppConfig) {
    if let Ok(val) = std::env::var("DEEPGRAM_AUDIO_FORMAT") {
        config.audio.format = Some(val);
    }
    if let Ok(val) = std::env::var("DEEPGRAM_SAMPLE_RATE") {
        if let Ok(rate) = val.parse::<u32>() {
            config.audio.sample_rate = Some(rate);
        }
    }
    if let Ok(val) = std::env::var("TTS_TUI_FEATURE_STREAMING_PLAYBACK") {
        config.experimental.streaming_playback = parse_bool_env(&val);
    }
    if let Ok(val) = std::env::var("TTS_TUI_FEATURE_SSML_SUPPORT") {
        config.experimental.ssml_support = parse_bool_env(&val);
    }
}

fn parse_bool_env(val: &str) -> bool {
    matches!(val.to_lowercase().as_str(), "true" | "1" | "yes")
}

/// Return the path to the config file for display purposes.
pub fn config_path_display() -> String {
    get_config_path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "~/.config/tts-tui.toml".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_flags_are_disabled() {
        let flags = ExperimentalFlags::default();
        assert!(!flags.streaming_playback);
        assert!(!flags.ssml_support);
    }

    #[test]
    fn parse_bool_env_handles_variants() {
        assert!(parse_bool_env("true"));
        assert!(parse_bool_env("True"));
        assert!(parse_bool_env("1"));
        assert!(parse_bool_env("yes"));
        assert!(!parse_bool_env("false"));
        assert!(!parse_bool_env("0"));
        assert!(!parse_bool_env("no"));
        assert!(!parse_bool_env(""));
    }

    #[test]
    fn default_config_is_valid_toml() {
        let result = toml::from_str::<AppConfig>(DEFAULT_CONFIG);
        assert!(result.is_ok(), "Default config template must parse cleanly: {:?}", result.err());
    }
}
