use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use futures_util::{SinkExt, StreamExt};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Stream, StreamConfig, SampleFormat};
use serde::Deserialize;
use dotenv::dotenv;
use std::env;
use std::path::PathBuf;
use clap::{Parser, Subcommand};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use std::fs::File;
use std::time::Duration;

#[derive(Parser)]
#[command(name = "dg-stt")]
#[command(about = "Deepgram Speech-to-Text CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Stream audio from microphone for real-time transcription
    Microphone,
    /// Stream audio from a file for transcription
    File {
        /// Path to the audio file (supports MP3, WAV, FLAC)
        #[arg(short, long)]
        file: PathBuf,
        
        /// Stream audio as fast as possible instead of real-time rate
        #[arg(long)]
        fast: bool,
    },
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

struct AudioFileReader {
    path: PathBuf,
}

impl AudioFileReader {
    fn new(path: PathBuf) -> Self {
        AudioFileReader { path }
    }
    
    async fn stream_file(
        &self,
        tx: mpsc::UnboundedSender<Vec<u8>>,
        fast_mode: bool,
    ) -> Result<(u32, u16), Box<dyn std::error::Error + Send + Sync>> {
        let file = File::open(&self.path)?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        
        let mut hint = Hint::new();
        if let Some(ext) = self.path.extension() {
            hint.with_extension(ext.to_str().unwrap_or(""));
        }
        
        let meta_opts: MetadataOptions = Default::default();
        let fmt_opts: FormatOptions = Default::default();
        
        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &fmt_opts, &meta_opts)?;
        
        let mut format = probed.format;
        
        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or("No supported audio tracks found")?;
        
        let track_id = track.id;
        let codec_params = &track.codec_params;
        
        let sample_rate = codec_params.sample_rate.ok_or("Sample rate not found")?;
        let channels = codec_params.channels.ok_or("Channels not found")?.count() as u16;
        
        println!("File audio config - Sample rate: {}, Channels: {}", sample_rate, channels);
        
        let dec_opts: DecoderOptions = Default::default();
        let mut decoder = symphonia::default::get_codecs()
            .make(&codec_params, &dec_opts)?;
        
        let mut sample_buf = None;
        
        loop {
            let packet = match format.next_packet() {
                Ok(packet) => packet,
                Err(SymphoniaError::ResetRequired) => {
                    // The decoder needs to be reset
                    decoder.reset();
                    continue;
                }
                Err(SymphoniaError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    // End of file
                    break;
                }
                Err(err) => return Err(Box::new(err)),
            };
            
            if packet.track_id() != track_id {
                continue;
            }
            
            match decoder.decode(&packet) {
                Ok(decoded) => {
                    if sample_buf.is_none() {
                        let spec = *decoded.spec();
                        let duration = decoded.capacity() as u64;
                        sample_buf = Some(SampleBuffer::<i16>::new(duration, spec));
                    }
                    
                    if let Some(buf) = &mut sample_buf {
                        buf.copy_interleaved_ref(decoded);
                        
                        let samples = buf.samples();
                        let mut audio_data = Vec::with_capacity(samples.len() * 2);
                        
                        for &sample in samples {
                            audio_data.extend_from_slice(&sample.to_le_bytes());
                        }
                        
                        if tx.send(audio_data).is_err() {
                            break;
                        }
                        
                        // If not in fast mode, simulate real-time streaming
                        if !fast_mode {
                            let actual_samples = samples.len() / channels as usize;
                            let sleep_duration = Duration::from_secs_f64(
                                actual_samples as f64 / sample_rate as f64
                            );
                            tokio::time::sleep(sleep_duration).await;
                        }
                    }
                }
                Err(SymphoniaError::IoError(_)) => continue,
                Err(SymphoniaError::DecodeError(_)) => continue,
                Err(err) => return Err(Box::new(err)),
            }
        }
        
        Ok((sample_rate, channels))
    }
}

async fn run_deepgram_client(
    api_key: String,
    sample_rate: u32,
    channels: u16,
    mut audio_rx: mpsc::UnboundedReceiver<Vec<u8>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let url = format!(
        "wss://api.deepgram.com/v1/listen?encoding=linear16&sample_rate={}&channels={}&interim_results=true&punctuate=true&smart_format=true",
        sample_rate, channels
    );
    
    println!("Connecting to Deepgram WebSocket...");
    
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
    
    while let Some(audio_data) = audio_rx.recv().await {
        if let Err(e) = ws_sender.send(Message::Binary(audio_data.into())).await {
            eprintln!("Failed to send audio to WebSocket: {}", e);
            break;
        }
    }
    
    let _ = ws_sender.close().await;
    let _ = response_handler.await;
    
    Ok(())
}

async fn run_microphone_mode(api_key: String) -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting Deepgram real-time transcription from microphone...");
    
    let audio_capture = AudioCapture::new()?;
    let sample_rate = audio_capture.config.sample_rate.0;
    let channels = audio_capture.config.channels;
    
    println!("Audio config - Sample rate: {}, Channels: {}", sample_rate, channels);
    
    let (audio_tx, audio_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    
    let _stream = audio_capture.start_capture(audio_tx)?;
    
    println!("Listening for audio... Press Ctrl+C to stop.");
    
    let deepgram_task = tokio::spawn(run_deepgram_client(api_key, sample_rate, channels, audio_rx));
    
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

async fn run_file_mode(api_key: String, file_path: PathBuf, fast: bool) -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting Deepgram transcription from file...");
    println!("File: {}", file_path.display());
    println!("Mode: {}", if fast { "Fast" } else { "Real-time" });
    
    let (audio_tx, audio_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    
    let file_reader = AudioFileReader::new(file_path);
    
    let stream_task = tokio::spawn(async move {
        file_reader.stream_file(audio_tx, fast).await
    });
    
    match stream_task.await {
        Ok(Ok((sr, ch))) => {
            let deepgram_task = tokio::spawn(run_deepgram_client(api_key, sr, ch, audio_rx));
            
            match deepgram_task.await {
                Ok(Ok(())) => println!("\nTranscription completed successfully"),
                Ok(Err(e)) => eprintln!("Deepgram client error: {}", e),
                Err(e) => eprintln!("Task join error: {}", e),
            }
        }
        Ok(Err(e)) => return Err(e),
        Err(e) => return Err(format!("Task join error: {}", e).into()),
    }
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    
    let api_key = env::var("DEEPGRAM_API_KEY")
        .map_err(|_| "DEEPGRAM_API_KEY environment variable not set")?;
    
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Microphone => run_microphone_mode(api_key).await?,
        Commands::File { file, fast } => run_file_mode(api_key, file, fast).await?,
    }
    
    Ok(())
}
