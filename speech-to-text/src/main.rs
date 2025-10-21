use tokio::sync::{mpsc, oneshot};
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
    Microphone {
        /// Callback URL for receiving transcription results
        #[arg(long)]
        callback: Option<String>,
        
        /// Suppress console output of transcripts
        #[arg(long)]
        silent: bool,
        
        /// Override the Deepgram API base URL
        #[arg(long)]
        endpoint: Option<String>,
        
        /// Audio encoding format (e.g., linear16, mulaw, flac)
        #[arg(long)]
        encoding: Option<String>,
        
        /// Audio sample rate in Hz∫∫
        #[arg(long)]
        sample_rate: Option<u32>,
        
        /// Number of audio channels
        #[arg(long)]
        channels: Option<u16>,
        
        /// Enable interim results
        #[arg(long)]
        interim_results: Option<bool>,
        
        /// Enable punctuation
        #[arg(long)]
        punctuate: Option<bool>,
        
        /// Enable smart formatting
        #[arg(long)]
        smart_format: Option<bool>,
        
        /// Deepgram model to use (e.g., nova-2, enhanced, base)
        #[arg(long)]
        model: Option<String>,
        
        /// Redact entities (comma-separated). Can include specific entities or categories: phi, pii, pci, other
        #[arg(long)]
        redact: Option<String>,
    },
    /// Stream audio from a file for transcription
    File {
        /// Path to the audio file (supports MP3, WAV, FLAC)
        #[arg(short, long)]
        file: PathBuf,
        
        /// Stream audio as fast as possible instead of real-time rate
        #[arg(long)]
        fast: bool,
        
        /// Callback URL for receiving transcription results
        #[arg(long)]
        callback: Option<String>,
        
        /// Suppress console output of transcripts
        #[arg(long)]
        silent: bool,
        
        /// Override the Deepgram API base URL
        #[arg(long)]
        endpoint: Option<String>,
        
        /// Audio encoding format (e.g., linear16, mulaw, flac)
        #[arg(long)]
        encoding: Option<String>,
        
        /// Audio sample rate in Hz
        #[arg(long)]
        sample_rate: Option<u32>,
        
        /// Number of audio channels
        #[arg(long)]
        channels: Option<u16>,
        
        /// Enable interim results
        #[arg(long)]
        interim_results: Option<bool>,
        
        /// Enable punctuation
        #[arg(long)]
        punctuate: Option<bool>,
        
        /// Enable smart formatting
        #[arg(long)]
        smart_format: Option<bool>,
        
        /// Deepgram model to use (e.g., nova-2, enhanced, base)
        #[arg(long)]
        model: Option<String>,
        
        /// Redact entities (comma-separated). Can include specific entities or categories: phi, pii, pci, other
        #[arg(long)]
        redact: Option<String>,
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
        config_tx: oneshot::Sender<(u32, u16)>,
        ready_rx: Option<oneshot::Receiver<()>>,
        fast_mode: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
        
        // Send the audio configuration immediately so Deepgram client can start
        let _ = config_tx.send((sample_rate, channels));
        
        // In fast mode, wait for WebSocket to be ready before streaming
        if let Some(rx) = ready_rx {
            let _ = rx.await;
            println!("WebSocket ready, starting fast audio stream...");
        }
        
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
        
        Ok(())
    }
}

async fn run_deepgram_client(
    api_key: String,
    detected_sample_rate: u32,
    detected_channels: u16,
    mut audio_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    ready_tx: Option<oneshot::Sender<()>>,
    callback: Option<String>,
    silent: bool,
    endpoint: Option<String>,
    encoding: Option<String>,
    sample_rate_override: Option<u32>,
    channels_override: Option<u16>,
    interim_results: Option<bool>,
    punctuate: Option<bool>,
    smart_format: Option<bool>,
    model: Option<String>,
    redact: Option<String>,
    mut shutdown_rx: mpsc::Receiver<()>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Use custom endpoint or default to Deepgram API
    let base_url = endpoint.unwrap_or_else(|| "wss://api.deepgram.com".to_string());
    
    // Start building the URL
    let mut url = format!("{}/v1/listen?", base_url);
    let mut params = Vec::new();
    
    // Add encoding parameter (default to linear16 if not specified)
    let encoding_value = encoding.unwrap_or_else(|| "linear16".to_string());
    params.push(format!("encoding={}", encoding_value));
    
    // Add sample_rate parameter (use override if provided, otherwise use detected)
    let sample_rate_value = sample_rate_override.unwrap_or(detected_sample_rate);
    params.push(format!("sample_rate={}", sample_rate_value));
    
    // Add channels parameter (use override if provided, otherwise use detected)
    let channels_value = channels_override.unwrap_or(detected_channels);
    params.push(format!("channels={}", channels_value));
    
    // Add interim_results parameter if specified
    if let Some(interim) = interim_results {
        params.push(format!("interim_results={}", interim));
    }
    
    // Add punctuate parameter if specified
    if let Some(punct) = punctuate {
        params.push(format!("punctuate={}", punct));
    }
    
    // Add smart_format parameter if specified
    if let Some(smart) = smart_format {
        params.push(format!("smart_format={}", smart));
    }
    
    // Add model parameter if specified
    if let Some(model_name) = model {
        params.push(format!("model={}", model_name));
    }
    
    // Add redact parameter if specified
    if let Some(redact_value) = redact {
        // Parse the redact value to handle categories and individual entities
        let redact_entities = parse_redact_entities(&redact_value);
        if !redact_entities.is_empty() {
            params.push(format!("redact={}", redact_entities.join("&redact=")));
        }
    }
    
    // Join all parameters
    url.push_str(&params.join("&"));
    
    // Add callback parameters if provided
    if let Some(callback_url) = &callback {
        url.push_str(&format!("&callback={}&callback_method=post", urlencoding::encode(callback_url)));
    }
    
    println!("Connecting to Deepgram WebSocket...");
    
    let url_parsed = url::Url::parse(&url)?;
    let host = url_parsed.host_str().ok_or("Invalid host in URL")?;

    println!("Connecting to Deepgram URL: {0}", &url);
    
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
    
    // Signal that we're ready to receive audio
    if let Some(tx) = ready_tx {
        let _ = tx.send(());
    }
    
    let (ws_sender, mut ws_receiver) = ws_stream.split();
    
    let (result_tx, mut result_rx) = mpsc::unbounded_channel::<()>();
    let (msg_tx, mut msg_rx) = mpsc::unbounded_channel::<Message>();
    
    // Spawn a task to handle sending messages to WebSocket
    let sender_task = tokio::spawn(async move {
        let mut ws_sender = ws_sender;
        while let Some(msg) = msg_rx.recv().await {
            if ws_sender.send(msg).await.is_err() {
                break;
            }
        }
    });
    
    // Spawn a keep-alive task that sends a message every 5 seconds
    let keepalive_tx = msg_tx.clone();
    let keepalive_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        interval.tick().await; // Skip the first immediate tick
        
        loop {
            interval.tick().await;
            // Send keep-alive message
            let keepalive_msg = serde_json::json!({"type": "KeepAlive"});
            if let Ok(msg_str) = serde_json::to_string(&keepalive_msg) {
                if keepalive_tx.send(Message::Text(msg_str.into())).is_err() {
                    break;
                }
            }
        }
    });
    
    let response_handler = tokio::spawn(async move {
        let mut last_message_time = tokio::time::Instant::now();
        let timeout_duration = Duration::from_secs(10);
        
        loop {
            tokio::select! {
                msg = ws_receiver.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            last_message_time = tokio::time::Instant::now();
                            match serde_json::from_str::<DeepgramResponse>(&text) {
                                Ok(response) => {
                                    if response.message_type == "Results" {
                                        if let Some(channel) = response.channel {
                                            for alternative in channel.alternatives {
                                                if !alternative.transcript.trim().is_empty() && !silent {
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
                        Some(Ok(Message::Close(_))) => {
                            if !silent {
                                println!("WebSocket connection closed by server");
                            }
                            break;
                        }
                        Some(Err(e)) => {
                            eprintln!("WebSocket error: {}", e);
                            break;
                        }
                        None => break,
                        _ => {}
                    }
                }
                _ = tokio::time::sleep_until(last_message_time + timeout_duration) => {
                    // No messages received for timeout duration, we're done
                    if !silent {
                        println!("No more messages received, finishing...");
                    }
                    break;
                }
            }
        }
        let _ = result_tx.send(());
    });
    
    let mut audio_count = 0;
    loop {
        tokio::select! {
            Some(audio_data) = audio_rx.recv() => {
                audio_count += 1;
                if msg_tx.send(Message::Binary(audio_data.into())).is_err() {
                    eprintln!("Failed to send audio to WebSocket");
                    break;
                }
            }
            _ = shutdown_rx.recv() => {
                println!("\nReceived shutdown signal, sending CloseStream message...");
                // Send CloseStream message
                let close_stream_msg = serde_json::json!({"type": "CloseStream"});
                if let Ok(msg_str) = serde_json::to_string(&close_stream_msg) {
                    let _ = msg_tx.send(Message::Text(msg_str.into()));
                }
                break;
            }
            else => break,
        }
    }
    
    println!("Sent {} audio chunks, waiting for transcription results...", audio_count);
    
    // Wait for the response handler to signal it's done (no more messages)
    let _ = result_rx.recv().await;
    
    // Stop sending messages
    drop(msg_tx);
    
    // Wait for all tasks to finish
    let _ = keepalive_task.await;
    let _ = sender_task.await;
    let _ = response_handler.await;
    
    Ok(())
}

fn parse_redact_entities(redact_value: &str) -> Vec<String> {
    let mut entities = Vec::new();
    
    // Split by comma and trim whitespace
    for item in redact_value.split(',') {
        let item = item.trim();
        
        if !item.is_empty() {
            // Keep categories and individual entities as-is
            // The API will handle category expansion on the server side
            entities.push(item.to_lowercase());
        }
    }
    
    // Remove duplicates while preserving order
    let mut seen = std::collections::HashSet::new();
    entities.retain(|e| seen.insert(e.clone()));
    
    entities
}

async fn run_microphone_mode(
    api_key: String,
    callback: Option<String>,
    silent: bool,
    endpoint: Option<String>,
    encoding: Option<String>,
    sample_rate_override: Option<u32>,
    channels_override: Option<u16>,
    interim_results: Option<bool>,
    punctuate: Option<bool>,
    smart_format: Option<bool>,
    model: Option<String>,
    redact: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting Deepgram real-time transcription from microphone...");
    
    let audio_capture = AudioCapture::new()?;
    let sample_rate = audio_capture.config.sample_rate.0;
    let channels = audio_capture.config.channels;
    
    println!("Audio config - Sample rate: {}, Channels: {}", sample_rate, channels);
    
    let (audio_tx, audio_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>(1);
    
    let _stream = audio_capture.start_capture(audio_tx)?;
    
    println!("Listening for audio... Press Ctrl+C to stop.");
    
    let mut deepgram_task = tokio::spawn(run_deepgram_client(
        api_key,
        sample_rate,
        channels,
        audio_rx,
        None,
        callback,
        silent,
        endpoint,
        encoding,
        sample_rate_override,
        channels_override,
        interim_results,
        punctuate,
        smart_format,
        model,
        redact,
        shutdown_rx,
    ));
    
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            println!("\nReceived Ctrl+C, initiating graceful shutdown...");
            let _ = shutdown_tx.send(()).await;
            
            // Wait for the Deepgram task to finish after sending shutdown signal
            match deepgram_task.await {
                Ok(Ok(())) => println!("Deepgram client finished successfully"),
                Ok(Err(e)) => eprintln!("Deepgram client error: {}", e),
                Err(e) => eprintln!("Task join error: {}", e),
            }
        }
        result = &mut deepgram_task => {
            match result {
                Ok(Ok(())) => println!("Deepgram client finished successfully"),
                Ok(Err(e)) => eprintln!("Deepgram client error: {}", e),
                Err(e) => eprintln!("Task join error: {}", e),
            }
        }
    }
    
    Ok(())
}

async fn run_file_mode(
    api_key: String,
    file_path: PathBuf,
    fast: bool,
    callback: Option<String>,
    silent: bool,
    endpoint: Option<String>,
    encoding: Option<String>,
    sample_rate_override: Option<u32>,
    channels_override: Option<u16>,
    interim_results: Option<bool>,
    punctuate: Option<bool>,
    smart_format: Option<bool>,
    model: Option<String>,
    redact: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting Deepgram transcription from file...");
    println!("File: {}", file_path.display());
    println!("Mode: {}", if fast { "Fast" } else { "Real-time" });
    
    let (audio_tx, audio_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let (config_tx, config_rx) = oneshot::channel::<(u32, u16)>();
    let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>(1);
    
    let file_reader = AudioFileReader::new(file_path);
    
    // Create a ready signal channel for fast mode
    let (ready_tx, ready_rx) = if fast {
        let (tx, rx) = oneshot::channel();
        (Some(tx), Some(rx))
    } else {
        (None, None)
    };
    
    // Start streaming file audio in the background
    let stream_task = tokio::spawn(async move {
        file_reader.stream_file(audio_tx, config_tx, ready_rx, fast).await
    });
    
    // Wait for the audio configuration to be sent
    let (sample_rate, channels) = config_rx.await
        .map_err(|_| "Failed to receive audio configuration")?;
    
    println!("Sample rate: {}, Channels: {}", sample_rate, channels);
    
    // Start the Deepgram client (now both tasks run concurrently)
    let deepgram_task = tokio::spawn(run_deepgram_client(
        api_key,
        sample_rate,
        channels,
        audio_rx,
        ready_tx,
        callback,
        silent,
        endpoint,
        encoding,
        sample_rate_override,
        channels_override,
        interim_results,
        punctuate,
        smart_format,
        model,
        redact,
        shutdown_rx,
    ));
    
    // Wait for either CTRL+C or both tasks to complete
    let ctrl_c_future = tokio::signal::ctrl_c();
    let tasks_future = async {
        tokio::join!(stream_task, deepgram_task)
    };
    
    tokio::pin!(ctrl_c_future);
    tokio::pin!(tasks_future);
    
    tokio::select! {
        _ = &mut ctrl_c_future => {
            println!("\nReceived Ctrl+C, initiating graceful shutdown...");
            let _ = shutdown_tx.send(()).await;
            
            // Wait for tasks to finish after shutdown signal
            let (stream_result, deepgram_result) = tasks_future.await;
            
            match stream_result {
                Ok(Ok(())) => {},
                Ok(Err(e)) => eprintln!("File streaming error: {}", e),
                Err(e) => eprintln!("Stream task join error: {}", e),
            }
            
            match deepgram_result {
                Ok(Ok(())) => println!("Deepgram client finished successfully"),
                Ok(Err(e)) => eprintln!("Deepgram client error: {}", e),
                Err(e) => eprintln!("Deepgram task join error: {}", e),
            }
        }
        result = &mut tasks_future => {
            let (stream_result, deepgram_result) = result;
            
            match stream_result {
                Ok(Ok(())) => {},
                Ok(Err(e)) => eprintln!("File streaming error: {}", e),
                Err(e) => eprintln!("Stream task join error: {}", e),
            }
            
            match deepgram_result {
                Ok(Ok(())) => println!("\nTranscription completed successfully"),
                Ok(Err(e)) => eprintln!("Deepgram client error: {}", e),
                Err(e) => eprintln!("Deepgram task join error: {}", e),
            }
        }
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
        Commands::Microphone {
            callback,
            silent,
            endpoint,
            encoding,
            sample_rate,
            channels,
            interim_results,
            punctuate,
            smart_format,
            model,
            redact,
        } => {
            run_microphone_mode(
                api_key,
                callback,
                silent,
                endpoint,
                encoding,
                sample_rate,
                channels,
                interim_results,
                punctuate,
                smart_format,
                model,
                redact,
            )
            .await?
        }
        Commands::File {
            file,
            fast,
            callback,
            silent,
            endpoint,
            encoding,
            sample_rate,
            channels,
            interim_results,
            punctuate,
            smart_format,
            model,
            redact,
        } => {
            run_file_mode(
                api_key,
                file,
                fast,
                callback,
                silent,
                endpoint,
                encoding,
                sample_rate,
                channels,
                interim_results,
                punctuate,
                smart_format,
                model,
                redact,
            )
            .await?
        }
    }
    
    Ok(())
}
