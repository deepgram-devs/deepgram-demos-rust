mod stream;
mod tags;

pub const CLIENT_USER_AGENT: &str = concat!("dg-tts/", env!("CARGO_PKG_VERSION"));

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
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
    Speak(SpeakArgs),
    /// Save text-to-speech audio to a file
    Save(SaveArgs),
    /// Stream text-to-speech using WebSocket connection
    Stream(StreamArgs),
}

#[derive(Args)]
struct SpeakArgs {
    #[command(subcommand)]
    command: Option<SpeakSubcommand>,

    #[command(flatten)]
    options: InteractiveOptions,
}

#[derive(Subcommand)]
enum SpeakSubcommand {
    /// Use the Flux TTS v2 batch API
    V2(FluxBatchOptions),
}

#[derive(Args)]
struct SaveArgs {
    #[command(subcommand)]
    command: Option<SaveSubcommand>,

    #[command(flatten)]
    options: SaveOptions,
}

#[derive(Subcommand)]
enum SaveSubcommand {
    /// Use the Flux TTS v2 batch API
    V2(FluxSaveOptions),
}

#[derive(Args)]
struct StreamArgs {
    #[command(subcommand)]
    command: Option<StreamSubcommand>,

    #[command(flatten)]
    options: StreamOptions,
}

#[derive(Subcommand)]
enum StreamSubcommand {
    /// Use the Flux TTS v2 streaming API
    V2(FluxStreamOptions),
}

#[derive(Args)]
struct InteractiveOptions {
    /// Voice model to use (e.g., "aura-2")
    #[arg(long, default_value = "aura-2-thalia-en")]
    voice: String,

    /// Optional request tags
    #[arg(long)]
    tags: Option<String>,

    /// Override the base URL endpoint (e.g., "https://api.deepgram.com")
    #[arg(long, default_value = "https://api.deepgram.com")]
    endpoint: String,
}

#[derive(Args)]
struct SaveOptions {
    /// Text to convert to speech
    #[arg(long)]
    text: Option<String>,

    /// Output file path
    #[arg(long)]
    output: Option<String>,

    /// Voice model to use (e.g., "aura-2")
    #[arg(long)]
    voice: Option<String>,

    /// Optional request tags
    #[arg(long)]
    tags: Option<String>,

    /// Override the base URL endpoint (e.g., "https://api.deepgram.com")
    #[arg(long)]
    endpoint: Option<String>,
}

#[derive(Args)]
struct StreamOptions {
    /// Voice model to use (e.g., "aura-2")
    #[arg(long, default_value = "aura-2-thalia-en")]
    voice: String,

    /// Optional request tags
    #[arg(long)]
    tags: Option<String>,

    /// Override the base URL endpoint (e.g., "wss://api.deepgram.com")
    #[arg(long, default_value = "wss://api.deepgram.com")]
    endpoint: String,
}

#[derive(Args)]
struct FluxBatchOptions {
    /// Flux voice model, such as `flux-haley-en`
    #[arg(long, default_value = "flux-haley-en")]
    voice: String,

    /// Optional comma-separated request tags
    #[arg(long)]
    tags: Option<String>,

    /// Override the base URL endpoint
    #[arg(long, default_value = "https://api.deepgram.com")]
    endpoint: String,
}

#[derive(Args)]
struct FluxSaveOptions {
    /// Text to convert to speech
    #[arg(long)]
    text: String,

    /// Output file path
    #[arg(long)]
    output: String,

    /// Flux voice model, such as `flux-haley-en`
    #[arg(long, default_value = "flux-haley-en")]
    voice: String,

    /// Optional comma-separated request tags
    #[arg(long)]
    tags: Option<String>,

    /// Override the base URL endpoint
    #[arg(long, default_value = "https://api.deepgram.com")]
    endpoint: String,
}

#[derive(Args)]
struct FluxStreamOptions {
    /// Flux voice model, such as `flux-haley-en`
    #[arg(long, default_value = "flux-haley-en")]
    voice: String,

    /// Optional comma-separated request tags
    #[arg(long)]
    tags: Option<String>,

    /// Override the WebSocket base URL endpoint
    #[arg(long, default_value = "wss://api.deepgram.com")]
    endpoint: String,
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
    endpoint: &str,
    api_version: u8,
) -> Result<Vec<u8>> {
    let request = TtsRequest {
        text: text.to_string(),
    };

    // println!("Request is: {0}", serde_json::to_string(&request)?);

    let url = format!("{}/v{}/speak", endpoint, api_version);
    let mut request = client
        .post(&url)
        .header(reqwest::header::USER_AGENT, CLIENT_USER_AGENT)
        .header("Authorization", format!("Token {}", api_key))
        .header("Content-Type", "application/json")
        .query(&[("model", voice)])
        .json(&request);

    for tag in tags::request_tags(tags.as_deref()) {
        request = request.query(&[("tag", tag)]);
    }

    let response = request.send().await.context("Failed to send TTS request")?;

    let payload = response
        .bytes()
        .await
        .expect("Failed to get response bytes");

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

#[cfg(test)]
mod tests {
    use super::CLIENT_USER_AGENT;

    #[test]
    fn client_user_agent_identifies_application_and_version() {
        assert_eq!(CLIENT_USER_AGENT, "dg-tts/0.2.8");
    }
}

fn save_audio(audio_bytes: Vec<u8>, output_path: &str) -> Result<()> {
    let mut file = File::create(output_path)
        .context(format!("Failed to create output file: {}", output_path))?;

    file.write_all(&audio_bytes)
        .context("Failed to write audio data to file")?;

    println!("Audio saved to: {}", output_path);
    Ok(())
}

async fn run_interactive_speak(
    client: &Client,
    api_key: &str,
    voice: String,
    tags: Option<String>,
    endpoint: String,
    api_version: u8,
) -> Result<()> {
    let (tx, rx) = mpsc::channel();
    let output_stream = OutputStreamBuilder::open_default_stream()?;

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

    while let Ok(text) = rx.recv() {
        match generate_tts(
            client,
            api_key,
            &text,
            &voice,
            tags.clone(),
            &endpoint,
            api_version,
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

    input_thread.join().unwrap();
    Ok(())
}

async fn save_generated_audio(
    client: &Client,
    api_key: &str,
    text: &str,
    output: &str,
    voice: &str,
    tags: Option<String>,
    endpoint: &str,
    api_version: u8,
) -> Result<()> {
    println!("Generating audio for: {}", text);
    let audio_bytes =
        generate_tts(client, api_key, text, voice, tags, endpoint, api_version).await?;
    save_audio(audio_bytes, output)
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok(); // Load .env file if it exists
    let api_key = env::var("DEEPGRAM_API_KEY").context("DEEPGRAM_API_KEY must be set")?;

    let cli = Cli::parse();
    let client = Client::new();

    match cli.command {
        Some(Commands::Speak(args)) => match args.command {
            Some(SpeakSubcommand::V2(options)) => {
                let FluxBatchOptions {
                    voice,
                    tags,
                    endpoint,
                } = options;
                let (tx, rx) = mpsc::channel();
                let output_stream = OutputStreamBuilder::open_default_stream().unwrap();

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

                async move {
                    while let Ok(text) = rx.recv() {
                        match generate_tts(
                            &client,
                            &api_key,
                            &text,
                            &voice,
                            tags.clone(),
                            &endpoint,
                            2,
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
                }
                .await;

                input_thread.join().unwrap();
            }
            None => {
                let InteractiveOptions {
                    voice,
                    tags,
                    endpoint,
                } = args.options;
                run_interactive_speak(&client, &api_key, voice, tags, endpoint, 1).await?;
            }
        },
        Some(Commands::Save(args)) => match args.command {
            Some(SaveSubcommand::V2(options)) => {
                let FluxSaveOptions {
                    text,
                    output,
                    voice,
                    tags,
                    endpoint,
                } = options;
                save_generated_audio(
                    &client, &api_key, &text, &output, &voice, tags, &endpoint, 2,
                )
                .await?;
            }
            None => {
                let SaveOptions {
                    text,
                    output,
                    voice,
                    tags,
                    endpoint,
                } = args.options;
                let text = text.context("--text is required for the save command")?;
                let output = output.context("--output is required for the save command")?;
                save_generated_audio(
                    &client,
                    &api_key,
                    &text,
                    &output,
                    voice.as_deref().unwrap_or("aura-2-thalia-en"),
                    tags,
                    endpoint.as_deref().unwrap_or("https://api.deepgram.com"),
                    1,
                )
                .await?;
            }
        },
        Some(Commands::Stream(args)) => match args.command {
            Some(StreamSubcommand::V2(options)) => {
                stream::run_stream_v2(&api_key, &options.voice, options.tags, &options.endpoint)
                    .await?;
            }
            None => {
                stream::run_stream(
                    &api_key,
                    &args.options.voice,
                    args.options.tags,
                    &args.options.endpoint,
                )
                .await?;
            }
        },
        None => {
            println!("No command specified. Use --help for usage information.");
        }
    }

    Ok(())
}
