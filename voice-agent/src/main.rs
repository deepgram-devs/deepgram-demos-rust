use std::env;
use std::io::{self, Write};

use std::time::Duration;

use clap::{Parser, Subcommand};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, Stream, StreamConfig};
use futures_util::{SinkExt, StreamExt};
use log::{debug, error, info};
use rodio::{OutputStream, Sink, Source};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc as std_mpsc;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tokio::time::sleep;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use url::Url;

const DEFAULT_SYSTEM_PROMPT: &str =
    "Keep your responses concise and focused. Answer in as few words as possible while remaining helpful.";

#[derive(Parser, Debug)]
#[command(name = "voice-agent")]
#[command(about = "A Deepgram Voice Agent client")]
#[command(version)]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,

    #[command(flatten)]
    launch: LaunchOptions,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Manage reusable Deepgram agent configurations
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
}

#[derive(Subcommand, Debug)]
enum ConfigCommand {
    /// Create an agent configuration without opening a voice connection
    Create(ConfigCreateArgs),
    /// Open a voice connection using a saved agent configuration UUID
    Use(ConfigUseArgs),
    /// Delete a saved agent configuration
    Delete(ConfigDeleteArgs),
    /// Manage reusable agent template variables
    #[command(alias = "variables")]
    Variable {
        #[command(subcommand)]
        command: ConfigVariableCommand,
    },
}

#[derive(Parser, Debug)]
struct ConfigCreateArgs {
    /// Deepgram project ID where the configuration will be saved
    #[arg(long, env = "DEEPGRAM_PROJECT_ID")]
    project_id: Option<String>,

    /// Metadata name for the saved configuration
    #[arg(long, value_name = "NAME")]
    name: String,

    #[command(flatten)]
    launch: LaunchOptions,
}

#[derive(Parser, Debug)]
struct ConfigUseArgs {
    /// UUID returned when the reusable configuration was saved
    #[arg(value_name = "AGENT_CONFIG_ID")]
    agent_config_id: String,

    #[command(flatten)]
    launch: LaunchOptions,
}

#[derive(Parser, Debug)]
struct ConfigDeleteArgs {
    /// UUID returned when the reusable configuration was saved
    #[arg(value_name = "AGENT_CONFIG_ID")]
    agent_config_id: String,

    /// Deepgram project containing the configuration
    #[arg(long, env = "DEEPGRAM_PROJECT_ID")]
    project_id: Option<String>,

    /// Skip the interactive deletion confirmation
    #[arg(long)]
    yes: bool,

    /// Print the exact API URL and payload
    #[arg(long)]
    verbose: bool,
}

#[derive(Subcommand, Debug)]
enum ConfigVariableCommand {
    /// Create a template variable
    Create(ConfigVariableCreateArgs),
    /// List template variables
    List(ConfigVariableProjectArgs),
    /// Get a template variable
    Get(ConfigVariableGetArgs),
    /// Update a template variable value
    Update(ConfigVariableUpdateArgs),
    /// Delete a template variable
    Delete(ConfigVariableDeleteArgs),
}

#[derive(Parser, Debug)]
struct ConfigVariableProjectArgs {
    /// Deepgram project containing the variable
    #[arg(long, env = "DEEPGRAM_PROJECT_ID")]
    project_id: Option<String>,

    /// Print the exact API URL and payload
    #[arg(long)]
    verbose: bool,
}

#[derive(Parser, Debug)]
struct ConfigVariableCreateArgs {
    /// Variable key, for example DG_SYSTEM_PROMPT
    #[arg(long)]
    key: String,

    /// Value to store; valid JSON is preserved, otherwise the value is stored as a string
    #[arg(long)]
    value: String,

    #[command(flatten)]
    project: ConfigVariableProjectArgs,
}

#[derive(Parser, Debug)]
struct ConfigVariableGetArgs {
    /// Variable ID returned by the API
    #[arg(value_name = "VARIABLE_ID")]
    variable_id: String,

    #[command(flatten)]
    project: ConfigVariableProjectArgs,
}

#[derive(Parser, Debug)]
struct ConfigVariableUpdateArgs {
    /// Variable ID returned by the API
    #[arg(value_name = "VARIABLE_ID")]
    variable_id: String,

    /// New value; valid JSON is preserved, otherwise the value is stored as a string
    #[arg(long)]
    value: String,

    #[command(flatten)]
    project: ConfigVariableProjectArgs,
}

#[derive(Parser, Debug)]
struct ConfigVariableDeleteArgs {
    /// Variable ID returned by the API
    #[arg(value_name = "VARIABLE_ID")]
    variable_id: String,

    #[command(flatten)]
    project: ConfigVariableProjectArgs,

    /// Skip the interactive deletion confirmation
    #[arg(long)]
    yes: bool,
}

#[derive(Parser, Debug)]
struct LaunchOptions {
    /// Custom Deepgram endpoint URL to connect to
    #[arg(long, default_value = "wss://agent.deepgram.com")]
    endpoint: String,

    /// Speak provider type (deepgram or eleven_labs)
    #[arg(long, default_value = "deepgram")]
    speak_provider: String,

    /// Speak provider model to use for text-to-speech (Deepgram model name)
    #[arg(long, default_value = "aura-2-thalia-en")]
    speak_model: String,

    /// Speak provider model ID (for Eleven Labs: e.g. eleven_turbo_v2_5)
    #[arg(long, default_value = "eleven_turbo_v2_5")]
    speak_model_id: String,

    /// Speak language code (for Eleven Labs: e.g. en-US)
    #[arg(long, default_value = "en-US")]
    speak_language_code: String,

    /// Listen provider type to use for speech-to-text
    #[arg(long, default_value = "deepgram")]
    listen_provider: String,

    /// Listen provider model to use for speech-to-text
    #[arg(long, default_value = "nova-3")]
    listen_model: String,

    /// Listen provider model version
    #[arg(long)]
    listen_version: Option<String>,

    /// Listen provider language code
    #[arg(long, default_value = "en")]
    listen_language: String,

    /// Listen provider language hints as comma-separated language codes
    #[arg(long = "language-hint", value_delimiter = ',')]
    language_hints: Vec<String>,

    /// Listen provider keyterms as comma-separated values
    #[arg(long, value_delimiter = ',')]
    listen_keyterms: Vec<String>,

    /// Listen provider end-of-turn threshold
    #[arg(long)]
    listen_eot_threshold: Option<f32>,

    /// Listen provider eager end-of-turn threshold
    #[arg(long)]
    listen_eager_eot_threshold: Option<f32>,

    /// Listen provider smart formatting; omit this option to leave it out of the Settings JSON
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    listen_smart_format: Option<bool>,

    /// Eleven Labs voice ID (used in the endpoint URL)
    #[arg(long)]
    speak_voice_id: Option<String>,

    /// Think provider type to use for LLM processing
    #[arg(long, default_value = "open_ai")]
    think_type: String,

    /// Think provider model to use for LLM processing
    #[arg(long, default_value = "gpt-4o-mini")]
    think_model: String,

    /// Think provider temperature
    #[arg(long)]
    think_temperature: Option<f32>,

    /// Custom endpoint URL for think provider
    #[arg(long)]
    think_endpoint: Option<String>,

    /// Custom headers for think provider in format "key=value" (can be specified multiple times)
    #[arg(long)]
    think_header: Vec<String>,

    /// AWS Bedrock credential type: iam or sts
    #[arg(long, value_parser = ["iam", "sts"])]
    think_credentials_type: Option<String>,

    /// AWS Bedrock region
    #[arg(long, env = "AWS_REGION")]
    think_aws_region: Option<String>,

    /// AWS Bedrock access key ID
    #[arg(long, env = "AWS_ACCESS_KEY_ID")]
    think_aws_access_key_id: Option<String>,

    /// AWS Bedrock secret access key
    #[arg(long, env = "AWS_SECRET_ACCESS_KEY")]
    think_aws_secret_access_key: Option<String>,

    /// AWS STS session token, required for temporary STS credentials
    #[arg(long, env = "AWS_SESSION_TOKEN")]
    think_aws_session_token: Option<String>,

    /// Agent system prompt / instructions
    #[arg(long)]
    prompt: Option<String>,

    /// Enable verbose output including full Settings JSON message and request ID
    #[arg(long)]
    verbose: bool,

    /// Disable microphone muting during agent audio playback
    #[arg(long)]
    no_mic_mute: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct VoiceAgentConfig {
    #[serde(rename = "type")]
    message_type: String,
    tags: Vec<String>,
    audio: AudioSettings,
    agent: AgentConfiguration,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum AgentConfiguration {
    Inline(AgentSettings),
    Reference(String),
}

#[derive(Debug, Serialize, Deserialize)]
struct AudioSettings {
    input: AudioInputConfig,
    output: AudioOutputConfig,
}

#[derive(Debug, Serialize, Deserialize)]
struct AudioInputConfig {
    encoding: String,
    sample_rate: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct AudioOutputConfig {
    encoding: String,
    sample_rate: u32,
    container: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct AgentSettings {
    listen: ListenConfig,
    think: ThinkConfig,
    speak: SpeakConfig,
}

#[derive(Debug, Serialize, Deserialize)]
struct ListenConfig {
    provider: ListenProviderConfig,
}

#[derive(Debug, Serialize, Deserialize)]
struct ThinkConfig {
    provider: ThinkProviderConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    endpoint: Option<ThinkEndpointConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ThinkEndpointConfig {
    url: String,
    headers: std::collections::HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SpeakConfig {
    provider: SpeakProviderConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    endpoint: Option<SpeakEndpointConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SpeakEndpointConfig {
    url: String,
    headers: std::collections::HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ListenProviderConfig {
    #[serde(rename = "type")]
    provider_type: String,
    model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    language_hints: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    keyterms: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    eot_threshold: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    eager_eot_threshold: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    smart_format: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ThinkProviderConfig {
    #[serde(rename = "type")]
    provider_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    credentials: Option<ThinkCredentials>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ThinkCredentials {
    #[serde(rename = "type")]
    credential_type: String,
    region: String,
    access_key_id: String,
    secret_access_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_token: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SpeakProviderConfig {
    #[serde(rename = "type")]
    provider_type: String,
    /// Deepgram model name (used when provider_type is "deepgram")
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    /// Eleven Labs model ID (used when provider_type is "eleven_labs")
    #[serde(skip_serializing_if = "Option::is_none")]
    model_id: Option<String>,
    /// Language code (used when provider_type is "eleven_labs")
    #[serde(skip_serializing_if = "Option::is_none")]
    language_code: Option<String>,
}

#[derive(Debug, Deserialize)]
struct VoiceAgentResponse {
    #[serde(rename = "type")]
    message_type: String,
    #[serde(flatten)]
    data: serde_json::Value,
}

struct AudioCapture {
    device: Device,
    config: StreamConfig,
    sample_format: SampleFormat,
}

impl AudioCapture {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or("No input device available")?;

        debug!("Input device: {}", device.name()?);

        let supported_config = device.default_input_config()?;
        debug!("Default input config: {:?}", supported_config);

        let sample_format = supported_config.sample_format();
        let config: StreamConfig = supported_config.into();

        Ok(AudioCapture {
            device,
            config,
            sample_format,
        })
    }

    fn start_capture(
        &self,
        tx: mpsc::UnboundedSender<Vec<u8>>,
        mic_enabled: Arc<AtomicBool>,
    ) -> Result<Stream, Box<dyn std::error::Error>> {
        let config = self.config.clone();

        let stream = match self.sample_format {
            SampleFormat::F32 => self.build_stream::<f32>(config, tx, mic_enabled)?,
            SampleFormat::I16 => self.build_stream::<i16>(config, tx, mic_enabled)?,
            SampleFormat::U16 => self.build_stream::<u16>(config, tx, mic_enabled)?,
            _ => return Err("Unsupported sample format".into()),
        };

        stream.play()?;
        Ok(stream)
    }

    fn build_stream<T>(
        &self,
        config: StreamConfig,
        tx: mpsc::UnboundedSender<Vec<u8>>,
        mic_enabled: Arc<AtomicBool>,
    ) -> Result<Stream, Box<dyn std::error::Error>>
    where
        T: cpal::Sample + cpal::SizedSample + Send + 'static,
        f32: cpal::FromSample<T>,
    {
        let stream = self.device.build_input_stream(
            &config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                let audio_data = if mic_enabled.load(Ordering::Relaxed) {
                    // Convert real mic samples to linear16 for Deepgram
                    let mut buf = Vec::with_capacity(data.len() * 2);
                    for &sample in data.iter() {
                        let f32_sample: f32 = cpal::Sample::from_sample(sample);
                        let i16_sample = (f32_sample * i16::MAX as f32) as i16;
                        buf.extend_from_slice(&i16_sample.to_le_bytes());
                    }
                    buf
                } else {
                    // Mic is muted — send silence to keep the connection alive
                    vec![0u8; data.len() * 2]
                };

                if let Err(_e) = tx.send(audio_data) {
                    // Channel closed during shutdown, expected
                }
            },
            |err| error!("Audio stream error: {}", err),
            None,
        )?;

        Ok(stream)
    }
}

struct AudioPlayer {
    _stream_handle: OutputStream,
    sink: Arc<Sink>,
    mic_enabled: Arc<AtomicBool>,
    mute_on_playback: bool,
}

impl AudioPlayer {
    fn new(
        mic_enabled: Arc<AtomicBool>,
        mute_on_playback: bool,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let stream_handle = rodio::OutputStreamBuilder::open_default_stream()
            .map_err(|e| format!("Failed to create audio output stream: {}", e))?;
        let sink = Arc::new(Sink::connect_new(&stream_handle.mixer()));

        if mute_on_playback {
            // Background thread polls the sink to detect when audio finishes and
            // re-enables the microphone after a short silence period.
            let mic_enabled_clone = Arc::clone(&mic_enabled);
            let sink_clone = Arc::clone(&sink);
            std::thread::spawn(move || {
                let mut playback_ended_at: Option<Instant> = None;

                loop {
                    std::thread::sleep(Duration::from_millis(50));

                    let is_playing = !sink_clone.empty();

                    if is_playing {
                        // Audio is actively playing — keep mic disabled and reset timer
                        if mic_enabled_clone.load(Ordering::Relaxed) {
                            mic_enabled_clone.store(false, Ordering::Relaxed);
                            debug!("🎤 Microphone disabled — audio playing");
                        }
                        playback_ended_at = None;
                    } else {
                        // Sink is empty (no audio playing)
                        match playback_ended_at {
                            None => {
                                // If mic is still disabled, audio just finished — start the silence timer
                                if !mic_enabled_clone.load(Ordering::Relaxed) {
                                    playback_ended_at = Some(Instant::now());
                                    debug!("🔊 Audio playback ended, waiting 600ms before re-enabling mic");
                                }
                            }
                            Some(ended_at) => {
                                if ended_at.elapsed() >= Duration::from_millis(600) {
                                    mic_enabled_clone.store(true, Ordering::Relaxed);
                                    debug!("🎤 Microphone re-enabled after 600ms of silence");
                                    playback_ended_at = None;
                                }
                            }
                        }
                    }
                }
            });
        }

        Ok(AudioPlayer {
            _stream_handle: stream_handle,
            sink,
            mic_enabled,
            mute_on_playback,
        })
    }

    fn play_audio(&self, audio_data: Vec<u8>) -> Result<(), Box<dyn std::error::Error>> {
        debug!(
            "🔊 Received audio data for playback: {} bytes",
            audio_data.len()
        );

        if audio_data.is_empty() {
            return Ok(());
        }

        // Disable microphone immediately so we don't capture our own output
        if self.mute_on_playback {
            self.mic_enabled.store(false, Ordering::Relaxed);
        }

        // Convert linear16 PCM bytes to f32 samples for rodio
        let mut samples = Vec::with_capacity(audio_data.len() / 2);
        for chunk in audio_data.chunks_exact(2) {
            let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
            samples.push(sample as f32 / i16::MAX as f32);
        }

        self.sink.append(PCMSource::new(samples, 24000, 1));

        Ok(())
    }
}

// Custom PCM source for rodio
struct PCMSource {
    samples: std::vec::IntoIter<f32>,
    sample_rate: u32,
    channels: u16,
}

impl PCMSource {
    fn new(samples: Vec<f32>, sample_rate: u32, channels: u16) -> Self {
        Self {
            samples: samples.into_iter(),
            sample_rate,
            channels,
        }
    }
}

impl Iterator for PCMSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        self.samples.next()
    }
}

impl Source for PCMSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

async fn connect_to_voice_agent(
    api_key: &str,
    endpoint: &str,
    _sample_rate: u32,
    _channels: u16,
    verbose: bool,
) -> Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    Box<dyn std::error::Error>,
> {
    let url = Url::parse(format!("{0}/v1/agent/converse", endpoint).as_str())?;

    if verbose {
        info!("Voice Agent WebSocket URL: {}", url);
        info!(
            "Voice Agent WebSocket request headers: Authorization: Token <redacted>, Host: {}, Upgrade: websocket, Connection: Upgrade, Sec-WebSocket-Key: <redacted>, Sec-WebSocket-Version: 13",
            url.host_str().unwrap_or("agent.deepgram.com")
        );
    }

    let request = tokio_tungstenite::tungstenite::handshake::client::Request::get(url.as_str())
        .header("Authorization", format!("Token {}", api_key))
        .header("Host", url.host_str().unwrap_or("agent.deepgram.com"))
        .header("Upgrade", "websocket")
        .header("Connection", "Upgrade")
        .header(
            "Sec-WebSocket-Key",
            tokio_tungstenite::tungstenite::handshake::client::generate_key(),
        )
        .header("Sec-WebSocket-Version", "13")
        .body(())?;

    debug!("Connecting to Deepgram Voice Agent WebSocket...");
    let (ws_stream, _response) = connect_async(request).await?;
    debug!("Connected to Deepgram Voice Agent successfully");

    Ok(ws_stream)
}

struct SpeakArgs<'a> {
    provider: &'a str,
    model: &'a str,
    model_id: &'a str,
    language_code: &'a str,
    voice_id: Option<&'a str>,
    eleven_labs_api_key: Option<String>,
}

struct ListenArgs<'a> {
    provider: &'a str,
    model: &'a str,
    version: Option<&'a str>,
    language: &'a str,
    language_hints: &'a [String],
    keyterms: &'a [String],
    eot_threshold: Option<f32>,
    eager_eot_threshold: Option<f32>,
    smart_format: Option<bool>,
}

fn listen_language_for_model(model: &str, language: &str) -> Option<String> {
    if model.to_ascii_lowercase().starts_with("flux-") {
        None
    } else {
        Some(language.to_string())
    }
}

fn cleaned_values(values: &[String]) -> Vec<String> {
    values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .collect()
}

fn create_agent_config(
    sample_rate: u32,
    _channels: u16,
    listen: ListenArgs<'_>,
    speak: SpeakArgs<'_>,
    think_type: &str,
    think_model: &str,
    think_temperature: Option<f32>,
    think_endpoint: Option<&str>,
    think_headers: &[String],
    think_credentials_type: Option<&str>,
    think_aws_region: Option<&str>,
    think_aws_access_key_id: Option<&str>,
    think_aws_secret_access_key: Option<&str>,
    think_aws_session_token: Option<&str>,
    prompt: Option<&str>,
) -> VoiceAgentConfig {
    // Parse think headers from "key=value" format
    let mut headers = std::collections::HashMap::new();
    for header in think_headers {
        if let Some((key, value)) = header.split_once('=') {
            headers.insert(key.to_string(), value.to_string());
        }
    }

    // Create endpoint config if think_endpoint is provided
    let endpoint_config = think_endpoint.map(|url| ThinkEndpointConfig {
        url: url.to_string(),
        headers,
    });

    // Build speak config based on provider type
    let speak_config = match speak.provider {
        "eleven_labs" => {
            let voice_id = speak.voice_id.unwrap_or("rachel");
            let endpoint_url = format!(
                "wss://api.elevenlabs.io/v1/text-to-speech/{}/multi-stream-input",
                voice_id
            );
            let mut speak_headers = std::collections::HashMap::new();
            if let Some(key) = speak.eleven_labs_api_key {
                speak_headers.insert("xi-api-key".to_string(), key);
            }
            SpeakConfig {
                provider: SpeakProviderConfig {
                    provider_type: "eleven_labs".to_string(),
                    model: None,
                    model_id: Some(speak.model_id.to_string()),
                    language_code: Some(speak.language_code.to_string()),
                },
                endpoint: Some(SpeakEndpointConfig {
                    url: endpoint_url,
                    headers: speak_headers,
                }),
            }
        }
        _ => SpeakConfig {
            provider: SpeakProviderConfig {
                provider_type: "deepgram".to_string(),
                model: Some(speak.model.to_string()),
                model_id: None,
                language_code: None,
            },
            endpoint: None,
        },
    };

    VoiceAgentConfig {
        message_type: "Settings".to_string(),
        tags: vec!["demo".to_string(), "voice_agent".to_string()],
        audio: AudioSettings {
            input: AudioInputConfig {
                encoding: "linear16".to_string(),
                sample_rate,
            },
            output: AudioOutputConfig {
                encoding: "linear16".to_string(),
                sample_rate: 24000,
                container: "none".to_string(),
            },
        },
        agent: AgentConfiguration::Inline(AgentSettings {
            listen: ListenConfig {
                provider: ListenProviderConfig {
                    provider_type: listen.provider.to_string(),
                    model: listen.model.to_string(),
                    version: listen.version.map(|version| version.to_string()),
                    language: listen_language_for_model(listen.model, listen.language),
                    language_hints: cleaned_values(listen.language_hints),
                    keyterms: cleaned_values(listen.keyterms),
                    eot_threshold: listen.eot_threshold,
                    eager_eot_threshold: listen.eager_eot_threshold,
                    smart_format: listen.smart_format,
                },
            },
            think: ThinkConfig {
                provider: ThinkProviderConfig {
                    provider_type: think_type.to_string(),
                    model: if think_model.is_empty() {
                        None
                    } else {
                        Some(think_model.to_string())
                    },
                    temperature: think_temperature,
                    credentials: think_credentials_type
                        .or(think_aws_region)
                        .or(think_aws_access_key_id)
                        .or(think_aws_secret_access_key)
                        .or(think_aws_session_token)
                        .map(|_| ThinkCredentials {
                            credential_type: think_credentials_type.unwrap_or("iam").to_string(),
                            region: think_aws_region.unwrap_or_default().to_string(),
                            access_key_id: think_aws_access_key_id.unwrap_or_default().to_string(),
                            secret_access_key: think_aws_secret_access_key
                                .unwrap_or_default()
                                .to_string(),
                            session_token: think_aws_session_token.map(str::to_string),
                        }),
                },
                prompt: Some(prompt.unwrap_or(DEFAULT_SYSTEM_PROMPT).to_string()),
                endpoint: endpoint_config,
            },
            speak: speak_config,
        }),
    }
}

fn config_from_options(
    options: &LaunchOptions,
    sample_rate: u32,
    channels: u16,
    eleven_labs_api_key: Option<String>,
) -> VoiceAgentConfig {
    create_agent_config(
        sample_rate,
        channels,
        ListenArgs {
            provider: &options.listen_provider,
            model: &options.listen_model,
            version: options.listen_version.as_deref(),
            language: &options.listen_language,
            language_hints: &options.language_hints,
            keyterms: &options.listen_keyterms,
            eot_threshold: options.listen_eot_threshold,
            eager_eot_threshold: options.listen_eager_eot_threshold,
            smart_format: options.listen_smart_format,
        },
        SpeakArgs {
            provider: &options.speak_provider,
            model: &options.speak_model,
            model_id: &options.speak_model_id,
            language_code: &options.speak_language_code,
            voice_id: options.speak_voice_id.as_deref(),
            eleven_labs_api_key,
        },
        &options.think_type,
        &options.think_model,
        options.think_temperature,
        options.think_endpoint.as_deref(),
        &options.think_header,
        options.think_credentials_type.as_deref(),
        options.think_aws_region.as_deref(),
        options.think_aws_access_key_id.as_deref(),
        options.think_aws_secret_access_key.as_deref(),
        options.think_aws_session_token.as_deref(),
        options.prompt.as_deref(),
    )
}

fn validate_think_options(options: &LaunchOptions) -> Result<(), Box<dyn std::error::Error>> {
    let has_bedrock_credentials = options.think_credentials_type.is_some()
        || options.think_aws_region.is_some()
        || options.think_aws_access_key_id.is_some()
        || options.think_aws_secret_access_key.is_some()
        || options.think_aws_session_token.is_some();

    if has_bedrock_credentials && options.think_type != "aws_bedrock" {
        return Err("AWS Bedrock credentials require --think-type aws_bedrock".into());
    }

    if options.think_type != "aws_bedrock" {
        return Ok(());
    }

    if options.think_endpoint.as_deref().is_none_or(str::is_empty) {
        return Err("AWS Bedrock requires --think-endpoint".into());
    }

    let missing = [
        (
            "--think-credentials-type",
            options.think_credentials_type.is_none(),
        ),
        ("--think-aws-region", options.think_aws_region.is_none()),
        (
            "--think-aws-access-key-id",
            options.think_aws_access_key_id.is_none(),
        ),
        (
            "--think-aws-secret-access-key",
            options.think_aws_secret_access_key.is_none(),
        ),
    ];
    if let Some((name, _)) = missing.iter().find(|(_, missing)| *missing) {
        return Err(format!("AWS Bedrock requires {name}").into());
    }

    if options.think_credentials_type.as_deref() == Some("sts")
        && options.think_aws_session_token.is_none()
    {
        return Err("AWS STS credentials require --think-aws-session-token".into());
    }

    Ok(())
}

fn load_eleven_labs_api_key(
    options: &LaunchOptions,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    if options.speak_provider == "eleven_labs" {
        Ok(Some(env::var("ELEVEN_LABS_API_KEY").map_err(|_| {
            "ELEVEN_LABS_API_KEY environment variable not set (required for eleven_labs speak provider)"
        })?))
    } else {
        Ok(None)
    }
}

#[derive(Debug, Deserialize)]
struct CreateAgentConfigResponse {
    agent_id: String,
}

#[derive(Debug, Deserialize)]
struct ProjectsResponse {
    projects: Vec<ProjectSummary>,
}

#[derive(Debug, Deserialize)]
struct ProjectSummary {
    project_id: String,
    name: String,
}

fn log_api_request(
    verbose: bool,
    method: &reqwest::Method,
    url: &str,
    payload: Option<&serde_json::Value>,
) {
    if verbose {
        info!("API request: {} {}", method, url);
        info!(
            "API request headers: Authorization: Token <redacted>{}",
            if payload.is_some() {
                ", Content-Type: application/json"
            } else {
                ""
            }
        );
        info!(
            "API payload: {}",
            payload
                .map(|value| value.to_string())
                .unwrap_or_else(|| "<none>".to_string())
        );
    }
}

async fn resolve_project_id(
    api_key: &str,
    requested_project_id: Option<&str>,
    verbose: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    if let Some(project_id) = requested_project_id {
        return Ok(project_id.to_string());
    }

    let url = "https://api.deepgram.com/v1/projects";
    log_api_request(verbose, &reqwest::Method::GET, url, None);
    let response = reqwest::Client::new()
        .get(url)
        .header(reqwest::header::AUTHORIZATION, format!("Token {api_key}"))
        .send()
        .await?;
    let status = response.status();
    let body = response.text().await?;
    if !status.is_success() {
        return Err(format!("Deepgram project listing failed ({status}): {body}").into());
    }

    let projects: ProjectsResponse = serde_json::from_str(&body)
        .map_err(|error| format!("invalid response from project API: {error}; body: {body}"))?;
    match projects.projects.as_slice() {
        [] => Err("the API key has access to no Deepgram projects; provide a project ID with --project-id".into()),
        [project] => {
            info!(
                "Using the only accessible Deepgram project: {} ({})",
                project.name, project.project_id
            );
            Ok(project.project_id.clone())
        }
        _ => {
            let available = projects
                .projects
                .iter()
                .map(|project| format!("{} ({})", project.name, project.project_id))
                .collect::<Vec<_>>()
                .join(", ");
            Err(format!(
                "the API key has access to multiple Deepgram projects: {available}; specify one with --project-id"
            )
            .into())
        }
    }
}

async fn create_reusable_agent_config(
    api_key: &str,
    project_id: &str,
    config: &VoiceAgentConfig,
    name: &str,
    verbose: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    let agent = match &config.agent {
        AgentConfiguration::Inline(agent) => agent,
        AgentConfiguration::Reference(_) => {
            return Err("cannot create a configuration that is already an agent reference".into())
        }
    };

    if agent
        .think
        .endpoint
        .as_ref()
        .is_some_and(|endpoint| !endpoint.headers.is_empty())
        || agent
            .speak
            .endpoint
            .as_ref()
            .is_some_and(|endpoint| !endpoint.headers.is_empty())
    {
        return Err(
            "cannot create agent configurations containing provider headers; headers may contain secrets"
                .into(),
        );
    }

    if agent.think.provider.credentials.is_some() {
        return Err(
            "cannot create reusable agent configurations containing AWS credentials; use an inline Voice Agent launch"
                .into(),
        );
    }

    let request = serde_json::json!({
        "config": serde_json::to_string(agent)?,
        "metadata": { "name": name },
    });
    let url = format!("https://api.deepgram.com/v1/projects/{project_id}/agents");
    log_api_request(verbose, &reqwest::Method::POST, &url, Some(&request));
    let response = reqwest::Client::new()
        .post(url)
        .header(reqwest::header::AUTHORIZATION, format!("Token {api_key}"))
        .json(&request)
        .send()
        .await?;
    let status = response.status();
    let body = response.text().await?;
    if !status.is_success() {
        return Err(
            format!("Deepgram agent configuration create failed ({status}): {body}").into(),
        );
    }

    let result: CreateAgentConfigResponse = serde_json::from_str(&body).map_err(|error| {
        format!("invalid response from agent configuration API: {error}; body: {body}")
    })?;
    Ok(result.agent_id)
}

async fn delete_agent_config(
    api_key: &str,
    project_id: &str,
    agent_id: &str,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let url = format!("https://api.deepgram.com/v1/projects/{project_id}/agents/{agent_id}");
    log_api_request(verbose, &reqwest::Method::DELETE, &url, None);
    let response = reqwest::Client::new()
        .delete(url)
        .header(reqwest::header::AUTHORIZATION, format!("Token {api_key}"))
        .send()
        .await?;
    let status = response.status();
    let body = response.text().await?;
    if !status.is_success() {
        return Err(
            format!("Deepgram agent configuration deletion failed ({status}): {body}").into(),
        );
    }
    Ok(())
}

async fn delete_agent_configuration(
    args: ConfigDeleteArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    let api_key = env::var("DEEPGRAM_API_KEY")
        .map_err(|_| "DEEPGRAM_API_KEY environment variable not set")?;
    let project_id = resolve_project_id(&api_key, args.project_id.as_deref(), args.verbose).await?;

    if !args.yes {
        print!(
            "Delete reusable agent configuration '{}' from project '{}' [y/N]? ",
            args.agent_config_id, project_id
        );
        io::stdout().flush()?;
        let mut answer = String::new();
        io::stdin().read_line(&mut answer)?;
        if !matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
            println!("Deletion cancelled.");
            return Ok(());
        }
    }

    delete_agent_config(&api_key, &project_id, &args.agent_config_id, args.verbose).await?;
    println!(
        "Deleted reusable agent configuration {}",
        args.agent_config_id
    );
    Ok(())
}

fn parse_variable_value(raw: &str) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    Ok(serde_json::from_str(raw).unwrap_or_else(|_| serde_json::Value::String(raw.to_string())))
}

fn print_json_response(body: &str) {
    match serde_json::from_str::<serde_json::Value>(body) {
        Ok(value) => println!(
            "{}",
            serde_json::to_string_pretty(&value).unwrap_or_else(|_| body.to_string())
        ),
        Err(_) if !body.is_empty() => println!("{body}"),
        Err(_) => {}
    }
}

fn redact_voice_agent_credentials(config: &VoiceAgentConfig) -> serde_json::Value {
    let mut value = serde_json::to_value(config).expect("voice agent config should serialize");
    if let Some(credentials) = value
        .get_mut("agent")
        .and_then(|agent| agent.get_mut("think"))
        .and_then(|think| think.get_mut("provider"))
        .and_then(|provider| provider.get_mut("credentials"))
        .and_then(|credentials| credentials.as_object_mut())
    {
        for key in ["access_key_id", "secret_access_key", "session_token"] {
            if credentials.contains_key(key) {
                credentials.insert(key.to_string(), serde_json::json!("<redacted>"));
            }
        }
    }
    value
}

async fn agent_variable_request(
    method: reqwest::Method,
    url: String,
    api_key: &str,
    payload: Option<serde_json::Value>,
    verbose: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    log_api_request(verbose, &method, &url, payload.as_ref());
    let client = reqwest::Client::new();
    let mut request = client
        .request(method, url)
        .header(reqwest::header::AUTHORIZATION, format!("Token {api_key}"));
    if let Some(payload) = payload {
        request = request.json(&payload);
    }
    let response = request.send().await?;
    let status = response.status();
    let body = response.text().await?;
    if !status.is_success() {
        return Err(format!("Deepgram agent variable request failed ({status}): {body}").into());
    }
    Ok(body)
}

fn agent_variables_url(project_id: &str, variable_id: Option<&str>) -> String {
    match variable_id {
        Some(variable_id) => format!(
            "https://api.deepgram.com/v1/projects/{project_id}/agent-variables/{variable_id}"
        ),
        None => format!("https://api.deepgram.com/v1/projects/{project_id}/agent-variables"),
    }
}

async fn create_agent_variable(
    args: ConfigVariableCreateArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    let api_key = env::var("DEEPGRAM_API_KEY")
        .map_err(|_| "DEEPGRAM_API_KEY environment variable not set")?;
    let verbose = args.project.verbose;
    let project_id =
        resolve_project_id(&api_key, args.project.project_id.as_deref(), verbose).await?;
    let value = parse_variable_value(&args.value)?;
    let body = agent_variable_request(
        reqwest::Method::POST,
        agent_variables_url(&project_id, None),
        &api_key,
        Some(serde_json::json!({
            "key": args.key,
            "value": value,
            "is_sensitive": false
        })),
        verbose,
    )
    .await?;
    print_json_response(&body);
    Ok(())
}

async fn list_agent_variables(
    args: ConfigVariableProjectArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    let api_key = env::var("DEEPGRAM_API_KEY")
        .map_err(|_| "DEEPGRAM_API_KEY environment variable not set")?;
    let verbose = args.verbose;
    let project_id = resolve_project_id(&api_key, args.project_id.as_deref(), verbose).await?;
    let body = agent_variable_request(
        reqwest::Method::GET,
        agent_variables_url(&project_id, None),
        &api_key,
        None,
        verbose,
    )
    .await?;
    print_json_response(&body);
    Ok(())
}

async fn get_agent_variable(args: ConfigVariableGetArgs) -> Result<(), Box<dyn std::error::Error>> {
    let api_key = env::var("DEEPGRAM_API_KEY")
        .map_err(|_| "DEEPGRAM_API_KEY environment variable not set")?;
    let verbose = args.project.verbose;
    let project_id =
        resolve_project_id(&api_key, args.project.project_id.as_deref(), verbose).await?;
    let body = agent_variable_request(
        reqwest::Method::GET,
        agent_variables_url(&project_id, Some(&args.variable_id)),
        &api_key,
        None,
        verbose,
    )
    .await?;
    print_json_response(&body);
    Ok(())
}

async fn update_agent_variable(
    args: ConfigVariableUpdateArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    let api_key = env::var("DEEPGRAM_API_KEY")
        .map_err(|_| "DEEPGRAM_API_KEY environment variable not set")?;
    let verbose = args.project.verbose;
    let project_id =
        resolve_project_id(&api_key, args.project.project_id.as_deref(), verbose).await?;
    let value = parse_variable_value(&args.value)?;
    let body = agent_variable_request(
        reqwest::Method::PATCH,
        agent_variables_url(&project_id, Some(&args.variable_id)),
        &api_key,
        Some(serde_json::json!({"value": value})),
        verbose,
    )
    .await?;
    print_json_response(&body);
    Ok(())
}

async fn delete_agent_variable(
    args: ConfigVariableDeleteArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    let api_key = env::var("DEEPGRAM_API_KEY")
        .map_err(|_| "DEEPGRAM_API_KEY environment variable not set")?;
    let verbose = args.project.verbose;
    let project_id =
        resolve_project_id(&api_key, args.project.project_id.as_deref(), verbose).await?;
    if !args.yes {
        print!(
            "Delete agent template variable '{}' from project '{}' [y/N]? ",
            args.variable_id, project_id
        );
        io::stdout().flush()?;
        let mut answer = String::new();
        io::stdin().read_line(&mut answer)?;
        if !matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
            println!("Deletion cancelled.");
            return Ok(());
        }
    }

    agent_variable_request(
        reqwest::Method::DELETE,
        agent_variables_url(&project_id, Some(&args.variable_id)),
        &api_key,
        None,
        verbose,
    )
    .await?;
    println!("Deleted agent template variable {}", args.variable_id);
    Ok(())
}

async fn handle_voice_agent_responses(
    mut ws_receiver: futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
    audio_tx: std_mpsc::Sender<Vec<u8>>,
    mic_enabled: Arc<AtomicBool>,
    mute_on_playback: bool,
    verbose: bool,
) {
    while let Some(message) = ws_receiver.next().await {
        match message {
            Ok(Message::Text(text)) => match serde_json::from_str::<VoiceAgentResponse>(&text) {
                Ok(response) => {
                    debug!("📨 Message Type: {}", response.message_type);

                    match response.message_type.as_str() {
                        "ConversationText" => {
                            let role = response
                                .data
                                .get("role")
                                .and_then(|r| r.as_str())
                                .unwrap_or("");
                            let content = response
                                .data
                                .get("content")
                                .and_then(|c| c.as_str())
                                .unwrap_or("");
                            match role {
                                "user" => info!("👤 You: {}", content),
                                "assistant" => info!("🤖 Agent: {}", content),
                                _ => debug!("ConversationText ({}): {}", role, content),
                            }
                        }
                        "AgentThinking" => {
                            debug!("🤔 Agent is thinking...");
                        }
                        "AgentStartedSpeaking" => {
                            debug!("🗣️ Agent is speaking...");
                        }
                        "UserStartedSpeaking" => {
                            debug!("🎙️ User started speaking");
                        }
                        "AgentAudioDone" => {
                            debug!("🔊 Agent audio done");
                        }
                        "Welcome" => {
                            let request_id = response
                                .data
                                .get("request_id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            debug!("👋 Connected: request_id={}", request_id);
                            if verbose {
                                info!("Request ID: {}", request_id);
                            }
                        }
                        "SettingsApplied" => {
                            debug!("✅ Settings applied");
                        }
                        "Error" => {
                            let desc = response
                                .data
                                .get("description")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown");
                            let code = response
                                .data
                                .get("code")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            error!("❌ Agent error [{}]: {}", code, desc);
                        }
                        "Warning" => {
                            let desc = response
                                .data
                                .get("description")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown");
                            let code = response
                                .data
                                .get("code")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            log::warn!("⚠️ Agent warning [{}]: {}", code, desc);
                        }
                        _ => {
                            debug!(
                                "📄 {}: {}",
                                response.message_type,
                                serde_json::to_string_pretty(&response.data).unwrap_or_default()
                            );
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to parse response: {}", e);
                    debug!("📨 Raw response: {}", text);
                }
            },
            Ok(Message::Binary(data)) => {
                debug!("🔊 Received binary audio data: {} bytes", data.len());
                if mute_on_playback {
                    mic_enabled.store(false, Ordering::Relaxed);
                    debug!("🎤 Microphone disabled immediately upon receiving binary audio");
                }

                // Handle binary audio data directly
                if let Err(e) = audio_tx.send(data.to_vec()) {
                    error!("Failed to send binary audio to player: {}", e);
                }
            }
            Ok(Message::Close(frame)) => {
                debug!("📨 WebSocket connection closed");
                if let Some(frame) = frame {
                    debug!("Close frame: code={}, reason={}", frame.code, frame.reason);
                }
                break;
            }
            Ok(Message::Ping(_)) => {
                debug!("📨 Received ping");
            }
            Ok(Message::Pong(_)) => {
                debug!("📨 Received pong");
            }
            Err(e) => {
                error!("WebSocket error: {}", e);
                break;
            }
            _ => {}
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging; defaults to "info" but RUST_LOG overrides (e.g. RUST_LOG=debug)
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .target(env_logger::Target::Stdout)
        .init();

    dotenv::dotenv().ok();

    let args = Args::parse();
    match args.command {
        None => run_voice_agent(args.launch, None).await,
        Some(Command::Config { command }) => match command {
            ConfigCommand::Create(create) => create_agent_configuration(create).await,
            ConfigCommand::Use(use_config) => {
                run_voice_agent(use_config.launch, Some(use_config.agent_config_id)).await
            }
            ConfigCommand::Delete(delete) => delete_agent_configuration(delete).await,
            ConfigCommand::Variable { command } => match command {
                ConfigVariableCommand::Create(create) => create_agent_variable(create).await,
                ConfigVariableCommand::List(list) => list_agent_variables(list).await,
                ConfigVariableCommand::Get(get) => get_agent_variable(get).await,
                ConfigVariableCommand::Update(update) => update_agent_variable(update).await,
                ConfigVariableCommand::Delete(delete) => delete_agent_variable(delete).await,
            },
        },
    }
}

async fn create_agent_configuration(
    args: ConfigCreateArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    validate_think_options(&args.launch)?;
    let api_key = env::var("DEEPGRAM_API_KEY")
        .map_err(|_| "DEEPGRAM_API_KEY environment variable not set")?;
    let project_id =
        resolve_project_id(&api_key, args.project_id.as_deref(), args.launch.verbose).await?;
    let eleven_labs_api_key = load_eleven_labs_api_key(&args.launch)?;
    let audio_capture = AudioCapture::new()?;
    let config = config_from_options(
        &args.launch,
        audio_capture.config.sample_rate.0,
        audio_capture.config.channels,
        eleven_labs_api_key,
    );
    let agent_id = create_reusable_agent_config(
        &api_key,
        &project_id,
        &config,
        &args.name,
        args.launch.verbose,
    )
    .await?;
    println!(
        "Saved reusable agent configuration '{}' as {}",
        args.name, agent_id
    );
    Ok(())
}

async fn run_voice_agent(
    args: LaunchOptions,
    agent_config_id: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables

    validate_think_options(&args)?;

    let api_key = env::var("DEEPGRAM_API_KEY")
        .map_err(|_| "DEEPGRAM_API_KEY environment variable not set")?;

    // Load Eleven Labs API key if using eleven_labs speak provider
    let eleven_labs_api_key = load_eleven_labs_api_key(&args)?;

    info!("Starting Deepgram Voice Agent...");
    debug!("Using endpoint: {}", args.endpoint);

    // Initialize audio capture
    let audio_capture = AudioCapture::new()?;
    let sample_rate = audio_capture.config.sample_rate.0;
    let channels = audio_capture.config.channels;

    debug!(
        "Audio config - Sample rate: {}, Channels: {}",
        sample_rate, channels
    );

    // Create microphone control flag - start with mic enabled
    let mic_enabled = Arc::new(AtomicBool::new(true));

    // Create channels for audio data
    let (audio_tx, mut audio_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let (playback_tx, playback_rx) = std_mpsc::channel::<Vec<u8>>();

    let mute_on_playback = !args.no_mic_mute;
    if !mute_on_playback {
        info!("Microphone muting during playback is disabled");
    }

    // Initialize audio player in a separate thread
    let mic_enabled_for_player = Arc::clone(&mic_enabled);
    std::thread::spawn(move || {
        let audio_player = match AudioPlayer::new(mic_enabled_for_player, mute_on_playback) {
            Ok(player) => player,
            Err(e) => {
                error!("Failed to create audio player: {}", e);
                return;
            }
        };

        for audio_data in playback_rx {
            if let Err(e) = audio_player.play_audio(audio_data) {
                error!("Failed to play audio: {}", e);
            }
        }
    });

    // Start audio capture with microphone control
    let mic_enabled_for_capture = Arc::clone(&mic_enabled);
    let _stream = audio_capture.start_capture(audio_tx, mic_enabled_for_capture)?;
    debug!("Audio capture started");

    let mut config = config_from_options(&args, sample_rate, channels, eleven_labs_api_key);
    if let Some(agent_id) = agent_config_id {
        config.agent = AgentConfiguration::Reference(agent_id.to_string());
    }

    // Connect to Deepgram Voice Agent
    let ws_stream = connect_to_voice_agent(
        &api_key,
        &args.endpoint,
        sample_rate,
        channels,
        args.verbose,
    )
    .await?;
    let (mut ws_sender, ws_receiver) = ws_stream.split();

    // Send Settings configuration
    let config_json = serde_json::to_string(&config)?;
    debug!("📤 Sending Settings configuration to WebSocket...");

    if args.verbose {
        info!(
            "Voice Agent Settings payload: {}",
            redact_voice_agent_credentials(&config)
        );
    }

    ws_sender.send(Message::Text(config_json.into())).await?;
    debug!("✅ Settings configuration sent successfully");

    // Wait a moment for configuration to be processed
    sleep(Duration::from_millis(500)).await;

    // Spawn task to handle WebSocket responses
    let playback_tx_clone = playback_tx.clone();
    let mic_enabled_for_ws = Arc::clone(&mic_enabled);
    let response_handle = tokio::spawn(async move {
        handle_voice_agent_responses(
            ws_receiver,
            playback_tx_clone,
            mic_enabled_for_ws,
            mute_on_playback,
            args.verbose,
        )
        .await;
    });

    // Main loop: send audio data to WebSocket
    info!("🎤 Voice Agent is ready! Start speaking...");
    info!("Press Ctrl+C to stop");

    let audio_handle = tokio::spawn(async move {
        let mut packet_count = 0u64;

        while let Some(audio_data) = audio_rx.recv().await {
            packet_count += 1;

            // Send audio data as binary message
            if let Err(e) = ws_sender.send(Message::Binary(audio_data.into())).await {
                error!("❌ Failed to send audio data to WebSocket: {}", e);
                break;
            }

            // Log every 100 packets to avoid spam
            if packet_count % 100 == 0 {
                debug!("📤 Sent {} audio packets to WebSocket", packet_count);
            }
        }
    });

    // Wait for either task to complete or for Ctrl+C
    tokio::select! {
        _ = response_handle => {
            debug!("Response handler completed");
        }
        _ = audio_handle => {
            debug!("Audio handler completed");
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Received Ctrl+C, shutting down...");
        }
    }

    info!("🛑 Voice Agent stopped");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_for_listen_model(
        model: &str,
        smart_format: Option<bool>,
        language_hints: &[String],
    ) -> serde_json::Value {
        let keyterms: Vec<String> = Vec::new();
        let config = create_agent_config(
            16000,
            1,
            ListenArgs {
                provider: "deepgram",
                model,
                version: None,
                language: "en",
                language_hints,
                keyterms: &keyterms,
                eot_threshold: None,
                eager_eot_threshold: None,
                smart_format,
            },
            SpeakArgs {
                provider: "deepgram",
                model: "aura-2-thalia-en",
                model_id: "eleven_turbo_v2_5",
                language_code: "en-US",
                voice_id: None,
                eleven_labs_api_key: None,
            },
            "open_ai",
            "gpt-4o-mini",
            None,
            None,
            &[],
            None,
            None,
            None,
            None,
            None,
            None,
        );

        serde_json::to_value(config).expect("settings config should serialize")
    }

    #[test]
    fn listen_language_is_omitted_for_flux_models() {
        let config = config_for_listen_model("flux-general-en", None, &[]);
        let provider = &config["agent"]["listen"]["provider"];

        assert!(provider.get("language").is_none());
    }

    #[test]
    fn listen_language_is_included_for_non_flux_models() {
        let config = config_for_listen_model("nova-3", None, &[]);
        let provider = &config["agent"]["listen"]["provider"];

        assert_eq!(provider["language"], "en");
    }

    #[test]
    fn smart_format_is_omitted_when_unspecified() {
        let config = config_for_listen_model("nova-3", None, &[]);
        let provider = &config["agent"]["listen"]["provider"];

        assert!(provider.get("smart_format").is_none());
    }

    #[test]
    fn smart_format_is_included_when_specified() {
        let config = config_for_listen_model("nova-3", Some(true), &[]);
        let provider = &config["agent"]["listen"]["provider"];

        assert_eq!(provider["smart_format"], true);
    }

    #[test]
    fn language_hints_are_omitted_when_unspecified() {
        let config = config_for_listen_model("nova-3", None, &[]);
        let provider = &config["agent"]["listen"]["provider"];

        assert!(provider.get("language_hints").is_none());
    }

    #[test]
    fn language_hints_are_included_when_specified() {
        let language_hints = vec!["en".to_string(), "es".to_string()];
        let config = config_for_listen_model("nova-3", None, &language_hints);
        let provider = &config["agent"]["listen"]["provider"];

        assert_eq!(provider["language_hints"], serde_json::json!(["en", "es"]));
    }

    #[test]
    fn language_hint_cli_accepts_comma_separated_values() {
        let args = Args::try_parse_from(["voice-agent", "--language-hint", "en,es"])
            .expect("language hint CSV should parse");

        assert_eq!(args.launch.language_hints, vec!["en", "es"]);
    }

    #[test]
    fn reusable_agent_reference_serializes_as_a_string() {
        let mut config = config_for_listen_model("nova-3", None, &[]);
        config["agent"] = serde_json::json!("a1b2c3d4-e5f6-7890-abcd-ef1234567890");

        assert_eq!(config["agent"], "a1b2c3d4-e5f6-7890-abcd-ef1234567890");
    }

    #[test]
    fn default_system_prompt_requests_concise_responses() {
        let config = config_for_listen_model("nova-3", None, &[]);

        assert_eq!(config["agent"]["think"]["prompt"], DEFAULT_SYSTEM_PROMPT);
    }

    #[test]
    fn config_create_is_a_subcommand() {
        let args = Args::try_parse_from([
            "voice-agent",
            "config",
            "create",
            "--project-id",
            "project",
            "--name",
            "support",
        ])
        .expect("config create should parse");

        assert!(matches!(
            args.command,
            Some(Command::Config {
                command: ConfigCommand::Create(_)
            })
        ));
    }

    #[test]
    fn config_use_accepts_a_positional_agent_id() {
        let args = Args::try_parse_from([
            "voice-agent",
            "config",
            "use",
            "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
        ])
        .expect("config use should parse");

        assert!(matches!(
            args.command,
            Some(Command::Config {
                command: ConfigCommand::Use(ConfigUseArgs { agent_config_id, .. })
            }) if agent_config_id == "a1b2c3d4-e5f6-7890-abcd-ef1234567890"
        ));
    }

    #[test]
    fn config_delete_accepts_project_and_agent_id() {
        let args = Args::try_parse_from([
            "voice-agent",
            "config",
            "delete",
            "--project-id",
            "project",
            "--yes",
            "agent-id",
        ])
        .expect("config delete should parse");

        assert!(matches!(
            args.command,
            Some(Command::Config {
                command: ConfigCommand::Delete(ConfigDeleteArgs {
                    project_id,
                    agent_config_id,
                    yes: true,
                    ..
                })
            }) if project_id == Some("project".to_string()) && agent_config_id == "agent-id"
        ));
    }

    #[test]
    fn projects_response_deserializes_project_id_and_name() {
        let response: ProjectsResponse = serde_json::from_value(serde_json::json!({
            "projects": [{"project_id": "project", "name": "Support"}]
        }))
        .expect("project response should deserialize");

        assert_eq!(response.projects[0].project_id, "project");
        assert_eq!(response.projects[0].name, "Support");
    }

    #[test]
    fn plain_text_variable_values_are_encoded_as_json_strings() {
        assert_eq!(
            parse_variable_value("Hello and welcome").expect("value should parse"),
            serde_json::json!("Hello and welcome")
        );
        assert_eq!(
            parse_variable_value("42").expect("JSON number should parse"),
            serde_json::json!(42)
        );
    }
}
