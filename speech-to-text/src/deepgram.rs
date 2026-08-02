use futures_util::{SinkExt, StreamExt};
use std::fs::OpenOptions;
use std::io::Write;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

use crate::audio::connection_prefix;
use crate::protocol::{Channel, DeepgramClientConfig, DeepgramResponse, StreamResult};

pub(crate) async fn run_deepgram_client(
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
                                        let channel = response.channel.and_then(|channel| {
                                            serde_json::from_value::<Channel>(channel).ok()
                                        });
                                        if let Some(channel) = channel {
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
