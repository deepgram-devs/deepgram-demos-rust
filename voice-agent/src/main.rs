use std::env;
use std::sync::Arc;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Stream, StreamConfig, SampleFormat};
use futures_util::{SinkExt, StreamExt};
use log::{error, info};
use base64::{Engine as _, engine::general_purpose};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::time::sleep;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use url::Url;

#[derive(Debug, Serialize, Deserialize)]
struct VoiceAgentConfig {
    #[serde(rename = "type")]
    message_type: String,
    config: AgentConfig,
}

#[derive(Debug, Serialize, Deserialize)]
struct AgentConfig {
    agent: AgentSettings,
    audio: AudioConfig,
}

#[derive(Debug, Serialize, Deserialize)]
struct AgentSettings {
    listen: ListenConfig,
    think: ThinkConfig,
    speak: SpeakConfig,
}

#[derive(Debug, Serialize, Deserialize)]
struct ListenConfig {
    model: String,
    language: String,
    smart_format: bool,
    interim_results: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct ThinkConfig {
    provider: String,
    model: String,
    instructions: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct SpeakConfig {
    model: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct AudioConfig {
    input: AudioInputConfig,
    output: AudioOutputConfig,
}

#[derive(Debug, Serialize, Deserialize)]
struct AudioInputConfig {
    encoding: String,
    sample_rate: u32,
    channels: u16,
}

#[derive(Debug, Serialize, Deserialize)]
struct AudioOutputConfig {
    encoding: String,
    sample_rate: u32,
    channels: u16,
    container: String,
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
    
    fn start_capture(&self, tx: mpsc::UnboundedSender<Vec<u8>>) -> Result<Stream, Box<dyn std::error::Error>> {
        let config = self.config.clone();
        
        let stream = match self.sample_format {
            SampleFormat::F32 => self.build_stream::<f32>(config, tx)?,
            SampleFormat::I16 => self.build_stream::<i16>(config, tx)?,
            SampleFormat::U16 => self.build_stream::<u16>(config, tx)?,
            _ => return Err("Unsupported sample format".into()),
        };
        
        stream.play()?;
        Ok(stream)
    }
    
    fn build_stream<T>(&self, config: StreamConfig, tx: mpsc::UnboundedSender<Vec<u8>>) -> Result<Stream, Box<dyn std::error::Error>>
    where
        T: cpal::Sample + cpal::SizedSample + Send + 'static,
        f32: cpal::FromSample<T>,
    {
        let stream = self.device.build_input_stream(
            &config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
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
    // For now, we'll just log audio reception since rodio setup is complex
    // In a real implementation, you'd set up proper audio playback
}

impl AudioPlayer {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(AudioPlayer {})
    }
    
    fn play_audio(&self, audio_data: Vec<u8>) -> Result<(), Box<dyn std::error::Error>> {
        info!("🔊 Received audio data for playback: {} bytes", audio_data.len());
        // TODO: Implement actual audio playback with rodio
        // For now, we just log that we received audio
        Ok(())
    }
}

async fn connect_to_voice_agent(api_key: &str, _sample_rate: u32, _channels: u16) -> Result<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, Box<dyn std::error::Error>> {
    let url = "wss://agent.deepgram.com/v1/agent/converse";
    let url = Url::parse(url)?;
    
    let request = tokio_tungstenite::tungstenite::handshake::client::Request::get(url.as_str())
        .header("Authorization", format!("Token {}", api_key))
        .header("Host", url.host_str().unwrap_or("api.deepgram.com"))
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

fn create_agent_config(sample_rate: u32, channels: u16) -> VoiceAgentConfig {
    VoiceAgentConfig {
        message_type: "agent_config".to_string(),
        config: AgentConfig {
            agent: AgentSettings {
                listen: ListenConfig {
                    model: "nova-2".to_string(),
                    language: "en".to_string(),
                    smart_format: true,
                    interim_results: true,
                },
                think: ThinkConfig {
                    provider: "open_ai".to_string(),
                    model: "gpt-4".to_string(),
                    instructions: "You are a helpful AI assistant. Keep your responses conversational and concise.".to_string(),
                },
                speak: SpeakConfig {
                    model: "aura-asteria-en".to_string(),
                },
            },
            audio: AudioConfig {
                input: AudioInputConfig {
                    encoding: "linear16".to_string(),
                    sample_rate,
                    channels,
                },
                output: AudioOutputConfig {
                    encoding: "linear16".to_string(),
                    sample_rate: 24000,
                    channels: 1,
                    container: "none".to_string(),
                },
            },
        },
    }
}

async fn handle_voice_agent_responses(
    mut ws_receiver: futures_util::stream::SplitStream<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>>,
    audio_player: Arc<AudioPlayer>,
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
                                // Audio data should be in the response, but the exact format depends on the API
                                // This is a placeholder - you'll need to extract the actual audio data
                                if let Some(audio_data) = response.data.get("audio") {
                                    if let Some(audio_str) = audio_data.as_str() {
                                        // Decode base64 audio data
                                        if let Ok(decoded_audio) = general_purpose::STANDARD.decode(audio_str) {
                                            if let Err(e) = audio_player.play_audio(decoded_audio) {
                                                error!("Failed to play audio: {}", e);
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
                // Handle binary audio data directly
                if let Err(e) = audio_player.play_audio(data.to_vec()) {
                    error!("Failed to play binary audio: {}", e);
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
    // Initialize logging
    env_logger::init();
    
    // Load environment variables
    dotenv::dotenv().ok();
    
    let api_key = env::var("DEEPGRAM_API_KEY")
        .map_err(|_| "DEEPGRAM_API_KEY environment variable not set")?;
    
    info!("Starting Deepgram Voice Agent...");
    
    // Initialize audio capture
    let audio_capture = AudioCapture::new()?;
    let sample_rate = audio_capture.config.sample_rate.0;
    let channels = audio_capture.config.channels;
    
    info!("Audio config - Sample rate: {}, Channels: {}", sample_rate, channels);
    
    // Initialize audio player
    let audio_player = Arc::new(AudioPlayer::new()?);
    
    // Create channel for audio data
    let (audio_tx, mut audio_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    
    // Start audio capture
    let _stream = audio_capture.start_capture(audio_tx)?;
    info!("Audio capture started");
    
    // Connect to Deepgram Voice Agent
    let ws_stream = connect_to_voice_agent(&api_key, sample_rate, channels).await?;
    let (mut ws_sender, ws_receiver) = ws_stream.split();
    
    // Send initial configuration
    let config = create_agent_config(sample_rate, channels);
    let config_json = serde_json::to_string(&config)?;
    info!("📤 Sending agent configuration to WebSocket...");
    info!("📄 Config: {}", config_json);
    ws_sender.send(Message::Text(config_json.into())).await?;
    info!("✅ Agent configuration sent successfully");
    
    // Wait a moment for configuration to be processed
    sleep(Duration::from_millis(500)).await;
    
    // Spawn task to handle WebSocket responses
    let audio_player_clone = Arc::clone(&audio_player);
    let response_handle = tokio::spawn(async move {
        handle_voice_agent_responses(ws_receiver, audio_player_clone).await;
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
