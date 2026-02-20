use std::env;

use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Stream, StreamConfig, SampleFormat};
use futures_util::{SinkExt, StreamExt};
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use std::sync::mpsc as std_mpsc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::time::sleep;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use url::Url;
use rodio::{OutputStream, Sink, Source};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "voice-agent")]
#[command(about = "A Deepgram Voice Agent client")]
#[command(version)]
struct Args {
    /// Custom Deepgram endpoint URL to connect to
    #[arg(long, default_value = "wss://agent.deepgram.com")]
    endpoint: String,
    
    /// Speak provider model to use for text-to-speech
    #[arg(long, default_value = "aura-2-thalia-en")]
    speak_model: String,
    
    /// Think provider type to use for LLM processing
    #[arg(long, default_value = "open_ai")]
    think_type: String,
    
    /// Think provider model to use for LLM processing
    #[arg(long, default_value = "gpt-4o-mini")]
    think_model: String,
    
    /// Custom endpoint URL for think provider
    #[arg(long)]
    think_endpoint: Option<String>,
    
    /// Custom headers for think provider in format "key=value" (can be specified multiple times)
    #[arg(long)]
    think_header: Vec<String>,
    
    /// Enable verbose output including full Settings JSON message
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
    agent: AgentSettings,
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
    language: String,
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
}

#[derive(Debug, Serialize, Deserialize)]
struct ListenProviderConfig {
    #[serde(rename = "type")]
    provider_type: String,
    model: String,
    smart_format: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct ThinkProviderConfig {
    #[serde(rename = "type")]
    provider_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SpeakProviderConfig {
    #[serde(rename = "type")]
    provider_type: String,
    model: String,
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
        
        Ok(AudioCapture { device, config, sample_format })
    }
    
    fn start_capture(&self, tx: mpsc::UnboundedSender<Vec<u8>>, mic_enabled: Arc<AtomicBool>) -> Result<Stream, Box<dyn std::error::Error>> {
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
    
    fn build_stream<T>(&self, config: StreamConfig, tx: mpsc::UnboundedSender<Vec<u8>>, mic_enabled: Arc<AtomicBool>) -> Result<Stream, Box<dyn std::error::Error>>
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
    fn new(mic_enabled: Arc<AtomicBool>, mute_on_playback: bool) -> Result<Self, Box<dyn std::error::Error>> {
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
        debug!("🔊 Received audio data for playback: {} bytes", audio_data.len());

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

async fn connect_to_voice_agent(api_key: &str, endpoint: &str, _sample_rate: u32, _channels: u16) -> Result<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, Box<dyn std::error::Error>> {
    let url = Url::parse(format!("{0}/v1/agent/converse", endpoint).as_str())?;
    
    let request = tokio_tungstenite::tungstenite::handshake::client::Request::get(url.as_str())
        .header("Authorization", format!("Token {}", api_key))
        .header("Host", url.host_str().unwrap_or("agent.deepgram.com"))
        .header("Upgrade", "websocket")
        .header("Connection", "Upgrade")
        .header("Sec-WebSocket-Key", tokio_tungstenite::tungstenite::handshake::client::generate_key())
        .header("Sec-WebSocket-Version", "13")
        .body(())?;
    
    debug!("Connecting to Deepgram Voice Agent WebSocket...");
    let (ws_stream, _response) = connect_async(request).await?;
    debug!("Connected to Deepgram Voice Agent successfully");
    
    Ok(ws_stream)
}

fn create_agent_config(sample_rate: u32, _channels: u16, speak_model: &str, think_type: &str, think_model: &str, think_endpoint: Option<&str>, think_headers: &[String]) -> VoiceAgentConfig {
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
        agent: AgentSettings {
            language: "en".to_string(),
            listen: ListenConfig {
                provider: ListenProviderConfig {
                    provider_type: "deepgram".to_string(),
                    model: "nova-3".to_string(),
                    smart_format: false,
                },
            },
            think: ThinkConfig {
                provider: ThinkProviderConfig {
                    provider_type: think_type.to_string(),
                    model: if think_model.is_empty() { None } else { Some(think_model.to_string()) },
                    temperature: None,
                },
                endpoint: endpoint_config,
            },
            speak: SpeakConfig {
                provider: SpeakProviderConfig {
                    provider_type: "deepgram".to_string(),
                    model: speak_model.to_string(),
                },
            },
        },
    }
}

async fn handle_voice_agent_responses(
    mut ws_receiver: futures_util::stream::SplitStream<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>>,
    audio_tx: std_mpsc::Sender<Vec<u8>>,
    mic_enabled: Arc<AtomicBool>,
    mute_on_playback: bool,
) {
    while let Some(message) = ws_receiver.next().await {
        match message {
            Ok(Message::Text(text)) => {
                match serde_json::from_str::<VoiceAgentResponse>(&text) {
                    Ok(response) => {
                        debug!("📨 Message Type: {}", response.message_type);

                        match response.message_type.as_str() {
                            "ConversationText" => {
                                let role = response.data.get("role").and_then(|r| r.as_str()).unwrap_or("");
                                let content = response.data.get("content").and_then(|c| c.as_str()).unwrap_or("");
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
                                debug!("👋 Connected: request_id={}", response.data.get("request_id").and_then(|v| v.as_str()).unwrap_or(""));
                            }
                            "SettingsApplied" => {
                                debug!("✅ Settings applied");
                            }
                            "Error" => {
                                let desc = response.data.get("description").and_then(|v| v.as_str()).unwrap_or("unknown");
                                let code = response.data.get("code").and_then(|v| v.as_str()).unwrap_or("");
                                error!("❌ Agent error [{}]: {}", code, desc);
                            }
                            "Warning" => {
                                let desc = response.data.get("description").and_then(|v| v.as_str()).unwrap_or("unknown");
                                let code = response.data.get("code").and_then(|v| v.as_str()).unwrap_or("");
                                log::warn!("⚠️ Agent warning [{}]: {}", code, desc);
                            }
                            _ => {
                                debug!("📄 {}: {}", response.message_type, serde_json::to_string_pretty(&response.data).unwrap_or_default());
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to parse response: {}", e);
                        debug!("📨 Raw response: {}", text);
                    }
                }
            }
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
    // Parse command-line arguments
    let args = Args::parse();
    
    // Initialize logging; defaults to "info" but RUST_LOG overrides (e.g. RUST_LOG=debug)
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .target(env_logger::Target::Stdout)
        .init();
    
    // Load environment variables
    dotenv::dotenv().ok();
    
    let api_key = env::var("DEEPGRAM_API_KEY")
        .map_err(|_| "DEEPGRAM_API_KEY environment variable not set")?;
    
    info!("Starting Deepgram Voice Agent...");
    debug!("Using endpoint: {}", args.endpoint);
    
    // Initialize audio capture
    let audio_capture = AudioCapture::new()?;
    let sample_rate = audio_capture.config.sample_rate.0;
    let channels = audio_capture.config.channels;
    
    debug!("Audio config - Sample rate: {}, Channels: {}", sample_rate, channels);
    
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
    
    // Connect to Deepgram Voice Agent
    let ws_stream = connect_to_voice_agent(&api_key, &args.endpoint, sample_rate, channels).await?;
    let (mut ws_sender, ws_receiver) = ws_stream.split();
    
    // Send Settings configuration
    let config = create_agent_config(
        sample_rate, 
        channels, 
        &args.speak_model, 
        &args.think_type, 
        &args.think_model,
        args.think_endpoint.as_deref(),
        &args.think_header
    );
    let config_json = serde_json::to_string(&config)?;
    debug!("📤 Sending Settings configuration to WebSocket...");
    
    if args.verbose {
        // Print the entire JSON Settings message with pretty formatting
        let pretty_config = serde_json::to_string_pretty(&config)?;
        info!("📄 Complete Settings JSON message:\n{}", pretty_config);
    }
    
    ws_sender.send(Message::Text(config_json.into())).await?;
    debug!("✅ Settings configuration sent successfully");
    
    // Wait a moment for configuration to be processed
    sleep(Duration::from_millis(500)).await;
    
    // Spawn task to handle WebSocket responses
    let playback_tx_clone = playback_tx.clone();
    let mic_enabled_for_ws = Arc::clone(&mic_enabled);
    let response_handle = tokio::spawn(async move {
        handle_voice_agent_responses(ws_receiver, playback_tx_clone, mic_enabled_for_ws, mute_on_playback).await;
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
