mod stream;
mod transcribe;

use clap::{Parser, Subcommand};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, Stream, StreamConfig};
use dotenv::dotenv;
use futures_util::{SinkExt, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use serde::Deserialize;
use std::env;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CODEC_TYPE_NULL, CodecType, DecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

use crate::stream::StreamSource;
use crate::transcribe::TranscribeArgs;

fn hosted_deepgram_endpoint(endpoint: Option<&str>) -> bool {
    let Some(endpoint) = endpoint else {
        return true;
    };

    url::Url::parse(endpoint)
        .ok()
        .and_then(|url| url.host_str().map(|host| host.to_ascii_lowercase()))
        .map(|host| host == "api.deepgram.com" || host.ends_with(".api.deepgram.com"))
        .unwrap_or_else(|| endpoint.contains("api.deepgram.com"))
}

fn api_key_for_endpoint(
    endpoint: Option<&str>,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    match env::var("DEEPGRAM_API_KEY") {
        Ok(api_key) => Ok(Some(api_key)),
        Err(_) if hosted_deepgram_endpoint(endpoint) => {
            Err("DEEPGRAM_API_KEY environment variable not set".into())
        }
        Err(_) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::hosted_deepgram_endpoint;

    #[test]
    fn default_and_hosted_endpoints_require_deepgram_auth() {
        assert!(hosted_deepgram_endpoint(None));
        assert!(hosted_deepgram_endpoint(Some("https://api.deepgram.com")));
        assert!(hosted_deepgram_endpoint(Some("wss://api.deepgram.com")));
    }

    #[test]
    fn custom_endpoints_can_run_without_deepgram_auth() {
        assert!(!hosted_deepgram_endpoint(Some("http://localhost:8080")));
        assert!(!hosted_deepgram_endpoint(Some("ws://127.0.0.1:8119")));
        assert!(!hosted_deepgram_endpoint(Some(
            "https://stt.internal.example.com"
        )));
    }
}

fn codec_name(codec: CodecType) -> &'static str {
    use symphonia::core::codecs::*;
    match codec {
        CODEC_TYPE_MP3 => "MP3",
        CODEC_TYPE_MP2 => "MP2",
        CODEC_TYPE_MP1 => "MP1",
        CODEC_TYPE_AAC => "AAC",
        CODEC_TYPE_FLAC => "FLAC",
        CODEC_TYPE_VORBIS => "Vorbis",
        CODEC_TYPE_OPUS => "Opus",
        CODEC_TYPE_ALAC => "ALAC",
        CODEC_TYPE_PCM_ALAW => "PCM A-law",
        CODEC_TYPE_PCM_MULAW => "PCM μ-law",
        CODEC_TYPE_PCM_S16LE | CODEC_TYPE_PCM_S16BE | CODEC_TYPE_PCM_S24LE
        | CODEC_TYPE_PCM_S24BE | CODEC_TYPE_PCM_S32LE | CODEC_TYPE_PCM_S32BE
        | CODEC_TYPE_PCM_F32LE | CODEC_TYPE_PCM_F32BE | CODEC_TYPE_PCM_F64LE
        | CODEC_TYPE_PCM_F64BE => "PCM",
        _ => "Unknown",
    }
}

#[derive(Parser)]
#[command(name = "dg-stt")]
#[command(about = "Deepgram Speech-to-Text CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Stream audio for real-time transcription
    Stream {
        #[command(subcommand)]
        source: StreamSource,
    },
    /// Transcribe pre-recorded audio file using HTTP API
    Transcribe {
        #[command(flatten)]
        args: TranscribeArgs,
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
    #[serde(default)]
    words: Vec<Word>,
}

#[derive(Debug, Deserialize)]
struct Word {
    word: String,
    speaker: Option<u32>,
}

type StreamError = Box<dyn std::error::Error + Send + Sync>;
type StreamResult = Result<(), StreamError>;

#[derive(Clone)]
struct DeepgramClientConfig {
    api_key: Option<String>,
    callback: Option<String>,
    silent: bool,
    endpoint: Option<String>,
    encoding: Option<String>,
    sample_rate_override: Option<u32>,
    channels_override: Option<u16>,
    multichannel: bool,
    diarize: bool,
    detect_entities: bool,
    interim_results: bool,
    vad_events: bool,
    punctuate: bool,
    smart_format: bool,
    sentiment: bool,
    intents: bool,
    topics: bool,
    model: Option<String>,
    redact: Option<String>,
    language: Option<String>,
    endpointing: Option<u32>,
    utterance_end: Option<u32>,
    keyterm: Option<String>,
    keywords: Option<String>,
}

fn connection_prefix(connection_id: usize, connection_count: usize) -> String {
    if connection_count > 1 {
        format!("[connection {connection_id}/{connection_count}] ")
    } else {
        String::new()
    }
}

fn start_audio_fanout(
    connection_count: usize,
) -> (
    mpsc::UnboundedSender<Vec<u8>>,
    Vec<mpsc::UnboundedReceiver<Vec<u8>>>,
    JoinHandle<()>,
) {
    let (source_tx, mut source_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let mut output_senders = Vec::with_capacity(connection_count);
    let mut output_receivers = Vec::with_capacity(connection_count);

    for _ in 0..connection_count {
        let (tx, rx) = mpsc::unbounded_channel::<Vec<u8>>();
        output_senders.push(tx);
        output_receivers.push(rx);
    }

    let fanout_task = tokio::spawn(async move {
        let mut output_senders = output_senders;
        while let Some(audio_data) = source_rx.recv().await {
            output_senders.retain(|tx| tx.send(audio_data.clone()).is_ok());
            if output_senders.is_empty() {
                break;
            }
        }
    });

    (source_tx, output_receivers, fanout_task)
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

        Ok(AudioCapture {
            device,
            config,
            sample_format,
        })
    }

    fn start_capture(
        &self,
        tx: mpsc::UnboundedSender<Vec<u8>>,
    ) -> Result<Stream, Box<dyn std::error::Error>> {
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

    fn build_stream<T>(
        &self,
        config: StreamConfig,
        tx: mpsc::UnboundedSender<Vec<u8>>,
    ) -> Result<Stream, Box<dyn std::error::Error>>
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

        let probed = symphonia::default::get_probe().format(&hint, mss, &fmt_opts, &meta_opts)?;

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
        let total_frames = codec_params.n_frames;

        // --- File metadata ---
        let codec_str = codec_name(codec_params.codec);

        let channel_str = match channels {
            1 => "mono".to_string(),
            2 => "stereo".to_string(),
            n => format!("{n}ch"),
        };

        let bit_depth_str = codec_params
            .bits_per_sample
            .map(|b| format!(", {b}-bit"))
            .unwrap_or_default();

        let duration_str = total_frames
            .map(|f| {
                let s = f / sample_rate as u64;
                format!("{}:{:02}", s / 60, s % 60)
            })
            .unwrap_or_else(|| "unknown".to_string());

        // Bitrate: for compressed formats derive from file size + duration;
        // for uncompressed (PCM) calculate directly from the stream parameters.
        let bitrate_str = {
            let from_file = total_frames.and_then(|f| {
                let dur = f as f64 / sample_rate as f64;
                if dur > 0.0 {
                    std::fs::metadata(&self.path).ok().map(|m| {
                        format!(
                            "{} kbps",
                            (m.len() as f64 * 8.0 / dur / 1000.0).round() as u32
                        )
                    })
                } else {
                    None
                }
            });
            let from_params = codec_params
                .bits_per_sample
                .map(|b| format!("{} kbps", sample_rate * channels as u32 * b / 1000));
            from_file
                .or(from_params)
                .unwrap_or_else(|| "unknown".to_string())
        };

        println!("File:    {}", self.path.display());
        println!("Format:  {codec_str}, {bitrate_str}");
        println!("Audio:   {sample_rate} Hz, {channel_str}{bit_depth_str}");
        println!("Length:  {duration_str}");

        // Send the audio configuration immediately so Deepgram client can start
        let _ = config_tx.send((sample_rate, channels));

        // Wait for WebSocket to be ready before streaming (ensures request ID
        // is printed before the progress bar appears).
        if let Some(rx) = ready_rx {
            let _ = rx.await;
            if fast_mode {
                println!("WebSocket ready, starting fast audio stream...");
            }
        }

        // Set up progress bar
        let pb = if let Some(total) = total_frames {
            let total_secs = total / sample_rate as u64;
            let pb = ProgressBar::new(total_secs);
            pb.set_style(
                ProgressStyle::with_template("{bar:40.cyan/blue} {msg}")
                    .unwrap()
                    .progress_chars("█▓░"),
            );
            pb.set_message(format!("0:00 / {}:{:02}", total_secs / 60, total_secs % 60));
            Some((pb, total_secs))
        } else {
            None
        };

        let dec_opts: DecoderOptions = Default::default();
        let mut decoder = symphonia::default::get_codecs().make(&codec_params, &dec_opts)?;

        let mut sample_buf = None;
        let mut frames_sent: u64 = 0;

        loop {
            let packet = match format.next_packet() {
                Ok(packet) => packet,
                Err(SymphoniaError::ResetRequired) => {
                    decoder.reset();
                    continue;
                }
                Err(SymphoniaError::IoError(e))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    break;
                }
                Err(err) => return Err(Box::new(err)),
            };

            if packet.track_id() != track_id {
                continue;
            }

            match decoder.decode(&packet) {
                Ok(decoded) => {
                    let frame_count = decoded.frames() as u64;

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

                        frames_sent += frame_count;

                        // Update progress bar
                        if let Some((ref pb, total_secs)) = pb {
                            let current_secs = frames_sent / sample_rate as u64;
                            pb.set_position(current_secs);
                            pb.set_message(format!(
                                "{}:{:02} / {}:{:02}",
                                current_secs / 60,
                                current_secs % 60,
                                total_secs / 60,
                                total_secs % 60,
                            ));
                        }

                        // If not in fast mode, simulate real-time streaming
                        if !fast_mode {
                            let sleep_duration =
                                Duration::from_secs_f64(frame_count as f64 / sample_rate as f64);
                            tokio::time::sleep(sleep_duration).await;
                        }
                    }
                }
                Err(SymphoniaError::IoError(_)) => continue,
                Err(SymphoniaError::DecodeError(_)) => continue,
                Err(err) => return Err(Box::new(err)),
            }
        }

        if let Some((pb, _)) = pb {
            pb.finish_and_clear();
        }

        Ok(())
    }
}

async fn run_deepgram_client(
    config: DeepgramClientConfig,
    connection_id: usize,
    connection_count: usize,
    detected_sample_rate: u32,
    detected_channels: u16,
    mut audio_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    ready_tx: Option<oneshot::Sender<()>>,
    mut shutdown_rx: mpsc::Receiver<()>,
) -> StreamResult {
    let prefix = connection_prefix(connection_id, connection_count);

    // Validate utterance_end dependencies
    if config.utterance_end.is_some() && (!config.interim_results || !config.vad_events) {
        return Err("--utterance-end requires --interim-results true and --vad-events".into());
    }

    // Use custom endpoint or default to Deepgram API
    let base_url = config
        .endpoint
        .clone()
        .unwrap_or_else(|| "wss://api.deepgram.com".to_string());

    // Start building the URL
    let mut url = format!("{}/v1/listen?", base_url);
    let mut params = Vec::new();

    // Add encoding parameter (default to linear16 if not specified)
    let encoding_value = config
        .encoding
        .clone()
        .unwrap_or_else(|| "linear16".to_string());
    params.push(format!("encoding={}", encoding_value));

    // Add sample_rate parameter (use override if provided, otherwise use detected)
    let sample_rate_value = config.sample_rate_override.unwrap_or(detected_sample_rate);
    params.push(format!("sample_rate={}", sample_rate_value));

    // Add channels parameter (use override if provided, otherwise use detected)
    let channels_value = config.channels_override.unwrap_or(detected_channels);
    params.push(format!("channels={}", channels_value));

    // Add multichannel parameter
    if config.multichannel {
        params.push("multichannel=true".to_string());
    }

    // Add diarize parameter
    if config.diarize {
        params.push("diarize=true".to_string());
    }

    // Add detect_entities parameter
    if config.detect_entities {
        params.push("detect_entities=true".to_string());
    }

    // Add interim_results parameter if specified
    if config.interim_results {
        params.push("interim_results=true".to_string());
    }

    // Add vad_events parameter
    if config.vad_events {
        params.push("vad_events=true".to_string());
    }

    if config.punctuate {
        params.push("punctuate=true".to_string());
    }

    if config.smart_format {
        params.push("smart_format=true".to_string());
    }

    if config.sentiment {
        params.push("sentiment=true".to_string());
    }

    if config.intents {
        params.push("intents=true".to_string());
    }

    if config.topics {
        params.push("topics=true".to_string());
    }

    // Add model parameter if specified
    if let Some(model_name) = &config.model {
        params.push(format!("model={}", model_name));
    }

    // Add redact parameter if specified
    if let Some(redact_value) = &config.redact {
        // Parse the redact value to handle categories and individual entities
        let redact_entities = parse_redact_entities(redact_value);
        if !redact_entities.is_empty() {
            params.push(format!("redact={}", redact_entities.join("&redact=")));
        }
    }

    // Add language parameter if specified
    if let Some(lang) = &config.language {
        params.push(format!("language={}", lang));
    }

    // Add endpointing parameter if specified
    if let Some(ep) = config.endpointing {
        params.push(format!("endpointing={}", ep));
    }

    // Add utterance_end_ms parameter if specified
    if let Some(ue) = config.utterance_end {
        params.push(format!("utterance_end_ms={}", ue));
    }

    // Add keyterm parameters if specified (each term becomes a separate keyterm= param)
    if let Some(keyterms) = &config.keyterm {
        for term in keyterms.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            params.push(format!("keyterm={}", urlencoding::encode(term)));
        }
    }

    // Add keywords parameters if specified (each entry becomes a separate keywords= param,
    // optionally with an intensifier: "word:2.0" or just "word")
    if let Some(kw) = &config.keywords {
        for entry in kw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            params.push(format!("keywords={}", urlencoding::encode(entry)));
        }
    }

    // Join all parameters
    url.push_str(&params.join("&"));

    // Add callback parameters if provided
    if let Some(callback_url) = &config.callback {
        url.push_str(&format!(
            "&callback={}&callback_method=post",
            urlencoding::encode(callback_url)
        ));
    }

    println!("{prefix}Connecting to Deepgram WebSocket...");

    let url_parsed = url::Url::parse(&url)?;
    let host = url_parsed.host_str().ok_or("Invalid host in URL")?;

    println!("{prefix}Connecting to Deepgram URL: {0}", &url);

    let mut request_builder = tokio_tungstenite::tungstenite::http::Request::builder()
        .method("GET")
        .uri(&url)
        .header("Host", host)
        .header("Upgrade", "websocket")
        .header("Connection", "Upgrade")
        .header(
            "Sec-WebSocket-Key",
            tokio_tungstenite::tungstenite::handshake::client::generate_key(),
        )
        .header("Sec-WebSocket-Version", "13");

    if let Some(api_key) = &config.api_key {
        request_builder = request_builder.header("Authorization", format!("Token {}", api_key));
    }

    let request = request_builder.body(())?;

    let (ws_stream, response) = connect_async(request).await.map_err(|e| {
        if let tokio_tungstenite::tungstenite::Error::Http(ref resp) = e {
            if let Some(request_id) = resp.headers().get("dg-request-id") {
                eprintln!(
                    "{prefix}Request ID: {}",
                    request_id.to_str().unwrap_or("(invalid)")
                );
            }
            let body = resp
                .body()
                .as_deref()
                .and_then(|b| std::str::from_utf8(b).ok())
                .unwrap_or("(no body)");
            eprintln!("{prefix}Error {}: {}", resp.status(), body);
        }
        e
    })?;
    println!("{prefix}Connected to Deepgram!");
    if let Some(request_id) = response.headers().get("dg-request-id") {
        println!(
            "{prefix}Request ID: {}",
            request_id.to_str().unwrap_or("(invalid)")
        );
    }

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

    let response_prefix = prefix.clone();
    let silent = config.silent;
    let diarize = config.diarize;
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
                                                    if diarize && !alternative.words.is_empty() {
                                                        // Group consecutive words by speaker
                                                        let mut segments: Vec<(u32, Vec<&str>)> = Vec::new();
                                                        for word in &alternative.words {
                                                            let speaker = word.speaker.unwrap_or(0);
                                                            if let Some(last) = segments.last_mut() {
                                                                if last.0 == speaker {
                                                                    last.1.push(&word.word);
                                                                    continue;
                                                                }
                                                            }
                                                            segments.push((speaker, vec![&word.word]));
                                                        }
                                                        for (speaker, words) in &segments {
                                                            println!("\r\x1b[2K{}Speaker {}: {}", response_prefix, speaker, words.join(" "));
                                                        }
                                                    } else {
                                                        print!("\r\x1b[2K{}Transcript: {}", response_prefix, alternative.transcript);
                                                        if let Some(confidence) = alternative.confidence {
                                                            print!(" (Confidence: {:.1}%)", confidence * 100.0);
                                                        }
                                                        println!();
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Failed to parse response: {}", e);
                                    if let Ok(mut f) = OpenOptions::new()
                                        .create(true)
                                        .append(true)
                                        .open("dg-stt-debug.log")
                                    {
                                        let _ = writeln!(f, "--- parse error: {} ---", e);
                                        let _ = writeln!(f, "{}", text);
                                    }
                                }
                            }
                        }
                        Some(Ok(Message::Close(_))) => {
                            if !silent {
                                println!("{}WebSocket connection closed by server", response_prefix);
                            }
                            break;
                        }
                        Some(Err(e)) => {
                            eprintln!("{}WebSocket error: {}", response_prefix, e);
                            break;
                        }
                        None => break,
                        _ => {}
                    }
                }
                _ = tokio::time::sleep_until(last_message_time + timeout_duration) => {
                    // No messages received for timeout duration, we're done
                    if !silent {
                        println!("{}No more messages received, finishing...", response_prefix);
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
                    eprintln!("{prefix}Failed to send audio to WebSocket");
                    break;
                }
            }
            _ = shutdown_rx.recv() => {
                println!("\n{prefix}Received shutdown signal, sending CloseStream message...");
                // Send CloseStream message
                let close_stream_msg = serde_json::json!({"type": "CloseStream"});
                if let Ok(msg_str) = serde_json::to_string(&close_stream_msg) {
                    let _ = msg_tx.send(Message::Text(msg_str.into()));
                }
                break;
            }
            _ = result_rx.recv() => {
                // WebSocket connection was closed.
                break;
            }
            else => {
                // Audio source exhausted (file done) — tell Deepgram we're finished
                let close_stream_msg = serde_json::json!({"type": "CloseStream"});
                if let Ok(msg_str) = serde_json::to_string(&close_stream_msg) {
                    let _ = msg_tx.send(Message::Text(msg_str.into()));
                }
                break;
            }
        }
    }

    println!(
        "{prefix}Sent {} audio chunks, waiting for transcription results...",
        audio_count
    );

    // Stop sending messages
    drop(msg_tx);

    // Wait for the response handler first — it completes as soon as the WS closes.
    // Awaiting keepalive/sender first would hang: they can only exit after ws_sender
    // errors, which doesn't happen until the TCP teardown completes (several seconds).
    let _ = response_handler.await;

    // WS is now closed; abort the other tasks rather than waiting for the chain to
    // propagate through sender_task → keepalive_task.
    keepalive_task.abort();
    sender_task.abort();
    let _ = keepalive_task.await;
    let _ = sender_task.await;

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

async fn wait_for_deepgram_tasks(
    tasks: Vec<JoinHandle<StreamResult>>,
    connection_count: usize,
    success_message: &'static str,
) {
    for (idx, task) in tasks.into_iter().enumerate() {
        let connection_id = idx + 1;
        match task.await {
            Ok(Ok(())) => println!(
                "{}{}",
                connection_prefix(connection_id, connection_count),
                success_message
            ),
            Ok(Err(e)) => eprintln!(
                "{}Deepgram client error: {}",
                connection_prefix(connection_id, connection_count),
                e
            ),
            Err(e) => eprintln!(
                "{}Deepgram task join error: {}",
                connection_prefix(connection_id, connection_count),
                e
            ),
        }
    }
}

async fn wait_for_file_tasks(
    stream_task: JoinHandle<StreamResult>,
    fanout_task: JoinHandle<()>,
    deepgram_tasks: Vec<JoinHandle<StreamResult>>,
    connection_count: usize,
) {
    match stream_task.await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => eprintln!("File streaming error: {}", e),
        Err(e) => eprintln!("Stream task join error: {}", e),
    }

    match fanout_task.await {
        Ok(()) => {}
        Err(e) if e.is_cancelled() => {}
        Err(e) => eprintln!("Audio fan-out task join error: {}", e),
    }

    wait_for_deepgram_tasks(
        deepgram_tasks,
        connection_count,
        "Deepgram client finished successfully",
    )
    .await;
}

async fn run_microphone_mode(
    api_key: Option<String>,
    connections: usize,
    callback: Option<String>,
    silent: bool,
    endpoint: Option<String>,
    encoding: Option<String>,
    sample_rate_override: Option<u32>,
    channels_override: Option<u16>,
    multichannel: bool,
    diarize: bool,
    detect_entities: bool,
    interim_results: bool,
    vad_events: bool,
    punctuate: bool,
    smart_format: bool,
    sentiment: bool,
    intents: bool,
    topics: bool,
    model: Option<String>,
    redact: Option<String>,
    language: Option<String>,
    keyterm: Option<String>,
    keywords: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    if connections == 0 {
        return Err("--connections must be at least 1".into());
    }

    println!("Starting Deepgram real-time transcription from microphone...");

    let audio_capture = AudioCapture::new()?;
    let sample_rate = audio_capture.config.sample_rate.0;
    let channels = audio_capture.config.channels;

    println!(
        "Audio config - Sample rate: {}, Channels: {}",
        sample_rate, channels
    );

    let (audio_tx, audio_receivers, fanout_task) = start_audio_fanout(connections);
    let mut shutdown_senders = Vec::with_capacity(connections);

    let stream_handle = audio_capture.start_capture(audio_tx)?;

    println!("Listening for audio... Press Ctrl+C to stop.");

    let client_config = DeepgramClientConfig {
        api_key,
        callback,
        silent,
        endpoint,
        encoding,
        sample_rate_override,
        channels_override,
        multichannel,
        diarize,
        detect_entities,
        interim_results,
        vad_events,
        punctuate,
        smart_format,
        sentiment,
        intents,
        topics,
        model,
        redact,
        language,
        endpointing: None,
        utterance_end: None,
        keyterm,
        keywords,
    };

    let mut deepgram_tasks = Vec::with_capacity(connections);
    for (idx, audio_rx) in audio_receivers.into_iter().enumerate() {
        let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>(1);
        shutdown_senders.push(shutdown_tx);
        deepgram_tasks.push(tokio::spawn(run_deepgram_client(
            client_config.clone(),
            idx + 1,
            connections,
            sample_rate,
            channels,
            audio_rx,
            None,
            shutdown_rx,
        )));
    }

    let mut deepgram_tasks_future = Box::pin(wait_for_deepgram_tasks(
        deepgram_tasks,
        connections,
        "Deepgram client finished successfully",
    ));

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            println!("\nReceived Ctrl+C, initiating graceful shutdown...");
            for shutdown_tx in shutdown_senders {
                let _ = shutdown_tx.send(()).await;
            }

            deepgram_tasks_future.await;
        }
        _ = &mut deepgram_tasks_future => {}
    }

    drop(stream_handle);
    fanout_task.abort();
    let _ = fanout_task.await;

    Ok(())
}

async fn run_file_mode(
    api_key: Option<String>,
    connections: usize,
    file_path: PathBuf,
    fast: bool,
    callback: Option<String>,
    silent: bool,
    endpoint: Option<String>,
    encoding: Option<String>,
    sample_rate_override: Option<u32>,
    channels_override: Option<u16>,
    multichannel: bool,
    diarize: bool,
    detect_entities: bool,
    interim_results: bool,
    vad_events: bool,
    punctuate: bool,
    smart_format: bool,
    sentiment: bool,
    intents: bool,
    topics: bool,
    model: Option<String>,
    redact: Option<String>,
    language: Option<String>,
    endpointing: Option<u32>,
    utterance_end: Option<u32>,
    keyterm: Option<String>,
    keywords: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    if connections == 0 {
        return Err("--connections must be at least 1".into());
    }

    println!("Starting Deepgram transcription from file...");
    println!("File: {}", file_path.display());
    println!("Mode: {}", if fast { "Fast" } else { "Real-time" });

    let (audio_tx, audio_receivers, fanout_task) = start_audio_fanout(connections);
    let (config_tx, config_rx) = oneshot::channel::<(u32, u16)>();
    let mut shutdown_senders = Vec::with_capacity(connections);

    let file_reader = AudioFileReader::new(file_path);

    // Always create a ready channel — stream_file waits on it before showing
    // the progress bar, ensuring the request ID is printed first.
    let (stream_ready_tx, stream_ready_rx) = oneshot::channel::<()>();

    // Start streaming file audio in the background
    let stream_task = tokio::spawn(async move {
        file_reader
            .stream_file(audio_tx, config_tx, Some(stream_ready_rx), fast)
            .await
    });

    // Wait for the audio configuration to be sent. If the channel closed without
    // sending, stream_file failed early (e.g. unsupported format) — surface that error.
    let (sample_rate, channels) = match config_rx.await {
        Ok(cfg) => cfg,
        Err(_) => {
            return match stream_task.await {
                Ok(Err(e)) => Err(format!("Failed to read audio file: {e}").into()),
                _ => Err("Failed to read audio file: unknown error".into()),
            };
        }
    };

    let client_config = DeepgramClientConfig {
        api_key,
        callback,
        silent,
        endpoint,
        encoding,
        sample_rate_override,
        channels_override,
        multichannel,
        diarize,
        detect_entities,
        interim_results,
        vad_events,
        punctuate,
        smart_format,
        sentiment,
        intents,
        topics,
        model,
        redact,
        language,
        endpointing,
        utterance_end,
        keyterm,
        keywords,
    };

    let mut ready_receivers = Vec::with_capacity(connections);
    let mut deepgram_tasks = Vec::with_capacity(connections);
    for (idx, audio_rx) in audio_receivers.into_iter().enumerate() {
        let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>(1);
        let (ready_tx, ready_rx) = oneshot::channel::<()>();
        shutdown_senders.push(shutdown_tx);
        ready_receivers.push(ready_rx);
        deepgram_tasks.push(tokio::spawn(run_deepgram_client(
            client_config.clone(),
            idx + 1,
            connections,
            sample_rate,
            channels,
            audio_rx,
            Some(ready_tx),
            shutdown_rx,
        )));
    }

    tokio::spawn(async move {
        for ready_rx in ready_receivers {
            let _ = ready_rx.await;
        }
        let _ = stream_ready_tx.send(());
    });

    // Wait for either CTRL+C or all tasks to complete.
    let mut tasks_future = Box::pin(wait_for_file_tasks(
        stream_task,
        fanout_task,
        deepgram_tasks,
        connections,
    ));

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            println!("\nReceived Ctrl+C, initiating graceful shutdown...");
            for shutdown_tx in shutdown_senders {
                let _ = shutdown_tx.send(()).await;
            }

            tasks_future.await;
        }
        _ = &mut tasks_future => {
            println!("\nTranscription completed successfully");
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    let cli = Cli::parse();

    match cli.command {
        Commands::Transcribe { args } => {
            let api_key = api_key_for_endpoint(args.endpoint.as_deref())?;
            transcribe::run_transcribe_mode(api_key, args).await?
        }
        Commands::Stream { source } => match source {
            StreamSource::Microphone {
                callback,
                silent,
                endpoint,
                connections,
                encoding,
                sample_rate,
                channels,
                multichannel,
                diarize,
                detect_entities,
                interim_results,
                vad_events,
                punctuate,
                smart_format,
                sentiment,
                intents,
                topics,
                model,
                redact,
                language,
                keyterm,
                keywords,
            } => {
                let api_key = api_key_for_endpoint(endpoint.as_deref())?;
                run_microphone_mode(
                    api_key,
                    connections,
                    callback,
                    silent,
                    endpoint,
                    encoding,
                    sample_rate,
                    channels,
                    multichannel,
                    diarize,
                    detect_entities,
                    interim_results,
                    vad_events,
                    punctuate,
                    smart_format,
                    sentiment,
                    intents,
                    topics,
                    model,
                    redact,
                    language,
                    keyterm,
                    keywords,
                )
                .await?
            }
            StreamSource::File {
                file,
                fast,
                callback,
                silent,
                endpoint,
                connections,
                encoding,
                sample_rate,
                channels,
                multichannel,
                diarize,
                detect_entities,
                interim_results,
                vad_events,
                punctuate,
                smart_format,
                sentiment,
                intents,
                topics,
                model,
                redact,
                language,
                endpointing,
                utterance_end,
                keyterm,
                keywords,
            } => {
                let api_key = api_key_for_endpoint(endpoint.as_deref())?;
                run_file_mode(
                    api_key,
                    connections,
                    file,
                    fast,
                    callback,
                    silent,
                    endpoint,
                    encoding,
                    sample_rate,
                    channels,
                    multichannel,
                    diarize,
                    detect_entities,
                    interim_results,
                    vad_events,
                    punctuate,
                    smart_format,
                    sentiment,
                    intents,
                    topics,
                    model,
                    redact,
                    language,
                    endpointing,
                    utterance_end,
                    keyterm,
                    keywords,
                )
                .await?
            }
        },
    }

    Ok(())
}
