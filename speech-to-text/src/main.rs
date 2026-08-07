mod audio;
mod cli;
mod deepgram;
mod models;
mod protocol;
mod stream;
mod transcribe;

use clap::Parser;
use dotenv::dotenv;
use std::env;
use std::path::PathBuf;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::audio::{AudioCapture, AudioFileReader, connection_prefix, start_audio_fanout};
use crate::cli::{Cli, Commands};
use crate::deepgram::run_deepgram_client;
use crate::protocol::{DeepgramClientConfig, StreamResult};
use crate::stream::StreamSource;

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
    use crate::audio::{repair_wav_header, wav_data_len};
    use crate::protocol::DeepgramResponse;

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

    #[test]
    fn response_parser_accepts_message_specific_channel_shapes() {
        let results: DeepgramResponse =
            serde_json::from_str(r#"{"type":"Results","channel":{"alternatives":[]}}"#).unwrap();
        assert!(results.channel.unwrap().is_object());

        let speech_started: DeepgramResponse =
            serde_json::from_str(r#"{"type":"SpeechStarted","channel":0}"#).unwrap();
        assert_eq!(speech_started.channel.unwrap(), serde_json::json!(0));

        let utterance_end: DeepgramResponse =
            serde_json::from_str(r#"{"type":"UtteranceEnd","channel":[0,1],"last_word_end":1.2}"#)
                .unwrap();
        assert_eq!(utterance_end.channel.unwrap(), serde_json::json!([0, 1]));
    }

    #[test]
    fn wav_data_len_uses_bytes_when_declared_chunk_size_is_zero() {
        let mut wav = b"RIFF\0\0\0\0WAVEdata\0\0\0\0".to_vec();
        wav.extend_from_slice(&[0; 8]);

        assert_eq!(wav_data_len(&wav), Some(8));
    }

    #[test]
    fn malformed_wav_header_is_repaired_for_streaming() {
        let mut wav = b"RIFF\0\0\0\0WAVEdata\0\0\0\0".to_vec();
        wav.extend_from_slice(&[0; 8]);

        let repaired = repair_wav_header(wav).unwrap();
        assert_eq!(&repaired[16..20], &8u32.to_le_bytes());
    }
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
    version: Option<String>,
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
        version,
        redact,
        language,
        endpointing,
        utterance_end,
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
    version: Option<String>,
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
        version,
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
        Commands::ListModels {
            include_outdated,
            endpoint,
        } => {
            let api_key = api_key_for_endpoint(endpoint.as_deref())?;
            models::run_list_models(api_key, endpoint, include_outdated).await?
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
                version,
                redact,
                language,
                endpointing,
                utterance_end,
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
                    version,
                    redact,
                    language,
                    endpointing,
                    utterance_end,
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
                version,
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
                    version,
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
