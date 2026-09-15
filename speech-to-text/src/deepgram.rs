use futures_util::{SinkExt, StreamExt};
use std::collections::HashSet;
use std::fs::OpenOptions;
use std::io::Write;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

use crate::audio::{AudioEvent, AudioReceiver, connection_prefix};
use crate::protocol::{Channel, DeepgramClientConfig, DeepgramResponse, StreamResult};

fn lifecycle_log(enabled: bool, prefix: &str, _started: Instant, message: impl std::fmt::Display) {
    if enabled {
        eprintln!("{prefix}{message}");
    }
}

fn control_message_name(message: &Message) -> Option<&'static str> {
    let Message::Text(text) = message else {
        return None;
    };

    if text.contains("\"Finalize\"") {
        Some("Finalize")
    } else if text.contains("\"CloseStream\"") {
        Some("CloseStream")
    } else if text.contains("\"KeepAlive\"") {
        Some("KeepAlive")
    } else {
        Some("text control message")
    }
}

async fn finalize_and_close(
    msg_tx: &mpsc::Sender<Message>,
    finalize_rx: &mut oneshot::Receiver<()>,
    shutdown_rx: &mut mpsc::Receiver<()>,
    result_rx: &mut mpsc::UnboundedReceiver<()>,
    prefix: &str,
    lifecycle_verbose: bool,
    lifecycle_started: Instant,
    reason: &str,
) -> StreamResult {
    lifecycle_log(lifecycle_verbose, prefix, lifecycle_started, reason);
    let finalize_msg = serde_json::json!({"type": "Finalize"});
    let msg_str = serde_json::to_string(&finalize_msg)?;
    msg_tx
        .send(Message::Text(msg_str.into()))
        .await
        .map_err(|_| "WebSocket sender closed before Finalize could be queued")?;
    lifecycle_log(
        lifecycle_verbose,
        prefix,
        lifecycle_started,
        "Finalize enqueued for WebSocket sender",
    );

    let finalized = tokio::select! {
        biased;
        result = &mut *finalize_rx => {
            result.map_err(|_| "Deepgram connection closed before sending a from_finalize result")?;
            lifecycle_log(
                lifecycle_verbose,
                prefix,
                lifecycle_started,
                "Main task received finalize acknowledgement",
            );
            true
        }
        _ = shutdown_rx.recv() => {
            lifecycle_log(
                lifecycle_verbose,
                prefix,
                lifecycle_started,
                "Shutdown requested while waiting for from_finalize",
            );
            false
        }
        _ = result_rx.recv() => {
            finalize_rx
                .await
                .map_err(|_| "Response handler exited before sending a from_finalize result")?;
            lifecycle_log(
                lifecycle_verbose,
                prefix,
                lifecycle_started,
                "Response handler completed after sending finalize acknowledgement",
            );
            true
        }
    };

    let close_stream_msg = serde_json::json!({"type": "CloseStream"});
    let close_str = serde_json::to_string(&close_stream_msg)?;
    msg_tx
        .send(Message::Text(close_str.into()))
        .await
        .map_err(|_| "WebSocket sender closed before CloseStream could be queued")?;
    lifecycle_log(
        lifecycle_verbose,
        prefix,
        lifecycle_started,
        if finalized {
            "CloseStream enqueued after finalize acknowledgement"
        } else {
            "CloseStream enqueued after shutdown request"
        },
    );
    Ok(())
}

fn add_used_models(
    response: &DeepgramResponse,
    used_models: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    let Some(metadata) = &response.metadata else {
        return;
    };

    if let Some(model_info) = &metadata.model_info {
        let name = model_info.name.as_deref().unwrap_or("unknown");
        let version = model_info.version.as_deref().unwrap_or("unknown");
        let arch = model_info.arch.as_deref().unwrap_or("unknown");
        let model_uuid = metadata.model_uuid.as_deref().unwrap_or("unknown");
        let model = format!("{name} (version: {version}, arch: {arch}, model UUID: {model_uuid})");
        if seen.insert(model.clone()) {
            used_models.push(model);
        }
    }

    if let Some(diarize_info) = &metadata.diarize_info {
        let arch = diarize_info.arch.as_deref().unwrap_or("unknown");
        let model_uuid = diarize_info.model_uuid.as_deref().unwrap_or("unknown");
        let model = format!("diarization (arch: {arch}, model UUID: {model_uuid})");
        if seen.insert(model.clone()) {
            used_models.push(model);
        }
    }
}

pub(crate) async fn run_deepgram_client(
    config: DeepgramClientConfig,
    connection_id: usize,
    connection_count: usize,
    detected_sample_rate: u32,
    detected_channels: u16,
    mut audio_rx: AudioReceiver,
    ready_tx: Option<oneshot::Sender<()>>,
    mut shutdown_rx: mpsc::Receiver<()>,
    finalize_on_audio_end: bool,
) -> StreamResult {
    let prefix = connection_prefix(connection_id, connection_count);

    // UtteranceEnd relies on interim results to detect the gap after the last finalized word.
    if config.utterance_end.is_some() && !config.interim_results {
        return Err("--utterance-end requires --interim-results".into());
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

    // diarize_model enables diarization and selects the newest available model.
    if config.diarize {
        params.push("diarize_model=latest".to_string());
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

    if config.profanity_filter {
        params.push("profanity_filter=true".to_string());
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

    // Add model version parameter if specified.
    if let Some(version) = &config.version {
        params.push(format!("version={}", urlencoding::encode(version)));
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
    let connect_started = Instant::now();

    let monitor_tx_for_connect = config.monitor_tx.clone();
    let (ws_stream, response) = connect_async(request).await.map_err(|e| {
        if let Some(tx) = &monitor_tx_for_connect {
            let _ = tx.send(crate::monitor::MonitorEvent::Status {
                connection_id,
                status: crate::monitor::ConnectionStatus::Error(e.to_string()),
            });
        }
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
    if let Some(tx) = &config.monitor_tx {
        let request_id = response
            .headers()
            .get("dg-request-id")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("unknown")
            .to_string();
        let _ = tx.send(crate::monitor::MonitorEvent::Connected {
            connection_id,
            request_id,
            established_after: connect_started.elapsed(),
        });
        let _ = tx.send(crate::monitor::MonitorEvent::Status {
            connection_id,
            status: crate::monitor::ConnectionStatus::Connected,
        });
    }
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
    let (finalize_tx, mut finalize_rx) = oneshot::channel::<()>();
    let lifecycle_started = Instant::now();
    let lifecycle_verbose = config.verbose;
    lifecycle_log(
        lifecycle_verbose,
        &prefix,
        lifecycle_started,
        if finalize_on_audio_end {
            "Streaming mode: file"
        } else {
            "Streaming mode: microphone"
        },
    );
    // Keep a small bounded queue so fast file streaming cannot build a large
    // backlog of audio ahead of the Finalize control message.
    let (msg_tx, mut msg_rx) = mpsc::channel::<Message>(8);
    let sender_prefix = prefix.clone();

    // Spawn a task to handle sending messages to WebSocket
    let sender_task = tokio::spawn(async move {
        let mut ws_sender = ws_sender;
        while let Some(msg) = msg_rx.recv().await {
            let control_name = control_message_name(&msg);
            if ws_sender.send(msg).await.is_err() {
                lifecycle_log(
                    lifecycle_verbose,
                    &sender_prefix,
                    lifecycle_started,
                    "WebSocket sender failed while transmitting a message",
                );
                break;
            }
            if let Some(name) = control_name {
                lifecycle_log(
                    lifecycle_verbose,
                    &sender_prefix,
                    lifecycle_started,
                    format!("WebSocket sender transmitted {name}"),
                );
            }
        }
        lifecycle_log(
            lifecycle_verbose,
            &sender_prefix,
            lifecycle_started,
            "WebSocket sender task exited",
        );
    });

    // Spawn a keep-alive task that sends a message every 5 seconds
    let keepalive_tx = msg_tx.clone();
    let keepalive_task = tokio::spawn(async move {
        if finalize_on_audio_end {
            return;
        }

        let mut interval = tokio::time::interval(Duration::from_secs(5));
        interval.tick().await; // Skip the first immediate tick

        loop {
            interval.tick().await;
            // Send keep-alive message
            let keepalive_msg = serde_json::json!({"type": "KeepAlive"});
            if let Ok(msg_str) = serde_json::to_string(&keepalive_msg) {
                if keepalive_tx
                    .send(Message::Text(msg_str.into()))
                    .await
                    .is_err()
                {
                    break;
                }
            }
        }
    });

    let response_prefix = prefix.clone();
    let silent = config.silent;
    let output_json = config.output == "json";
    let verbose = config.verbose;
    let diarize = config.diarize;
    let monitor_tx = config.monitor_tx.clone();
    let response_handler = tokio::spawn(async move {
        let mut finalize_tx = Some(finalize_tx);
        let mut used_models = Vec::new();
        let mut seen_models = HashSet::new();

        loop {
            tokio::select! {
                msg = ws_receiver.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            let parsed_response = serde_json::from_str::<DeepgramResponse>(&text);
                            if let Ok(response) = &parsed_response {
                                add_used_models(response, &mut used_models, &mut seen_models);
                            }
                            if output_json {
                                if !silent {
                                    println!("{}", text);
                                }
                                if parsed_response
                                    .as_ref()
                                    .is_ok_and(|response| response.from_finalize)
                                {
                                    lifecycle_log(
                                        lifecycle_verbose,
                                        &response_prefix,
                                        lifecycle_started,
                                        "Received from_finalize response (JSON output); final transcript processed",
                                    );
                                    if let Some(tx) = finalize_tx.take() {
                                        let _ = tx.send(());
                                    }
                                    break;
                                }
                                continue;
                            }
                            match parsed_response {
                                Ok(response) => {
                                    let from_finalize = response.from_finalize;
                                    if response.message_type == "Metadata" {
                                        if !silent {
                                            println!("{}Metadata: {}", response_prefix, text);
                                        }
                                    } else if response.message_type == "Results" {
                                        let channel = response.channel.and_then(|channel| {
                                            serde_json::from_value::<Channel>(channel).ok()
                                        });
                                        if let Some(channel) = channel {
                                            if let Some(tx) = &monitor_tx {
                                                let words = channel
                                                    .alternatives
                                                    .first()
                                                    .map(|alternative| {
                                                        if alternative.words.is_empty() {
                                                            alternative
                                                                .transcript
                                                                .split_whitespace()
                                                                .map(str::to_string)
                                                                .collect()
                                                        } else {
                                                            alternative
                                                                .words
                                                                .iter()
                                                                .map(|word| word.word.clone())
                                                                .collect()
                                                        }
                                                    })
                                                    .unwrap_or_default();
                                                let _ = tx.send(crate::monitor::MonitorEvent::Transcript {
                                                    connection_id,
                                                    words,
                                                    is_final: response.is_final,
                                                });
                                                let _ = tx.send(crate::monitor::MonitorEvent::Status {
                                                    connection_id,
                                                    status: crate::monitor::ConnectionStatus::Streaming,
                                                });
                                            }
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
                                    if from_finalize {
                                        lifecycle_log(
                                            lifecycle_verbose,
                                            &response_prefix,
                                            lifecycle_started,
                                            "Received from_finalize response; final transcript processed",
                                        );
                                        if let Some(tx) = finalize_tx.take() {
                                            let _ = tx.send(());
                                        }
                                        break;
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
                            if let Some(tx) = &monitor_tx {
                                let _ = tx.send(crate::monitor::MonitorEvent::Status {
                                    connection_id,
                                    status: crate::monitor::ConnectionStatus::Closed,
                                });
                            }
                            if !silent {
                                println!("{}WebSocket connection closed by server", response_prefix);
                            }
                            break;
                        }
                        Some(Err(e)) => {
                            if let Some(tx) = &monitor_tx {
                                let _ = tx.send(crate::monitor::MonitorEvent::Status {
                                    connection_id,
                                    status: crate::monitor::ConnectionStatus::Error(e.to_string()),
                                });
                            }
                            eprintln!("{}WebSocket error: {}", response_prefix, e);
                            break;
                        }
                        None => break,
                        _ => {}
                    }
                }
            }
        }
        if verbose && !used_models.is_empty() {
            println!("{response_prefix}Models used:");
            for model in used_models {
                println!("{response_prefix}- {model}");
            }
        }
        lifecycle_log(
            lifecycle_verbose,
            &response_prefix,
            lifecycle_started,
            "Response handler task exited",
        );
        let _ = result_tx.send(());
    });

    let mut audio_count = 0;
    loop {
        tokio::select! {
            biased;
            audio_event = audio_rx.recv() => {
                match audio_event {
                    Some(AudioEvent::Data(audio_data)) => {
                        audio_count += 1;
                        if msg_tx.send(Message::Binary(audio_data.into())).await.is_err() {
                            eprintln!("{prefix}Failed to send audio to WebSocket");
                            break;
                        }
                    }
                    Some(AudioEvent::End) | None => {
                        keepalive_task.abort();
                        if finalize_on_audio_end {
                            finalize_and_close(
                                &msg_tx,
                                &mut finalize_rx,
                                &mut shutdown_rx,
                                &mut result_rx,
                                &prefix,
                                lifecycle_verbose,
                                lifecycle_started,
                                "Explicit end-of-audio marker received; enqueueing Finalize",
                            )
                            .await?;
                        } else {
                            let close_stream_msg = serde_json::json!({"type": "CloseStream"});
                            let close_str = serde_json::to_string(&close_stream_msg)?;
                            let _ = msg_tx.send(Message::Text(close_str.into())).await;
                        }
                        break;
                    }
                }
            }
            shutdown_result = shutdown_rx.recv() => {
                keepalive_task.abort();
                lifecycle_log(
                    lifecycle_verbose,
                    &prefix,
                    lifecycle_started,
                    if finalize_on_audio_end {
                        "Keepalive task aborted before file finalization"
                    } else {
                        "Keepalive task aborted before microphone finalization"
                    },
                );
                if shutdown_result.is_none() {
                    lifecycle_log(
                        lifecycle_verbose,
                        &prefix,
                        lifecycle_started,
                        "Shutdown channel closed without an explicit shutdown signal",
                    );
                }
                lifecycle_log(
                    lifecycle_verbose,
                    &prefix,
                    lifecycle_started,
                    if shutdown_result.is_some() {
                        "Explicit shutdown signal received; enqueueing Finalize"
                    } else {
                        "Shutdown channel closure received; enqueueing Finalize"
                    },
                );
                finalize_and_close(
                    &msg_tx,
                    &mut finalize_rx,
                    &mut shutdown_rx,
                    &mut result_rx,
                    &prefix,
                    lifecycle_verbose,
                    lifecycle_started,
                    if shutdown_result.is_some() {
                        "Explicit shutdown signal received; enqueueing Finalize"
                    } else {
                        "Shutdown channel closure received; enqueueing Finalize"
                    },
                )
                .await?;
                break;
            }
            _ = result_rx.recv() => {
                // WebSocket connection was closed.
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

    lifecycle_log(
        lifecycle_verbose,
        &prefix,
        lifecycle_started,
        "Audio/control loop exited; waiting for response handler",
    );

    // Wait for the response handler first — it completes after the final result
    // has been processed or the WebSocket has closed.
    // Awaiting keepalive/sender first would hang: they can only exit after ws_sender
    // errors, which doesn't happen until the TCP teardown completes (several seconds).
    let _ = response_handler.await;
    lifecycle_log(
        lifecycle_verbose,
        &prefix,
        lifecycle_started,
        "Response handler join completed",
    );

    // WS is now closed; abort the other tasks rather than waiting for the chain to
    // propagate through sender_task → keepalive_task.
    lifecycle_log(
        lifecycle_verbose,
        &prefix,
        lifecycle_started,
        "Aborting keepalive and WebSocket sender tasks",
    );
    keepalive_task.abort();
    sender_task.abort();
    let _ = keepalive_task.await;
    let _ = sender_task.await;
    lifecycle_log(
        lifecycle_verbose,
        &prefix,
        lifecycle_started,
        "Shutdown task cleanup completed",
    );

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
