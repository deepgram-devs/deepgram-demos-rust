use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use dotenv::dotenv;
use reqwest::Client;
use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs::File;
use std::io::{Cursor, Write};
use std::sync::mpsc;
use std::thread;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Speak text using Deepgram TTS
    Speak {
        /// Voice model to use (e.g., "aura-2")
        #[arg(long, default_value = "aura-2-thalia-en")]
        voice: String,

        /// Optional request tags
        #[arg(long)]
        tags: Option<String>,
    },
    /// Save text-to-speech audio to a file
    Save {
        /// Text to convert to speech
        #[arg(long)]
        text: String,

        /// Output file path
        #[arg(long)]
        output: String,

        /// Voice model to use (e.g., "aura-2")
        #[arg(long, default_value = "aura-2-thalia-en")]
        voice: String,

        /// Optional request tags
        #[arg(long)]
        tags: Option<String>,
    },
}

#[derive(Serialize, Deserialize, Debug)]
struct TtsRequest {
    text: String,
}

async fn generate_tts(
    client: &Client,
    api_key: &str,
    text: &str,
    voice: &str,
    tags: Option<String>,
) -> Result<Vec<u8>> {
    let request = TtsRequest {
        text: text.to_string(),
    };

    // println!("Request is: {0}", serde_json::to_string(&request)?);

    let mut request = client
        .post("https://api.deepgram.com/v1/speak")
        .header("Authorization", format!("Token {}", api_key))
        .header("Content-Type", "application/json")
        .query(&[("model", voice)])
        .json(&request);
    
    if tags.is_some() {
        request = request.query(&[("tag", tags)]);
    }

    let response = request
        .send()
        .await
        .context("Failed to send TTS request")?;

    let payload = response.bytes().await.expect("Failed to get response bytes"); 

    // println!("\nResponse length is: {0}", payload.len());

    Ok(payload.into())
}

fn play_audio(audio_bytes: Vec<u8>, output_stream: &OutputStream) -> Result<()> {
    let sink = Sink::connect_new(&output_stream.mixer());

    let cursor = Cursor::new(audio_bytes);
    let source = Decoder::new(cursor)?;
    sink.append(source);

    sink.sleep_until_end();
    Ok(())
}

fn save_audio(audio_bytes: Vec<u8>, output_path: &str) -> Result<()> {
    let mut file = File::create(output_path)
        .context(format!("Failed to create output file: {}", output_path))?;
    
    file.write_all(&audio_bytes)
        .context("Failed to write audio data to file")?;
    
    println!("Audio saved to: {}", output_path);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok(); // Load .env file if it exists
    let api_key = env::var("DEEPGRAM_API_KEY")
        .context("DEEPGRAM_API_KEY must be set")?;

    let cli = Cli::parse();
    let client = Client::new();

    match &cli.command {
        Some(Commands::Speak {
            voice,
            tags,
        }) => {
            let (tx, rx) = mpsc::channel();
            let output_stream = OutputStreamBuilder::open_default_stream().unwrap();

            // Input thread
            let input_thread = thread::spawn(move || {
                let mut input = String::new();
                print!("Enter text to speak (type 'quit' to exit): ");
                std::io::stdout().flush().unwrap();

                while std::io::stdin().read_line(&mut input).is_ok() {
                    if input.trim() == "quit" {
                        break;
                    }

                    tx.send(input.clone()).unwrap();
                    input.clear();
                    print!("Enter text to speak (type 'quit' to exit): ");
                    std::io::stdout().flush().unwrap();
                }
            });

            // TTS and playback thread


            async move {
                while let Ok(text) = rx.recv() {
                    match generate_tts(
                        &client,
                        &api_key,
                        &text,
                        &voice,
                        tags.clone(),
                    )
                    .await
                    {
                        Ok(audio_bytes) => {
                            if let Err(e) = play_audio(audio_bytes, &output_stream) {
                                eprintln!("Error playing audio: {}", e);
                            }
                        }
                        Err(e) => eprintln!("TTS generation error: {:?}", e),
                    }
                }
            }.await;


            input_thread.join().unwrap();
            // tts_thread.await?;
        }
        Some(Commands::Save {
            text,
            output,
            voice,
            tags,
        }) => {
            println!("Generating audio for: {}", text);
            
            match generate_tts(&client, &api_key, text, voice, tags.clone()).await {
                Ok(audio_bytes) => {
                    if let Err(e) = save_audio(audio_bytes, output) {
                        eprintln!("Error saving audio: {}", e);
                        return Err(e);
                    }
                }
                Err(e) => {
                    eprintln!("TTS generation error: {:?}", e);
                    return Err(e);
                }
            }
        }
        None => {
            println!("No command specified. Use --help for usage information.");
        }
    }

    Ok(())
}
