use std::collections::HashMap;
use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use clap::{Parser, Subcommand};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Stream, StreamConfig};
use crossterm::{cursor, terminal, ExecutableCommand};
use futures_util::{SinkExt, StreamExt};
use log::{error, info};
use serde::Deserialize;
use tabled::{Table, Tabled};
use tokio::sync::broadcast;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use url::Url;

// Thread-safe writer for logging to file
struct ThreadSafeWriter(Arc<Mutex<std::fs::File>>);

impl Write for ThreadSafeWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.lock().unwrap().flush()
    }
}

#[derive(Debug, Deserialize)]
struct DeepgramResponse {
    #[serde(rename = "type")]
    message_type: String,
    #[serde(flatten)]
    data: serde_json::Value,
}

#[derive(Debug, Clone, Default, Tabled)]
struct ThreadStats {
    #[tabled(rename = "Thread")]
    thread_id: usize,
    #[tabled(rename = "Bytes Sent")]
    bytes_sent: u64,
    #[tabled(rename = "Bytes Recv")]
    bytes_received: u64,
    #[tabled(rename = "Results")]
    results_count: u64,
    #[tabled(rename = "SpeechStarted")]
    speech_started_count: u64,
    #[tabled(rename = "UtteranceEnd")]
    utterance_end_count: u64,
    #[tabled(rename = "Metadata")]
    metadata_count: u64,
    #[tabled(rename = "Other")]
    other_count: u64,
}

type StatsMap = Arc<Mutex<HashMap<usize, ThreadStats>>>;

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

        /// Number of concurrent threads/connections (default: 1)
        #[arg(long, default_value = "1")]
        threads: usize,

        /// Print all response messages instead of statistics table
        #[arg(long, short = 'v')]
        verbose: bool,
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

        /// Number of concurrent threads/connections (default: 1)
        #[arg(long, default_value = "1")]
        threads: usize,

        /// Print all response messages instead of statistics table
        #[arg(long, short = 'v')]
        verbose: bool,
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
        tx: broadcast::Sender<Vec<u8>>,
    ) -> Result<Stream, Box<dyn std::error::Error>> {
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

                // broadcast::Sender::send returns Result<usize, SendError>
                // The usize is the number of receivers that received the message
                if let Err(e) = tx.send(bytes) {
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

    // Remove trailing slashes from base_url to avoid double slashes
    let base_url = base_url.trim_end_matches('/');

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

    info!("Connecting to Deepgram WebSocket at {}...", url.as_str());
    let (ws_stream, response) = connect_async(request).await?;
    info!("Connected to Deepgram WebSocket successfully");

    Ok((ws_stream, response))
}

async fn handle_websocket_responses(
    thread_id: usize,
    mut ws_receiver: futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
    stats: StatsMap,
    verbose: bool,
) {
    let inactivity_timeout = tokio::time::Duration::from_millis(3000);

    loop {
        // Wait for next message with timeout
        let message_result = tokio::time::timeout(inactivity_timeout, ws_receiver.next()).await;

        let message = match message_result {
            Ok(Some(msg)) => msg,
            Ok(None) => {
                // Stream ended naturally
                info!("[Thread {}] WebSocket stream ended", thread_id);
                break;
            }
            Err(_) => {
                // Timeout - no message received in 3000ms
                info!("[Thread {}] No message received for 3000ms, assuming stream is finished", thread_id);
                break;
            }
        };

        match message {
            Ok(Message::Text(text)) => {
                // Update bytes received
                if let Ok(mut stats_map) = stats.lock() {
                    if let Some(thread_stats) = stats_map.get_mut(&thread_id) {
                        thread_stats.bytes_received += text.len() as u64;
                    }
                }

                match serde_json::from_str::<DeepgramResponse>(&text) {
                    Ok(response) => {
                        // Update message type counts
                        if let Ok(mut stats_map) = stats.lock() {
                            if let Some(thread_stats) = stats_map.get_mut(&thread_id) {
                                match response.message_type.as_str() {
                                    "Results" => thread_stats.results_count += 1,
                                    "SpeechStarted" => thread_stats.speech_started_count += 1,
                                    "UtteranceEnd" => thread_stats.utterance_end_count += 1,
                                    "Metadata" => thread_stats.metadata_count += 1,
                                    _ => thread_stats.other_count += 1,
                                }
                            }
                        }

                        if verbose {
                            println!("[Thread {}] 📨 Message Type: {}", thread_id, response.message_type);
                            println!(
                                "[Thread {}] 📄 Response Data: {}",
                                thread_id,
                                serde_json::to_string_pretty(&response.data).unwrap_or_default()
                            );
                            println!("---");
                        }
                    }
                    Err(e) => {
                        error!("[Thread {}] Failed to parse response: {}", thread_id, e);
                        if verbose {
                            println!("[Thread {}] 📨 Raw response: {}", thread_id, text);
                            println!("---");
                        }
                    }
                }
            }
            Ok(Message::Binary(data)) => {
                // Update bytes received
                if let Ok(mut stats_map) = stats.lock() {
                    if let Some(thread_stats) = stats_map.get_mut(&thread_id) {
                        thread_stats.bytes_received += data.len() as u64;
                    }
                }

                if verbose {
                    println!("[Thread {}] 📨 Message Type: Binary", thread_id);
                    println!("[Thread {}] 📄 Binary data received: {} bytes", thread_id, data.len());
                    println!("---");
                }
            }
            Ok(Message::Close(frame)) => {
                if verbose {
                    println!("[Thread {}] 📨 Message Type: Close", thread_id);
                    if let Some(frame) = &frame {
                        println!("[Thread {}] 📄 Close frame: code={}, reason={}", thread_id, frame.code, frame.reason);
                    }
                    println!("---");
                }
                break;
            }
            Ok(Message::Ping(data)) => {
                if verbose {
                    println!("[Thread {}] 📨 Message Type: Ping ({} bytes)", thread_id, data.len());
                }
            }
            Ok(Message::Pong(data)) => {
                if verbose {
                    println!("[Thread {}] 📨 Message Type: Pong ({} bytes)", thread_id, data.len());
                }
            }
            Ok(Message::Frame(_)) => {
                if verbose {
                    println!("[Thread {}] 📨 Message Type: Frame", thread_id);
                }
            }
            Err(e) => {
                error!("[Thread {}] WebSocket error: {}", thread_id, e);
                break;
            }
        }
    }
}

fn run_thread_worker(
    thread_id: usize,
    mut audio_rx: broadcast::Receiver<Vec<u8>>,
    api_key: String,
    endpoint: Option<String>,
    sample_rate: u32,
    encoding: String,
    stats: StatsMap,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Create thread-local Tokio runtime
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    runtime.block_on(async move {
        // Connect to Deepgram WebSocket
        info!("[Thread {}] Connecting to Deepgram WebSocket...", thread_id);

        let (ws_stream, _response) = match connect_to_deepgram(&api_key, endpoint.as_deref(), sample_rate, &encoding).await {
            Ok(result) => {
                info!("[Thread {}] Connected successfully", thread_id);
                result
            }
            Err(e) => {
                error!("[Thread {}] Failed to connect: {}", thread_id, e);
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to connect: {}", e)
                )) as Box<dyn std::error::Error + Send + Sync>);
            }
        };

        let (mut ws_sender, ws_receiver) = ws_stream.split();

        // Spawn response handler task
        let stats_clone = stats.clone();
        let response_handle = tokio::spawn(async move {
            handle_websocket_responses(thread_id, ws_receiver, stats_clone, verbose).await;
        });

        // Main loop: receive audio from broadcast and send to WebSocket
        let mut packet_count = 0u64;

        loop {
            match audio_rx.recv().await {
                Ok(audio_data) => {
                    packet_count += 1;
                    let bytes_len = audio_data.len() as u64;

                    // Update bytes sent in stats
                    if let Ok(mut stats_map) = stats.lock() {
                        if let Some(thread_stats) = stats_map.get_mut(&thread_id) {
                            thread_stats.bytes_sent += bytes_len;
                        }
                    }

                    // Print audio data info every 50 packets to avoid spam in verbose mode
                    if verbose && packet_count % 50 == 0 {
                        let total_bytes_sent = if let Ok(stats_map) = stats.lock() {
                            stats_map.get(&thread_id).map(|s| s.bytes_sent).unwrap_or(0)
                        } else {
                            0
                        };
                        println!(
                            "[Thread {}] 📤 Sent packet #{}: {} bytes (Total: {} bytes)",
                            thread_id,
                            packet_count,
                            bytes_len,
                            total_bytes_sent
                        );
                    }

                    if let Err(e) = ws_sender.send(Message::Binary(audio_data.into())).await {
                        error!("[Thread {}] Failed to send audio data: {}", thread_id, e);
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    log::warn!("[Thread {}] Lagged by {} messages, audio skipped", thread_id, n);
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => {
                    info!("[Thread {}] Channel closed, exiting", thread_id);
                    break;
                }
            }
        }

        // Send CloseStream message before exiting
        let close_message = r#"{"type":"CloseStream"}"#;
        if let Err(e) = ws_sender.send(Message::Text(close_message.to_string().into())).await {
            error!("[Thread {}] Failed to send CloseStream message: {}", thread_id, e);
        } else {
            info!("[Thread {}] Sent CloseStream message", thread_id);
        }

        // Wait for response handler to finish
        let _ = response_handle.await;

        info!("[Thread {}] Worker thread exiting", thread_id);
        Ok(())
    })
}

async fn run_microphone(
    endpoint: Option<String>,
    sample_rate: u32,
    encoding: String,
    threads: usize,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Get API key from environment variable
    let api_key =
        env::var("DEEPGRAM_API_KEY").map_err(|_| "DEEPGRAM_API_KEY environment variable not set")?;

    info!("Starting microphone streaming to Deepgram Flux with {} thread(s)...", threads);

    // Initialize audio capture
    let audio_capture = AudioCapture::new()?;

    // Create broadcast channel for audio data (1000 message buffer)
    let (audio_tx, _) = broadcast::channel::<Vec<u8>>(1000);

    // Start audio capture
    let stream = audio_capture.start_capture(audio_tx.clone())?;
    info!("Audio capture started");

    // Initialize stats map
    let stats: StatsMap = Arc::new(Mutex::new(HashMap::new()));
    for thread_id in 0..threads {
        stats.lock().unwrap().insert(
            thread_id,
            ThreadStats {
                thread_id,
                ..Default::default()
            },
        );
    }

    println!("🎤 Listening to microphone and streaming to Deepgram Flux API...");
    println!("Spawning {} worker thread(s)...", threads);
    println!("📝 Writing logs to: flux-turn-taking.log");
    println!("Press Ctrl+C to stop");
    if !verbose {
        println!("Use --verbose to see all messages");
    }
    println!("===");

    // Spawn worker threads
    let mut thread_handles = Vec::new();

    for thread_id in 0..threads {
        let audio_rx = audio_tx.subscribe();
        let api_key_clone = api_key.clone();
        let endpoint_clone = endpoint.clone();
        let encoding_clone = encoding.clone();
        let stats_clone = stats.clone();

        let handle = std::thread::spawn(move || {
            run_thread_worker(
                thread_id,
                audio_rx,
                api_key_clone,
                endpoint_clone,
                sample_rate,
                encoding_clone,
                stats_clone,
                verbose,
            )
        });

        thread_handles.push(handle);
    }

    // Spawn stats display task if not in verbose mode
    let display_task = if !verbose {
        let stats_clone = stats.clone();
        Some(tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                display_stats_table(&stats_clone);
            }
        }))
    } else {
        None
    };

    // Wait for Ctrl+C
    tokio::signal::ctrl_c().await?;
    info!("Received Ctrl+C, shutting down...");

    // Stop audio capture first
    drop(stream);

    // Drop the audio_tx to signal all threads to exit
    drop(audio_tx);

    // Cancel display task if it exists
    if let Some(task) = display_task {
        task.abort();
    }

    // Wait for all threads to finish with 2 second timeout
    println!("\nWaiting for worker threads to finish (2 second timeout)...");

    let shutdown_timeout = tokio::time::Duration::from_secs(2);
    let thread_count = thread_handles.len();

    // Spawn tasks to wait for each thread
    let mut join_tasks = Vec::new();
    for (thread_id, handle) in thread_handles.into_iter().enumerate() {
        let task = tokio::task::spawn_blocking(move || {
            (thread_id, handle.join())
        });
        join_tasks.push(task);
    }

    // Wait for all threads with timeout
    match tokio::time::timeout(shutdown_timeout, async {
        for task in join_tasks {
            if let Ok((thread_id, join_result)) = task.await {
                match join_result {
                    Ok(Ok(())) => {
                        info!("Thread {} exited successfully", thread_id);
                    }
                    Ok(Err(e)) => {
                        error!("Thread {} exited with error: {}", thread_id, e);
                    }
                    Err(e) => {
                        error!("Thread {} panicked: {:?}", thread_id, e);
                    }
                }
            }
        }
    }).await {
        Ok(_) => {
            info!("All {} threads exited successfully", thread_count);
        }
        Err(_) => {
            error!("Shutdown timeout exceeded after 2 seconds, forcing exit");
            println!("🛑 Application stopped (forced)");
            std::process::exit(0);
        }
    }

    println!("🛑 Application stopped");

    // Force exit to ensure immediate termination
    std::process::exit(0);
}

async fn run_file(
    file_path: PathBuf,
    endpoint: Option<String>,
    sample_rate: u32,
    encoding: String,
    threads: usize,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Get API key from environment variable
    let api_key =
        env::var("DEEPGRAM_API_KEY").map_err(|_| "DEEPGRAM_API_KEY environment variable not set")?;

    info!("Starting file streaming to Deepgram Flux with {} thread(s)...", threads);

    // Verify file exists
    if !file_path.exists() {
        return Err(format!("File not found: {}", file_path.display()).into());
    }

    info!("Reading audio file: {}", file_path.display());

    // Read the entire file into memory
    let audio_data = tokio::fs::read(&file_path).await?;
    info!("File loaded: {} bytes", audio_data.len());

    // Create broadcast channel for audio data (1000 message buffer)
    let (audio_tx, _) = broadcast::channel::<Vec<u8>>(1000);

    // Initialize stats map
    let stats: StatsMap = Arc::new(Mutex::new(HashMap::new()));
    for thread_id in 0..threads {
        stats.lock().unwrap().insert(
            thread_id,
            ThreadStats {
                thread_id,
                ..Default::default()
            },
        );
    }

    println!("📁 Streaming file to Deepgram Flux API...");
    println!("File: {}", file_path.display());
    println!("Spawning {} worker thread(s)...", threads);
    println!("📝 Writing logs to: flux-turn-taking.log");
    println!("Press Ctrl+C to stop");
    if !verbose {
        println!("Use --verbose to see all messages");
    }
    println!("===");

    // Spawn worker threads
    let mut thread_handles = Vec::new();

    for thread_id in 0..threads {
        let audio_rx = audio_tx.subscribe();
        let api_key_clone = api_key.clone();
        let endpoint_clone = endpoint.clone();
        let encoding_clone = encoding.clone();
        let stats_clone = stats.clone();

        let handle = std::thread::spawn(move || {
            run_thread_worker(
                thread_id,
                audio_rx,
                api_key_clone,
                endpoint_clone,
                sample_rate,
                encoding_clone,
                stats_clone,
                verbose,
            )
        });

        thread_handles.push(handle);
    }

    // Spawn stats display task if not in verbose mode
    let display_task = if !verbose {
        let stats_clone = stats.clone();
        Some(tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                display_stats_table(&stats_clone);
            }
        }))
    } else {
        None
    };

    // Stream the audio data in chunks to the broadcast channel
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
                "📤 Broadcasting chunk #{}: {} bytes (Progress: {:.1}%, Total: {} bytes)",
                chunk_count,
                chunk.len(),
                (total_bytes_sent as f64 / audio_data.len() as f64) * 100.0,
                total_bytes_sent
            );
        }

        // Send to broadcast channel (all threads will receive it)
        match audio_tx.send(chunk.to_vec()) {
            Ok(receiver_count) => {
                info!("Successfully broadcast chunk #{} to {} receiver(s)", chunk_count, receiver_count);
            }
            Err(e) => {
                error!("Failed to broadcast audio data chunk #{}: {}", chunk_count, e);
                break;
            }
        }

        offset = end;

        // Small delay to simulate streaming (adjust as needed)
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    }

    println!("✅ File streaming complete: {} chunks sent ({} total bytes)", chunk_count, total_bytes_sent);

    // Drop the audio_tx to signal all threads that streaming is complete
    drop(audio_tx);

    // Cancel display task if it exists
    if let Some(task) = display_task {
        task.abort();
    }

    // Wait for all threads to finish with 2 second timeout
    println!("\nWaiting for worker threads to finish (2 second timeout)...");

    let shutdown_timeout = tokio::time::Duration::from_secs(2);
    let thread_count = thread_handles.len();

    // Spawn tasks to wait for each thread
    let mut join_tasks = Vec::new();
    for (thread_id, handle) in thread_handles.into_iter().enumerate() {
        let task = tokio::task::spawn_blocking(move || {
            (thread_id, handle.join())
        });
        join_tasks.push(task);
    }

    // Wait for all threads with timeout
    match tokio::time::timeout(shutdown_timeout, async {
        for task in join_tasks {
            if let Ok((thread_id, join_result)) = task.await {
                match join_result {
                    Ok(Ok(())) => {
                        info!("Thread {} exited successfully", thread_id);
                    }
                    Ok(Err(e)) => {
                        error!("Thread {} exited with error: {}", thread_id, e);
                    }
                    Err(e) => {
                        error!("Thread {} panicked: {:?}", thread_id, e);
                    }
                }
            }
        }
    }).await {
        Ok(_) => {
            info!("All {} threads exited successfully", thread_count);
        }
        Err(_) => {
            error!("Shutdown timeout exceeded after 2 seconds, forcing exit");
            println!("🛑 Application stopped (forced)");
            std::process::exit(0);
        }
    }

    println!("🛑 Application stopped");

    // Force exit to ensure immediate termination
    std::process::exit(0);
}

fn display_stats_table(stats: &StatsMap) {
    if let Ok(stats_map) = stats.lock() {
        let mut thread_stats: Vec<ThreadStats> = stats_map.values().cloned().collect();
        thread_stats.sort_by_key(|s| s.thread_id);

        if !thread_stats.is_empty() {
            use tabled::settings::Style;

            let table = Table::new(&thread_stats)
                .with(Style::sharp())
                .to_string();

            // Clear screen and move cursor to top
            let mut stdout = std::io::stdout();
            let _ = stdout.execute(cursor::MoveTo(0, 0));
            let _ = stdout.execute(terminal::Clear(terminal::ClearType::FromCursorDown));

            println!("{}", table);
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure logging to write to file
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("flux-turn-taking.log")?;

    let writer = ThreadSafeWriter(Arc::new(Mutex::new(log_file)));

    env_logger::Builder::from_default_env()
        .target(env_logger::Target::Pipe(Box::new(writer)))
        .init();

    info!("=== Flux Turn-Taking Load Test Started ===");

    let cli = Cli::parse();

    match cli.command {
        Commands::Microphone { endpoint, sample_rate, encoding, threads, verbose } => {
            run_microphone(endpoint, sample_rate, encoding, threads, verbose).await?;
        }
        Commands::File { file, endpoint, sample_rate, encoding, threads, verbose } => {
            run_file(file, endpoint, sample_rate, encoding, threads, verbose).await?;
        }
    }

    Ok(())
}
