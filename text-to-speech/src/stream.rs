use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use rodio::{OutputStreamBuilder, Sink};
use serde::{Deserialize, Serialize};
use std::io::{Write};
use std::sync::mpsc;
use std::thread;
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[derive(Serialize, Deserialize, Debug)]
struct TtsStreamRequest {
    #[serde(rename = "type")]
    msg_type: String,
    text: String,
}

pub async fn run_stream(api_key: &str, voice: &str, tags: Option<String>, endpoint: &str) -> Result<()> {
    // Build WebSocket URL
    let mut url = format!(
        "{}/v1/speak?model={}&encoding=linear16&sample_rate=24000",
        endpoint,
        voice
    );
    
    if let Some(tag_value) = tags {
        url.push_str(&format!("&tag={}", tag_value));
    }

    println!("Connecting to Deepgram TTS WebSocket...");

    // Extract host from endpoint for the host header
    let host = endpoint.replace("wss://", "").replace("ws://", "");

    // Connect to WebSocket with required headers
    let request = tokio_tungstenite::tungstenite::http::Request::builder()
        .uri(&url)
        // .header("Authorization", format!("Token {}", api_key))
        .header("upgrade", "websocket")
        .header("connection", "Upgrade")
        .header("host", &host)
        .header("sec-websocket-key", "YXNkZmFzZGZhc2RmYXNkZgo=")
        .header("Sec-WebSocket-Version", "13")
        .header("Sec-WebSocket-Protocol", format!("token, {}", api_key))
        .body(())
        .context("Failed to build WebSocket request")?;

    let (ws_stream, _) = connect_async(request)
        .await
        .context("Failed to connect to WebSocket")?;

    println!("Connected! Type your text and press Enter to hear it spoken.");
    println!("Type 'quit' to quit.\n");

    let (mut write, mut read) = ws_stream.split();

    // Create channel for audio playback
    let (audio_tx, audio_rx) = mpsc::channel::<Vec<u8>>();

    // Spawn audio playback thread
    let audio_thread = thread::spawn(move || {
        // println!("Starting audio playback thread ...");
        let output_stream = match OutputStreamBuilder::open_default_stream() {
            Ok(stream) => stream,
            Err(e) => {
                eprintln!("Failed to open audio output stream: {}", e);
                return;
            }
        };

        // Create a single sink that will be reused for all audio chunks
        let sink = Sink::connect_new(&output_stream.mixer());

        // println!("Waiting for audio data ...");
        while let Ok(audio_bytes) = audio_rx.recv() {
            // println!("Received audio chunk of {} bytes", audio_bytes.len());
            if let Err(e) = append_audio_to_sink(audio_bytes, &sink) {
                eprintln!("Error appending audio: {}", e);
            }
        }
        
        // Wait for all audio to finish playing before exiting
        sink.sleep_until_end();
    });

    // Spawn task to handle incoming WebSocket messages
    let audio_tx_clone = audio_tx;
    let audio_task = tokio::spawn(async move {
        while let Some(message) = read.next().await {
            // println!("Read a WebSocket message from the server");
            match message {
                Ok(Message::Binary(data)) => {
                    // println!("Received binary payload response");
                    // Send audio data to playback thread
                    if let Err(e) = audio_tx_clone.send(data.to_vec()) {
                        eprintln!("Failed to send audio to playback thread: {}", e);
                        break;
                    }
                }
                Ok(Message::Text(_text)) => {
                    // Handle any text messages from server (metadata, errors, etc.)
                    // println!("Server message: {}", text);
                }
                Ok(Message::Close(_)) => {
                    println!("WebSocket connection closed by server");
                    break;
                }
                Err(e) => {
                    eprintln!("WebSocket error: {}", e);
                    break;
                }
                _ => {
                    // println!("Other response received ...");
                }
            }
        }
    });

    // Main input loop
    loop {
        print!("> ");
        std::io::stdout().flush()?;

        let mut input = String::new();
        std::io::stdin()
            .read_line(&mut input)
            .context("Failed to read input")?;

        let input = input.trim();

        if input == "quit" {
            println!("Quitting Deepgram TTS streaming application...");
            break;
        }

        if input.is_empty() {
            continue;
        }

        // Send text to Deepgram
        let request = TtsStreamRequest {
            msg_type: "Speak".to_string(),
            text: input.to_string(),
        };

        let json = serde_json::to_string(&request)
            .context("Failed to serialize request")?;

        if let Err(e) = write.send(Message::Text(json.into())).await {
            eprintln!("Failed to send message: {}", e);
            break;
        }

        // Send Flush message to process the audio immediately
        let flush_msg = serde_json::json!({ "type": "Flush" });
        if let Err(e) = write.send(Message::Text(flush_msg.to_string().into())).await {
            eprintln!("Failed to send flush message: {}", e);
            break;
        }
        else {
            println!("Sent a message to the server");
        }
    }

    println!("Client requesting to close connection");
    let _ = write.send(Message::Close(None)).await;

    // Wait for audio task to complete
    let _ = audio_task.await;

    // Drop the audio sender to signal the audio thread to exit
    // drop(audio_tx);
    
    // Wait for audio thread to finish
    let _ = audio_thread.join();

    Ok(())
}

fn append_audio_to_sink(audio_bytes: Vec<u8>, sink: &Sink) -> Result<()> {
    use rodio::buffer::SamplesBuffer;

    // Convert raw PCM bytes to i16 samples, then to f32 for rodio
    // linear16 is 16-bit signed PCM, so we need to convert bytes to i16, then normalize to f32
    let samples: Vec<f32> = audio_bytes
        .chunks_exact(2)
        .map(|chunk| {
            let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
            // Normalize i16 to f32 range [-1.0, 1.0]
            sample as f32 / i16::MAX as f32
        })
        .collect();

    // Create a buffer with the samples at 24000 Hz sample rate (as specified in the URL)
    // Assuming mono audio (1 channel)
    let buffer = SamplesBuffer::new(1, 24000, samples);
    
    // Append to the existing sink for continuous playback
    sink.append(buffer);
    
    Ok(())
}
