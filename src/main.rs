use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use futures_util::{SinkExt, StreamExt};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Stream, StreamConfig, SampleFormat};
use serde::{Deserialize, Serialize};
use dotenv::dotenv;
use std::env;

#[derive(Debug, Serialize, Deserialize)]
struct DeepgramConfig {
    #[serde(rename = "type")]
    message_type: String,
    config: TranscriptionConfig,
}

#[derive(Debug, Serialize, Deserialize)]
struct TranscriptionConfig {
    encoding: String,
    sample_rate: u32,
    channels: u16,
    interim_results: bool,
    punctuate: bool,
    smart_format: bool,
}

#[derive(Debug, Deserialize)]
struct DeepgramResponse {
    #[serde(rename = "type")]
    message_type: String,
    channel: Option<Channel>,
}

#[derive(Debug, Deserialize)]
struct Channel {
    alternatives: Vec<Alternative>,
}

#[derive(Debug, Deserialize)]
struct Alternative {
    transcript: String,
    confidence: Option<f64>,
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
        
        println!("Input device: {}", device.name()?);
        
        let supported_config = device.default_input_config()?;
        println!("Default input config: {:?}", supported_config);
        
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
            |err| eprintln!("Audio stream error: {}", err),
            None,
        )?;
        
        Ok(stream)
    }
}

async fn run_deepgram_client(api_key: String, sample_rate: u32, channels: u16, mut audio_rx: mpsc::UnboundedReceiver<Vec<u8>>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let url = format!(
        "wss://api.deepgram.com/v1/listen?encoding=linear16&sample_rate={}&channels={}&interim_results=true&punctuate=true&smart_format=true",
        sample_rate, channels
    );
    
    println!("Connecting to Deepgram WebSocket...");
    
    // Parse the URL and create a proper WebSocket request
    let url_parsed = url::Url::parse(&url)?;
    let host = url_parsed.host_str().ok_or("Invalid host in URL")?;
    
    let request = tokio_tungstenite::tungstenite::http::Request::builder()
        .method("GET")
        .uri(&url)
        .header("Host", host)
        .header("Upgrade", "websocket")
        .header("Connection", "Upgrade")
        .header("Sec-WebSocket-Key", tokio_tungstenite::tungstenite::handshake::client::generate_key())
        .header("Sec-WebSocket-Version", "13")
        .header("Authorization", format!("Token {}", api_key))
        .body(())?;
    
    let (ws_stream, _) = connect_async(request).await?;
    println!("Connected to Deepgram!");
    
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    
    // Spawn task to handle WebSocket responses
    let response_handler = tokio::spawn(async move {
        while let Some(msg) = ws_receiver.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    match serde_json::from_str::<DeepgramResponse>(&text) {
                        Ok(response) => {
                            if response.message_type == "Results" {
                                if let Some(channel) = response.channel {
                                    for alternative in channel.alternatives {
                                        if !alternative.transcript.trim().is_empty() {
                                            print!("\rTranscript: {}", alternative.transcript);
                                            if let Some(confidence) = alternative.confidence {
                                                print!(" (Confidence: {:.1}%)", confidence * 100.0);
                                            }
                                            println!();
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => eprintln!("Failed to parse response: {}", e),
                    }
                }
                Ok(Message::Close(_)) => {
                    println!("WebSocket connection closed by server");
                    break;
                }
                Err(e) => {
                    eprintln!("WebSocket error: {}", e);
                    break;
                }
                _ => {}
            }
        }
    });
    
    // Send audio data to WebSocket
    while let Some(audio_data) = audio_rx.recv().await {
        if let Err(e) = ws_sender.send(Message::Binary(audio_data.into())).await {
            eprintln!("Failed to send audio to WebSocket: {}", e);
            break;
        }
    }
    
    // Clean up
    let _ = ws_sender.close().await;
    let _ = response_handler.await;
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables
    dotenv().ok();
    
    let api_key = env::var("DEEPGRAM_API_KEY")
        .map_err(|_| "DEEPGRAM_API_KEY environment variable not set")?;
    
    println!("Starting Deepgram real-time transcription...");
    
    // Initialize audio capture
    let audio_capture = AudioCapture::new()?;
    let sample_rate = audio_capture.config.sample_rate.0;
    let channels = audio_capture.config.channels;
    
    println!("Audio config - Sample rate: {}, Channels: {}", sample_rate, channels);
    
    // Create channel for audio data
    let (audio_tx, audio_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    
    // Start audio capture
    let _stream = audio_capture.start_capture(audio_tx)?;
    
    println!("Listening for audio... Press Ctrl+C to stop.");
    
    // Run Deepgram client
    let deepgram_task = tokio::spawn(run_deepgram_client(api_key.clone(), sample_rate, channels, audio_rx));
    
    // Wait for Ctrl+C
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            println!("\nShutting down...");
        }
        result = deepgram_task => {
            match result {
                Ok(Ok(())) => println!("Deepgram client finished successfully"),
                Ok(Err(e)) => eprintln!("Deepgram client error: {}", e),
                Err(e) => eprintln!("Task join error: {}", e),
            }
        }
    }
    
    Ok(())
}