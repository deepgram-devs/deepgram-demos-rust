use std::env;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Stream, StreamConfig};
use futures_util::{SinkExt, StreamExt};
use log::{error, info};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use url::Url;

#[derive(Debug, Deserialize)]
struct DeepgramResponse {
    #[serde(rename = "type")]
    message_type: String,
    #[serde(flatten)]
    data: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct AudioInput {
    encoding: String,
    sample_rate: u32,
}

#[derive(Debug, Serialize)]
struct AudioOutput {
    encoding: String,
    sample_rate: u32,
    bitrate: u32,
    container: String,
}

#[derive(Debug, Serialize)]
struct Audio {
    input: AudioInput,
    output: AudioOutput,
}

#[derive(Debug, Serialize)]
struct SettingsMessage {
    #[serde(rename = "type")]
    message_type: String,
    audio: Audio,
}


struct AudioCapture {
    device: Device,
    config: StreamConfig,
}

impl AudioCapture {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or("No input device available")?;

        info!("Using input device: {}", device.name()?);

        // Get the default supported configuration and try to use it
        let supported_config = device.default_input_config()?;
        info!("Default config: channels={}, sample_rate={:?}, sample_format={:?}",
              supported_config.channels(),
              supported_config.sample_rate(),
              supported_config.sample_format());
        
        // Use the default sample rate from the supported config
        let config = StreamConfig {
            channels: 1.min(supported_config.channels()), // Use mono if possible, otherwise use what's available
            sample_rate: supported_config.sample_rate(),
            buffer_size: cpal::BufferSize::Default,
        };

        info!("Using config: channels={}, sample_rate={:?}", config.channels, config.sample_rate);

        Ok(AudioCapture { device, config })
    }

    fn start_capture(&self, tx: mpsc::UnboundedSender<Vec<u8>>) -> Result<Stream, Box<dyn std::error::Error>> {
        let tx_clone = tx.clone();
        
        let stream = self.device.build_input_stream(
            &self.config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                // Convert f32 samples to 16-bit linear PCM bytes
                let mut bytes = Vec::with_capacity(data.len() * 2);
                
                for &sample in data {
                    // Convert f32 sample (-1.0 to 1.0) to i16 (-32768 to 32767)
                    let sample_i16 = (sample.clamp(-1.0, 1.0) * 32767.0) as i16;
                    bytes.extend_from_slice(&sample_i16.to_le_bytes());
                }
                
                if let Err(e) = tx_clone.send(bytes) {
                    error!("Failed to send audio data: {}", e);
                }
            },
            |err| error!("Audio stream error: {}", err),
            None,
        )?;

        stream.play()?;
        Ok(stream)
    }
}

async fn connect_to_deepgram(api_key: &str) -> Result<(tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, tokio_tungstenite::tungstenite::http::Response<Option<Vec<u8>>>), Box<dyn std::error::Error>> {
    
    let url = format!(
        "wss://api.preview.deepgram.com/v2/listen?model=flux-general-en&sample_rate=44100&encoding=linear16"
    );
    
    let url = Url::parse(&url)?;
    
    // Create a simple request with authorization header
    let request = tokio_tungstenite::tungstenite::handshake::client::Request::get(url.as_str())
        .header("Authorization", format!("Token {}", api_key))
        .header("Host", url.host_str().unwrap_or("api.preview.deepgram.com"))
        .header("Upgrade", "websocket")
        .header("Connection", "Upgrade")
        .header("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==")
        .header("Sec-WebSocket-Version", "13")
        .body(())?;
    
    info!("Connecting to Deepgram WebSocket...");
    let (ws_stream, response) = connect_async(request).await?;
    info!("Connected to Deepgram WebSocket successfully");
    
    Ok((ws_stream, response))
}

async fn send_settings_message(
    ws_sender: &mut futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
        Message,
    >,
) -> Result<(), Box<dyn std::error::Error>> {
    let settings = SettingsMessage {
        message_type: "Settings".to_string(),
        audio: Audio {
            input: AudioInput {
                encoding: "linear16".to_string(),
                sample_rate: 44100,
            },
            output: AudioOutput {
                encoding: "mp3".to_string(),
                sample_rate: 24000,
                bitrate: 48000,
                container: "none".to_string(),
            },
        },
    };

    let settings_json = serde_json::to_string(&settings)?;
    info!("Sending settings message: {}", settings_json);
    
    ws_sender.send(Message::Text(settings_json.into())).await?;
    info!("Settings message sent successfully");
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    // Get API key from environment variable
    let api_key = env::var("DEEPGRAM_API_KEY")
        .map_err(|_| "DEEPGRAM_API_KEY environment variable not set")?;
    
    info!("Starting Rust Flux WebSocket client...");
    
    // Initialize audio capture
    let audio_capture = AudioCapture::new()?;
    
    // Create channel for audio data
    let (audio_tx, mut audio_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    
    // Start audio capture
    let _stream = audio_capture.start_capture(audio_tx)?;
    info!("Audio capture started");
    
    // Connect to Deepgram WebSocket
    let (ws_stream, _response) = connect_to_deepgram(&api_key).await?;
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    
    // Send settings message immediately after connection
    send_settings_message(&mut ws_sender).await?;
    
    // Spawn task to handle WebSocket responses
    let response_handle = tokio::spawn(async move {
        while let Some(message) = ws_receiver.next().await {
            match message {
                Ok(Message::Text(text)) => {
                    match serde_json::from_str::<DeepgramResponse>(&text) {
                        Ok(response) => {
                            println!("📨 Message Type: {}", response.message_type);
                            println!("📄 Response Data: {}", serde_json::to_string_pretty(&response.data).unwrap_or_default());
                            println!("---");
                        }
                        Err(e) => {
                            error!("Failed to parse response: {}", e);
                            println!("📨 Raw response: {}", text);
                            println!("---");
                        }
                    }
                }
                Ok(Message::Binary(data)) => {
                    println!("📨 Message Type: Binary");
                    println!("📄 Binary data received: {} bytes", data.len());
                    println!("---");
                }
                Ok(Message::Close(frame)) => {
                    println!("📨 Message Type: Close");
                    if let Some(frame) = frame {
                        println!("📄 Close frame: code={}, reason={}", frame.code, frame.reason);
                    }
                    println!("---");
                    break;
                }
                Ok(Message::Ping(data)) => {
                    println!("📨 Message Type: Ping ({} bytes)", data.len());
                }
                Ok(Message::Pong(data)) => {
                    println!("📨 Message Type: Pong ({} bytes)", data.len());
                }
                Ok(Message::Frame(_)) => {
                    // Handle frame messages if needed
                    println!("📨 Message Type: Frame");
                }
                Err(e) => {
                    error!("WebSocket error: {}", e);
                    break;
                }
            }
        }
    });
    
    // Main loop: send audio data to WebSocket
    info!("Starting audio streaming to Deepgram...");
    println!("🎤 Listening to microphone and streaming to Deepgram Flux API...");
    println!("Press Ctrl+C to stop");
    println!("===");
    
    let mut audio_handle = tokio::spawn(async move {
        let mut total_bytes_sent = 0u64;
        let mut packet_count = 0u64;
        
        while let Some(audio_data) = audio_rx.recv().await {
            packet_count += 1;
            total_bytes_sent += audio_data.len() as u64;
            
            // Print audio data info every 50 packets to avoid spam
            if packet_count % 50 == 0 {
                println!("📤 Sent packet #{}: {} bytes (Total: {} bytes)",
                         packet_count, audio_data.len(), total_bytes_sent);
            }
            
            if let Err(e) = ws_sender.send(Message::Binary(audio_data.into())).await {
                error!("Failed to send audio data: {}", e);
                break;
            }
        }
    });
    
    // Wait for either task to complete or for Ctrl+C
    tokio::select! {
        _ = response_handle => {
            info!("Response handler completed");
        }
        _ = &mut audio_handle => {
            info!("Audio handler completed");
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Received Ctrl+C, shutting down...");
        }
    }
    
    // Clean shutdown
    audio_handle.abort();
    
    println!("🛑 Application stopped");
    Ok(())
}
