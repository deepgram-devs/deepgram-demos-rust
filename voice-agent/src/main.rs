use std::env;

use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Stream, StreamConfig, SampleFormat};
use futures_util::{SinkExt, StreamExt};
use log::{error, info};
use base64::{Engine as _, engine::general_purpose};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use std::sync::mpsc as std_mpsc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
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
    model: String,
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
        
        info!("Input device: {}", device.name()?);
        
        let supported_config = device.default_input_config()?;
        info!("Default input config: {:?}", supported_config);
        
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
                // Only capture audio if microphone is enabled
                let mic_is_enabled = mic_enabled.load(Ordering::Relaxed);
                if !mic_is_enabled {
                    return;
                }
                
                // Convert samples to f32 and then to i16 for Deepgram
                let mut audio_data = Vec::with_capacity(data.len() * 2);
                
                for &sample in data.iter() {
                    let f32_sample: f32 = cpal::Sample::from_sample(sample);
                    let i16_sample = (f32_sample * i16::MAX as f32) as i16;
                    audio_data.extend_from_slice(&i16_sample.to_le_bytes());
                }
                
                if let Err(_e) = tx.send(audio_data) {
                    // Audio capture stopped, this is expected when shutting down
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
    sink: Sink,
    mic_enabled: Arc<AtomicBool>,
    playback_stopped_time: Arc<Mutex<Option<Instant>>>,
}

impl AudioPlayer {
    fn new(mic_enabled: Arc<AtomicBool>) -> Result<Self, Box<dyn std::error::Error>> {
        let stream_handle = rodio::OutputStreamBuilder::open_default_stream()
            .map_err(|e| format!("Failed to create audio output stream: {}", e))?;
        let sink = Sink::connect_new(&stream_handle.mixer());
        
        let playback_stopped_time = Arc::new(Mutex::new(None::<Instant>));
        
        // Start background task to monitor microphone re-enabling
        let mic_enabled_clone = Arc::clone(&mic_enabled);
        let playback_stopped_time_clone = Arc::clone(&playback_stopped_time);
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_millis(50)); // Check every 50ms
                
                // Check if we should re-enable microphone
                let should_enable_mic = {
                    let stopped_time_guard = playback_stopped_time_clone.lock().unwrap();
                    if let Some(stopped_time) = *stopped_time_guard {
                        // Check if 600ms have passed since playback stopped
                        stopped_time.elapsed() >= Duration::from_millis(600)
                    } else {
                        false
                    }
                };
                
                if should_enable_mic && !mic_enabled_clone.load(Ordering::Relaxed) {
                    mic_enabled_clone.store(true, Ordering::Relaxed);
                    info!("🎤 Microphone re-enabled after 600ms of playback silence");
                    // Reset the stopped time to prevent repeated enabling
                    *playback_stopped_time_clone.lock().unwrap() = None;
                }
            }
        });
        
        Ok(AudioPlayer {
            _stream_handle: stream_handle,
            sink,
            mic_enabled,
            playback_stopped_time,
        })
    }
    
    fn play_audio(&self, audio_data: Vec<u8>) -> Result<(), Box<dyn std::error::Error>> {
        info!("🔊 Received audio data for playback: {} bytes", audio_data.len());
        
        if audio_data.is_empty() {
            return Ok(());
        }
        
        // Microphone should already be disabled by WebSocket handler
        // but ensure it's disabled here as well for safety
        self.mic_enabled.store(false, Ordering::Relaxed);
        
        // The audio data from Deepgram is linear16 PCM at 24kHz (as configured)
        // Convert bytes to f32 samples (rodio expects f32)
        let mut samples = Vec::with_capacity(audio_data.len() / 2);
        for chunk in audio_data.chunks_exact(2) {
            let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
            // Convert i16 to f32 in range [-1.0, 1.0]
            let f32_sample = sample as f32 / i16::MAX as f32;
            samples.push(f32_sample);
        }
        
        // Calculate audio duration
        let sample_rate = 24000.0; // Hz
        let duration_seconds = (samples.len() as f32) / sample_rate;
        let audio_duration = Duration::from_secs_f32(duration_seconds);
        
        // Create a source from the PCM data
        let source = PCMSource::new(samples, 24000, 1);
        
        // Append to sink for playback
        self.sink.append(source);
        
        // Schedule playback completion time (but clear any previous timers)
        {
            let mut stopped_time_guard = self.playback_stopped_time.lock().unwrap();
            *stopped_time_guard = None; // Clear any existing timer since new audio is starting
        }
        
        let playback_stopped_time_clone = Arc::clone(&self.playback_stopped_time);
        
        std::thread::spawn(move || {
            // Wait for this audio chunk to finish playing
            std::thread::sleep(audio_duration);
            
            // Only set the stopped time if no newer audio has started
            {
                let mut stopped_time_guard = playback_stopped_time_clone.lock().unwrap();
                // Only update if we haven't been superseded by newer audio
                if stopped_time_guard.is_none() {
                    *stopped_time_guard = Some(Instant::now());
                    info!("🔊 Audio playback finished, starting 600ms timer");
                }
            }
        });
        
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
    
    info!("Connecting to Deepgram Voice Agent WebSocket...");
    let (ws_stream, _response) = connect_async(request).await?;
    info!("Connected to Deepgram Voice Agent successfully");
    
    Ok(ws_stream)
}

fn create_agent_config(sample_rate: u32, _channels: u16) -> VoiceAgentConfig {
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
                    provider_type: "open_ai".to_string(),
                    model: "gpt-4o-mini".to_string(),
                },
            },
            speak: SpeakConfig {
                provider: SpeakProviderConfig {
                    provider_type: "deepgram".to_string(),
                    model: "aura-2-thalia-en".to_string(),
                },
            },
        },
    }
}

async fn handle_voice_agent_responses(
    mut ws_receiver: futures_util::stream::SplitStream<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>>,
    audio_tx: std_mpsc::Sender<Vec<u8>>,
    mic_enabled: Arc<AtomicBool>,
) {
    while let Some(message) = ws_receiver.next().await {
        match message {
            Ok(Message::Text(text)) => {
                match serde_json::from_str::<VoiceAgentResponse>(&text) {
                    Ok(response) => {
                        info!("📨 Message Type: {}", response.message_type);
                        
                        match response.message_type.as_str() {
                            "agent_audio" => {
                                info!("🔊 Received audio from agent");
                                // Disable microphone immediately when audio is received
                                mic_enabled.store(false, Ordering::Relaxed);
                                info!("🎤 Microphone disabled immediately upon receiving audio");
                                
                                // Audio data should be in the response, but the exact format depends on the API
                                // This is a placeholder - you'll need to extract the actual audio data
                                if let Some(audio_data) = response.data.get("audio") {
                                    if let Some(audio_str) = audio_data.as_str() {
                                        // Decode base64 audio data
                                        if let Ok(decoded_audio) = general_purpose::STANDARD.decode(audio_str) {
                                            if let Err(e) = audio_tx.send(decoded_audio) {
                                                error!("Failed to send audio to player: {}", e);
                                            }
                                        }
                                    }
                                }
                            }
                            "agent_transcript" => {
                                if let Some(transcript) = response.data.get("transcript") {
                                    info!("🤖 Agent: {}", transcript.as_str().unwrap_or(""));
                                }
                            }
                            "user_transcript" => {
                                if let Some(transcript) = response.data.get("transcript") {
                                    info!("👤 You: {}", transcript.as_str().unwrap_or(""));
                                }
                            }
                            "agent_thinking" => {
                                info!("🤔 Agent is thinking...");
                            }
                            "agent_speaking" => {
                                info!("🗣️ Agent is speaking...");
                            }
                            _ => {
                                info!("📄 Response: {}", serde_json::to_string_pretty(&response.data).unwrap_or_default());
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to parse response: {}", e);
                        info!("📨 Raw response: {}", text);
                    }
                }
            }
            Ok(Message::Binary(data)) => {
                info!("🔊 Received binary audio data: {} bytes", data.len());
                // Disable microphone immediately when audio is received
                mic_enabled.store(false, Ordering::Relaxed);
                info!("🎤 Microphone disabled immediately upon receiving binary audio");
                
                // Handle binary audio data directly
                if let Err(e) = audio_tx.send(data.to_vec()) {
                    error!("Failed to send binary audio to player: {}", e);
                }
            }
            Ok(Message::Close(frame)) => {
                info!("📨 WebSocket connection closed");
                if let Some(frame) = frame {
                    info!("Close frame: code={}, reason={}", frame.code, frame.reason);
                }
                break;
            }
            Ok(Message::Ping(_)) => {
                info!("📨 Received ping");
            }
            Ok(Message::Pong(_)) => {
                info!("📨 Received pong");
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
    
    // Initialize logging
    env_logger::init();
    
    // Load environment variables
    dotenv::dotenv().ok();
    
    let api_key = env::var("DEEPGRAM_API_KEY")
        .map_err(|_| "DEEPGRAM_API_KEY environment variable not set")?;
    
    info!("Starting Deepgram Voice Agent...");
    info!("Using endpoint: {}", args.endpoint);
    
    // Initialize audio capture
    let audio_capture = AudioCapture::new()?;
    let sample_rate = audio_capture.config.sample_rate.0;
    let channels = audio_capture.config.channels;
    
    info!("Audio config - Sample rate: {}, Channels: {}", sample_rate, channels);
    
    // Create microphone control flag - start with mic enabled
    let mic_enabled = Arc::new(AtomicBool::new(true));
    
    // Create channels for audio data
    let (audio_tx, mut audio_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let (playback_tx, playback_rx) = std_mpsc::channel::<Vec<u8>>();
    
    // Initialize audio player in a separate thread
    let mic_enabled_for_player = Arc::clone(&mic_enabled);
    std::thread::spawn(move || {
        let audio_player = match AudioPlayer::new(mic_enabled_for_player) {
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
    info!("Audio capture started");
    
    // Connect to Deepgram Voice Agent
    let ws_stream = connect_to_voice_agent(&api_key, &args.endpoint, sample_rate, channels).await?;
    let (mut ws_sender, ws_receiver) = ws_stream.split();
    
    // Send Settings configuration
    let config = create_agent_config(sample_rate, channels);
    let config_json = serde_json::to_string(&config)?;
    info!("📤 Sending Settings configuration to WebSocket...");
    info!("📄 Config: {}", config_json);
    ws_sender.send(Message::Text(config_json.into())).await?;
    info!("✅ Settings configuration sent successfully");
    
    // Wait a moment for configuration to be processed
    sleep(Duration::from_millis(500)).await;
    
    // Spawn task to handle WebSocket responses
    let playback_tx_clone = playback_tx.clone();
    let mic_enabled_for_ws = Arc::clone(&mic_enabled);
    let response_handle = tokio::spawn(async move {
        handle_voice_agent_responses(ws_receiver, playback_tx_clone, mic_enabled_for_ws).await;
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
                info!("📤 Sent {} audio packets to WebSocket", packet_count);
            }
        }
    });
    
    // Wait for either task to complete or for Ctrl+C
    tokio::select! {
        _ = response_handle => {
            info!("Response handler completed");
        }
        _ = audio_handle => {
            info!("Audio handler completed");
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Received Ctrl+C, shutting down...");
        }
    }
    
    info!("🛑 Voice Agent stopped");
    Ok(())
}
