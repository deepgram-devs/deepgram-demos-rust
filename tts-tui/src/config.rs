use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

pub const DEFAULT_TTS_PROVIDER: &str = "deepgram";

/// Top-level application configuration loaded from ~/.config/deepgram/deepgram-tts-client.toml.
/// Priority order (highest to lowest): CLI args > env vars > TOML config > defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub api: ApiConfig,

    #[serde(default)]
    pub sagemaker: SageMakerConfig,

    #[serde(default)]
    pub audio: AudioConfig,

    #[serde(default)]
    pub logging: LoggingConfig,

    #[serde(default)]
    pub experimental: ExperimentalFlags,
}

/// API connection settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// TTS provider.
    /// Valid values: deepgram, sagemaker
    /// Overridden by --provider CLI flag or TTS_TUI_PROVIDER env var.
    /// Default: deepgram
    pub provider: Option<String>,

    /// Deepgram API key. Overridden by DEEPGRAM_API_KEY env var or the interactive 'k' command.
    /// Valid values: any valid Deepgram API key string. Leave unset for endpoints that do not require authentication.
    pub key: Option<String>,

    /// Deepgram TTS endpoint URL.
    /// Valid values: any valid HTTP or HTTPS URL pointing to a hosted or self-hosted Deepgram-compatible TTS endpoint.
    /// Host-only URLs such as https://api.eu.deepgram.com automatically use /v1/speak.
    /// Default: "https://api.deepgram.com/v1/speak"
    pub endpoint: Option<String>,
}

/// Amazon SageMaker settings for invoking self-hosted Deepgram TTS through AWS.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SageMakerConfig {
    /// SageMaker endpoint name running a Deepgram TTS model.
    /// Valid values: an endpoint in the configured AWS region.
    /// Overridden by --sagemaker-endpoint-name CLI flag or SAGEMAKER_ENDPOINT_NAME env var.
    pub endpoint_name: Option<String>,

    /// AWS region for the SageMaker endpoint.
    /// Valid values: any AWS region identifier, such as us-east-2.
    /// Overridden by --aws-region CLI flag, AWS_REGION, or AWS_DEFAULT_REGION env vars.
    /// Default: us-east-2
    pub region: Option<String>,
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

    /// Normalize the volume of generated audio.
    /// Valid values: true or false.
    /// Overridden by DEEPGRAM_NORMALIZE_VOLUME or toggled at runtime with the 'v' key.
    /// Default: false
    #[serde(default)]
    pub normalize_volume: bool,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            format: None,
            sample_rate: None,
            normalize_volume: false,
        }
    }
}

/// Persistent application log settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Maximum size in bytes of the active log before rotation.
    /// Valid values: positive integer. Overridden by --log-max-size-bytes or TTS_TUI_LOG_MAX_SIZE_BYTES.
    /// Default: 1048576 (1 MiB)
    #[serde(default = "default_log_max_size_bytes")]
    pub max_size_bytes: u64,

    /// Maximum number of log files to retain, including the active log.
    /// Valid values: positive integer. Overridden by --log-max-files or TTS_TUI_LOG_MAX_FILES.
    /// Default: 3
    #[serde(default = "default_log_max_files")]
    pub max_files: usize,
}

const fn default_log_max_size_bytes() -> u64 {
    1024 * 1024
}

const fn default_log_max_files() -> usize {
    3
}

/// Feature flags for in-development functionality.
/// Set a flag to `true` to opt in. Flagged features may be incomplete or unstable.
/// Each flag can also be overridden by an environment variable:
///   TTS_TUI_FEATURE_<FLAG_NAME_UPPERCASE>=true|false
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentalFlags {
    /// Enable Deepgram WebSocket TTS streaming on startup.
    /// Toggle this at runtime with the 'w' key; hosted endpoints require an API key, while self-hosted endpoints may not.
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
            sagemaker: SageMakerConfig::default(),
            audio: AudioConfig::default(),
            logging: LoggingConfig::default(),
            experimental: ExperimentalFlags::default(),
        }
    }
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            provider: None,
            key: None,
            endpoint: None,
        }
    }
}

impl Default for SageMakerConfig {
    fn default() -> Self {
        Self {
            endpoint_name: None,
            region: None,
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            max_size_bytes: default_log_max_size_bytes(),
            max_files: default_log_max_files(),
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
# Located at ~/.config/deepgram/deepgram-tts-client.toml
#
# Priority order for all settings (highest wins):
#   CLI arguments > environment variables > this file > built-in defaults

# [api] — Deepgram API connection settings
[api]
# TTS provider.
# Valid values: "deepgram", "sagemaker".
# Use "deepgram" for hosted or self-hosted Deepgram-compatible HTTP endpoints,
# or "sagemaker" for self-hosted Deepgram deployed as an Amazon SageMaker endpoint.
# Can also be set via --provider CLI flag or TTS_TUI_PROVIDER env var.
# Default: "deepgram"
# provider = "deepgram"

# Your Deepgram API key.
# Can also be set via the DEEPGRAM_API_KEY environment variable,
# or entered interactively at runtime with the 'k' key.
# Leave unset for Deepgram-compatible HTTP endpoints that do not require authentication.
# SageMaker mode uses AWS credentials instead.
# key = "your-api-key-here"

# Custom TTS endpoint URL.
# Useful for hosted, self-hosted, proxy, or non-production Deepgram-compatible deployments.
# Host-only URLs such as "https://api.eu.deepgram.com" automatically use "/v1/speak".
# Can also be set via --endpoint CLI flag or DEEPGRAM_TTS_ENDPOINT env var.
# Default: "https://api.deepgram.com/v1/speak"
# endpoint = "https://api.deepgram.com/v1/speak"


# [sagemaker] — AWS SageMaker settings for self-hosted Deepgram TTS.
[sagemaker]
# SageMaker endpoint name running a Deepgram TTS model.
# Valid values: an endpoint name in the configured AWS region.
# Can also be set via --sagemaker-endpoint-name CLI flag or
# SAGEMAKER_ENDPOINT_NAME env var.
# Required when provider = "sagemaker".
# endpoint_name = "your-sagemaker-endpoint"

# AWS region for the SageMaker endpoint.
# Valid values: any AWS region identifier, such as "us-east-2".
# Can also be set via --aws-region CLI flag, AWS_REGION, or AWS_DEFAULT_REGION.
# Default: "us-east-2"
# region = "us-east-2"


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

# Normalize the volume of generated audio through the Deepgram TTS API.
# Valid values: true or false.
# Can also be set via DEEPGRAM_NORMALIZE_VOLUME=true, or toggled while the app runs with 'v'.
# Default: false
normalize_volume = false


# [logging] — Persistent application log settings
[logging]
# Maximum size of the active tts-tui log in bytes before rotation.
# Valid values: positive integer. Can also be set via --log-max-size-bytes or
# TTS_TUI_LOG_MAX_SIZE_BYTES. Default: 1048576 (1 MiB).
max_size_bytes = 1048576

# Maximum number of tts-tui log files to retain, including the active log.
# Valid values: positive integer. Can also be set via --log-max-files or
# TTS_TUI_LOG_MAX_FILES. Default: 3.
max_files = 3


# [experimental] — Feature flags for in-development functionality.
# Set a flag to true to opt in. Features may be incomplete or unstable.
# Each flag can also be overridden by an environment variable:
#   TTS_TUI_FEATURE_<FLAG_NAME_UPPERCASE>=true|false
[experimental]
# Enable Deepgram WebSocket TTS streaming on startup.
# Hosted endpoints require a Deepgram API key; self-hosted endpoints may not. Toggle with 'w'.
# Env var: TTS_TUI_FEATURE_STREAMING_PLAYBACK=true|false
streaming_playback = false

# Allow SSML markup tags in text input for fine-grained speech control.
# Env var: TTS_TUI_FEATURE_SSML_SUPPORT=true|false
ssml_support = false
"#;

const CONFIG_FILE_NAME: &str = "deepgram-tts-client.toml";

fn get_config_path() -> Option<PathBuf> {
    home_directory().map(config_path_from_home)
}

pub fn config_directory() -> Option<PathBuf> {
    home_directory().map(|home| home.join(".config").join("deepgram"))
}

fn config_path_from_home(home: PathBuf) -> PathBuf {
    home.join(".config").join("deepgram").join(CONFIG_FILE_NAME)
}

fn get_legacy_config_path() -> Option<PathBuf> {
    home_directory().map(|home| home.join(".config").join(CONFIG_FILE_NAME))
}

/// Resolve the user's home directory across Unix shells and native Windows.
/// Windows applications normally receive USERPROFILE rather than HOME.
fn home_directory() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .or_else(|| {
            let drive = std::env::var_os("HOMEDRIVE")?;
            let path = std::env::var_os("HOMEPATH")?;
            Some(std::ffi::OsString::from(format!(
                "{}{}",
                drive.to_string_lossy(),
                path.to_string_lossy()
            )))
        })
        .map(PathBuf::from)
}

fn migrate_legacy_config(path: &PathBuf) {
    let Some(legacy_path) = get_legacy_config_path() else {
        return;
    };
    if path.exists() || !legacy_path.exists() {
        return;
    }

    if let Some(parent) = path.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            eprintln!(
                "Warning: failed to create config directory {}: {}",
                parent.display(),
                error
            );
            return;
        }
    }
    if let Err(error) = fs::rename(&legacy_path, path) {
        eprintln!(
            "Warning: failed to move legacy config {} to {}: {}. Keeping the legacy file.",
            legacy_path.display(),
            path.display(),
            error
        );
    }
}

/// Load the application config from ~/.config/deepgram/deepgram-tts-client.toml.
/// Creates the file with defaults and comments if it does not exist.
/// Applies environment variable overrides after loading.
pub fn load() -> AppConfig {
    let mut config = if let Some(path) = get_config_path() {
        migrate_legacy_config(&path);

        // Create default config file on first run
        if !path.exists() {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::write(&path, DEFAULT_CONFIG);
        }

        match fs::read_to_string(&path) {
            Ok(contents) => toml::from_str::<AppConfig>(&contents).unwrap_or_else(|e| {
                eprintln!(
                    "Warning: failed to parse {}: {}. Using defaults.",
                    path.display(),
                    e
                );
                AppConfig::default()
            }),
            Err(e) => {
                eprintln!(
                    "Warning: failed to read {}: {}. Using defaults.",
                    path.display(),
                    e
                );
                AppConfig::default()
            }
        }
    } else {
        // Still apply environment variables when the platform does not expose
        // a usable home directory (for example, a restricted Windows process).
        AppConfig::default()
    };

    apply_env_overrides(&mut config);
    normalize_provider(&mut config);
    config
}

fn apply_env_overrides(config: &mut AppConfig) {
    if let Ok(val) = std::env::var("TTS_TUI_PROVIDER") {
        config.api.provider = Some(val);
    }
    if let Ok(val) = std::env::var("DEEPGRAM_API_KEY") {
        config.api.key = normalized_api_key(Some(&val));
    }
    if let Ok(val) = std::env::var("DEEPGRAM_AUDIO_FORMAT") {
        config.audio.format = Some(val);
    }
    if let Ok(val) = std::env::var("DEEPGRAM_SAMPLE_RATE") {
        if let Ok(rate) = val.parse::<u32>() {
            config.audio.sample_rate = Some(rate);
        }
    }
    if let Ok(val) = std::env::var("DEEPGRAM_NORMALIZE_VOLUME") {
        config.audio.normalize_volume = parse_bool_env(&val);
    }
    if let Ok(val) = std::env::var("TTS_TUI_LOG_MAX_SIZE_BYTES") {
        if let Ok(size) = val.parse::<u64>() {
            config.logging.max_size_bytes = size.max(1);
        }
    }
    if let Ok(val) = std::env::var("TTS_TUI_LOG_MAX_FILES") {
        if let Ok(count) = val.parse::<usize>() {
            config.logging.max_files = count.max(1);
        }
    }
    if let Ok(val) = std::env::var("TTS_TUI_FEATURE_STREAMING_PLAYBACK") {
        config.experimental.streaming_playback = parse_bool_env(&val);
    }
    if let Ok(val) = std::env::var("TTS_TUI_FEATURE_SSML_SUPPORT") {
        config.experimental.ssml_support = parse_bool_env(&val);
    }
    if let Ok(val) = std::env::var("SAGEMAKER_ENDPOINT_NAME") {
        config.sagemaker.endpoint_name = Some(val);
    }
    if let Ok(val) = std::env::var("AWS_REGION") {
        config.sagemaker.region = Some(val);
    } else if let Ok(val) = std::env::var("AWS_DEFAULT_REGION") {
        config.sagemaker.region = Some(val);
    }
}

pub fn normalized_tts_provider(provider: Option<&str>) -> String {
    let provider = provider
        .map(str::trim)
        .filter(|provider| !provider.is_empty())
        .unwrap_or(DEFAULT_TTS_PROVIDER)
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '_' || *ch == '-')
        .collect::<String>()
        .to_ascii_lowercase();

    if provider.is_empty() {
        DEFAULT_TTS_PROVIDER.to_string()
    } else {
        provider
    }
}

pub fn normalize_provider(config: &mut AppConfig) {
    config.api.provider = Some(normalized_tts_provider(config.api.provider.as_deref()));
}

pub fn normalized_api_key(key: Option<&str>) -> Option<String> {
    key.map(str::trim)
        .filter(|key| !key.is_empty())
        .map(ToString::to_string)
}

fn parse_bool_env(val: &str) -> bool {
    matches!(val.to_lowercase().as_str(), "true" | "1" | "yes")
}

/// Return the path to the config file for display purposes.
pub fn config_path_display() -> String {
    get_config_path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "~/.config/deepgram/deepgram-tts-client.toml".to_string())
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
    fn default_audio_config_disables_volume_normalization() {
        assert!(!AudioConfig::default().normalize_volume);
    }

    #[test]
    fn default_logging_config_rotates_one_megabyte_across_three_files() {
        let logging = LoggingConfig::default();
        assert_eq!(logging.max_size_bytes, 1024 * 1024);
        assert_eq!(logging.max_files, 3);
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
        assert!(
            result.is_ok(),
            "Default config template must parse cleanly: {:?}",
            result.err()
        );
    }

    #[test]
    fn config_path_is_scoped_to_deepgram_directory() {
        assert_eq!(
            config_path_from_home(PathBuf::from("/example/home")),
            PathBuf::from("/example/home/.config/deepgram/deepgram-tts-client.toml")
        );
    }

    #[test]
    fn normalized_tts_provider_defaults_and_trims() {
        assert_eq!(normalized_tts_provider(None), DEFAULT_TTS_PROVIDER);
        assert_eq!(normalized_tts_provider(Some("")), DEFAULT_TTS_PROVIDER);
        assert_eq!(normalized_tts_provider(Some(" \n")), DEFAULT_TTS_PROVIDER);
        assert_eq!(
            normalized_tts_provider(Some("\u{200b}")),
            DEFAULT_TTS_PROVIDER
        );
        assert_eq!(normalized_tts_provider(Some(" Deepgram\n")), "deepgram");
        assert_eq!(
            normalized_tts_provider(Some("deepgram\u{200b}")),
            "deepgram"
        );
        assert_eq!(normalized_tts_provider(Some(" SAGEMAKER\t")), "sagemaker");
    }

    #[test]
    fn normalize_provider_stores_normalized_value() {
        let mut config = AppConfig::default();
        config.api.provider = Some(" Deepgram\n".to_string());

        normalize_provider(&mut config);

        assert_eq!(config.api.provider.as_deref(), Some(DEFAULT_TTS_PROVIDER));
    }

    #[test]
    fn normalized_api_key_trims_and_rejects_empty_values() {
        assert_eq!(normalized_api_key(None), None);
        assert_eq!(normalized_api_key(Some("")), None);
        assert_eq!(normalized_api_key(Some(" \n")), None);
        assert_eq!(
            normalized_api_key(Some("  dg-key-value\n")).as_deref(),
            Some("dg-key-value")
        );
    }
}
