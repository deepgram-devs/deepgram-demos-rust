use std::env;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Stream, StreamConfig};
use futures_util::{SinkExt, StreamExt};
use log::{error, info};
use serde::Deserialize;
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

#[derive(Parser)]
#[command(name = "rust-flux")]
#[command(about = "Deepgram Flux transcription client", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Stream audio from microphone to Deepgram Flux API
    Microphone {
        /// Custom endpoint base URL (e.g., ws://localhost:8119/)
        #[arg(long)]
        endpoint: Option<String>,
        
        /// Sample rate in Hz (default: 44100)
        #[arg(long, default_value = "44100")]
        sample_rate: u32,
        
        /// Audio encoding format (default: linear16)
        #[arg(long, default_value = "linear16")]
        encoding: String,
    },
    /// Stream audio from a file to Deepgram Flux API
    File {
        /// Path to the audio file to transcribe
        #[arg(long)]
        file: PathBuf,
        
        /// Custom endpoint base URL (e.g., ws://localhost:8119/)
        #[arg(long)]
        endpoint: Option<String>,
        
        /// Sample rate in Hz (default: 44100)
        #[arg(long, default_value = "44100")]
        sample_rate: u32,
        
        /// Audio encoding format (default: linear16)
        #[arg(long, default_value = "linear16")]
        encoding: String,
    },
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
        info!(
            "Default config: channels={}, sample_rate={:?}, sample_format={:?}",
            supported_config.channels(),
            supported_config.sample_rate(),
            supported_config.sample_format()
        );

        // Use the default sample rate from the supported config
        let config = StreamConfig {
            channels: 1.min(supported_config.channels()), // Use mono if possible, otherwise use what's available
            sample_rate: supported_config.sample_rate(),
            buffer_size: cpal::BufferSize::Default,
        };

        info!(
            "Using config: channels={}, sample_rate={:?}",
            config.channels, config.sample_rate
        );

        Ok(AudioCapture { device, config })
    }

    fn start_capture(
        &self,
        tx: mpsc::UnboundedSender<Vec<u8>>,
    ) -> Result<Stream, Box<dyn std::error::Error>> {
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

async fn connect_to_deepgram(
    api_key: &str,
    endpoint: Option<&str>,
    sample_rate: u32,
    encoding: &str,
) -> Result<
    (
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        tokio_tungstenite::tungstenite::http::Response<Option<Vec<u8>>>,
    ),
    Box<dyn std::error::Error>,
> {
    let base_url = endpoint.unwrap_or("wss://api.deepgram.com");
    
    let url = format!(
        "{}/v2/listen?model=flux-general-en&sample_rate={}&encoding={}",
        base_url, sample_rate, encoding
    );

    let url = Url::parse(&url)?;

    // Create a simple request with authorization header
    let request = tokio_tungstenite::tungstenite::handshake::client::Request::get(url.as_str())
        .header("Authorization", format!("Token {}", api_key))
        .header("Host", url.host_str().unwrap_or("api.deepgram.com"))
        .header("Upgrade", "websocket")
        .header("Connection", "Upgrade")
        .header("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==")
        .header("Sec-WebSocket-Version", "13")
        .body(())?;

    info!("Connecting to Deepgram WebSocket at {}...", base_url);
    let (ws_stream, response) = connect_async(request).await?;
    info!("Connected to Deepgram WebSocket successfully");

    Ok((ws_stream, response))
}

async fn handle_websocket_responses(
    mut ws_receiver: futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
) {
    while let Some(message) = ws_receiver.next().await {
        match message {
            Ok(Message::Text(text)) => match serde_json::from_str::<DeepgramResponse>(&text) {
                Ok(response) => {
                    println!("📨 Message Type: {}", response.message_type);
                    println!(
                        "📄 Response Data: {}",
                        serde_json::to_string_pretty(&response.data).unwrap_or_default()
                    );
                    println!("---");
                }
                Err(e) => {
                    error!("Failed to parse response: {}", e);
                    println!("📨 Raw response: {}", text);
                    println!("---");
                }
            },
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
                println!("📨 Message Type: Frame");
            }
            Err(e) => {
                error!("WebSocket error: {}", e);
                break;
            }
        }
    }
}

async fn run_microphone(
    endpoint: Option<String>,
    sample_rate: u32,
    encoding: String,
) -> Result<(), Box<dyn std::error::Error>> {
    // Get API key from environment variable
    let api_key =
        env::var("DEEPGRAM_API_KEY").map_err(|_| "DEEPGRAM_API_KEY environment variable not set")?;

    info!("Starting microphone streaming to Deepgram Flux...");

    // Initialize audio capture
    let audio_capture = AudioCapture::new()?;

    // Create channel for audio data
    let (audio_tx, mut audio_rx) = mpsc::unbounded_channel::<Vec<u8>>();

    // Start audio capture
    let _stream = audio_capture.start_capture(audio_tx)?;
    info!("Audio capture started");

    // Connect to Deepgram WebSocket
    let (ws_stream, _response) = connect_to_deepgram(&api_key, endpoint.as_deref(), sample_rate, &encoding).await?;
    let (mut ws_sender, ws_receiver) = ws_stream.split();

    // Spawn task to handle WebSocket responses
    let response_handle = tokio::spawn(async move {
        handle_websocket_responses(ws_receiver).await;
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
                println!(
                    "📤 Sent packet #{}: {} bytes (Total: {} bytes)",
                    packet_count,
                    audio_data.len(),
                    total_bytes_sent
                );
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

async fn run_file(
    file_path: PathBuf,
    endpoint: Option<String>,
    sample_rate: u32,
    encoding: String,
) -> Result<(), Box<dyn std::error::Error>> {
    // Get API key from environment variable
    let api_key =
        env::var("DEEPGRAM_API_KEY").map_err(|_| "DEEPGRAM_API_KEY environment variable not set")?;

    info!("Starting file streaming to Deepgram Flux...");

    // Verify file exists
    if !file_path.exists() {
        return Err(format!("File not found: {}", file_path.display()).into());
    }

    info!("Reading audio file: {}", file_path.display());

    // Read the entire file into memory
    let audio_data = tokio::fs::read(&file_path).await?;
    info!("File loaded: {} bytes", audio_data.len());

    // Connect to Deepgram WebSocket
    let (ws_stream, _response) = connect_to_deepgram(&api_key, endpoint.as_deref(), sample_rate, &encoding).await?;
    let (mut ws_sender, ws_receiver) = ws_stream.split();

    // Spawn task to handle WebSocket responses
    let response_handle = tokio::spawn(async move {
        handle_websocket_responses(ws_receiver).await;
    });

    println!("📁 Streaming file to Deepgram Flux API...");
    println!("File: {}", file_path.display());
    println!("Press Ctrl+C to stop");
    println!("===");

    // Stream the audio data in chunks
    let chunk_size = 8192*4;
    let mut offset = 0;
    let mut chunk_count = 0;
    let mut total_bytes_sent = 0;

    info!("Starting to stream {} bytes in chunks of {} bytes", audio_data.len(), chunk_size);

    while offset < audio_data.len() {
        let end = (offset + chunk_size).min(audio_data.len());
        let chunk = &audio_data[offset..end];

        chunk_count += 1;
        total_bytes_sent += chunk.len();
        
        // Print progress for every chunk initially, then every 10 chunks
        if chunk_count <= 5 || chunk_count % 10 == 0 {
            println!(
                "📤 Sending chunk #{}: {} bytes (Progress: {:.1}%, Total sent: {} bytes)",
                chunk_count,
                chunk.len(),
                (total_bytes_sent as f64 / audio_data.len() as f64) * 100.0,
                total_bytes_sent
            );
        }

        match ws_sender.send(Message::Binary(chunk.to_vec().into())).await {
            Ok(_) => {
                info!("Successfully sent chunk #{} ({} bytes)", chunk_count, chunk.len());
            }
            Err(e) => {
                error!("Failed to send audio data chunk #{}: {}", chunk_count, e);
                break;
            }
        }

        offset = end;

        // Small delay to simulate streaming (adjust as needed)
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    }

    println!("✅ File streaming complete: {} chunks sent ({} total bytes)", chunk_count, total_bytes_sent);

    // Send close frame to signal end of audio
    let _ = ws_sender.close().await;

    // Wait for response handler to complete or timeout
    tokio::select! {
        _ = response_handle => {
            info!("Response handler completed");
        }
        _ = tokio::time::sleep(tokio::time::Duration::from_secs(5)) => {
            info!("Timeout waiting for final responses");
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Received Ctrl+C, shutting down...");
        }
    }

    println!("🛑 Application stopped");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Microphone { endpoint, sample_rate, encoding } => {
            run_microphone(endpoint, sample_rate, encoding).await?;
        }
        Commands::File { file, endpoint, sample_rate, encoding } => {
            run_file(file, endpoint, sample_rate, encoding).await?;
        }
    }

    Ok(())
}
