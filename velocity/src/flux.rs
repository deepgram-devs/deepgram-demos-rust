use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

use crate::{audio, beep, deepgram, logger};

#[derive(serde::Deserialize)]
struct FluxMsg {
    #[serde(rename = "type")]
    msg_type: String,
    event: Option<String>,
    transcript: Option<String>,
    #[serde(default)]
    words: Vec<FluxWord>,
}

#[derive(serde::Deserialize)]
struct FluxWord {
    word: String,
}

fn parse_transcript(text: &str) -> Option<String> {
    let msg: FluxMsg = serde_json::from_str(text).ok()?;
    if msg.msg_type != "TurnInfo" || msg.event.as_deref() != Some("EndOfTurn") {
        return None;
    }

    if let Some(transcript) = msg
        .transcript
        .map(|transcript| transcript.trim().to_string())
        .filter(|transcript| !transcript.is_empty())
    {
        return Some(transcript);
    }

    let transcript = msg
        .words
        .into_iter()
        .map(|word| word.word)
        .collect::<Vec<_>>()
        .join(" ");
    (!transcript.trim().is_empty()).then_some(transcript)
}

fn log_text_message(text: &str) {
    match serde_json::from_str::<serde_json::Value>(text) {
        Ok(value) => {
            let msg_type = value
                .get("type")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("Unknown");
            let event = value
                .get("event")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            let message = value
                .get("message")
                .or_else(|| value.get("error"))
                .or_else(|| value.get("reason"))
                .and_then(serde_json::Value::as_str);

            if let Some(message) = message {
                logger::log(&format!(
                    "Flux: server message type={msg_type} event={event} message={message}"
                ));
            } else {
                logger::verbose(&format!(
                    "Flux: received text message type={msg_type} event={event}"
                ));
            }

            logger::verbose(&format!("Flux raw message: {text}"));
        }
        Err(error) => {
            logger::log(&format!("Flux: failed to parse server JSON: {error}"));
            logger::verbose(&format!("Flux raw non-JSON message: {text}"));
        }
    }
}

fn log_close_frame(frame: Option<tungstenite::protocol::CloseFrame>) {
    if let Some(frame) = frame {
        logger::log(&format!(
            "Flux: server closed WebSocket code={} reason={}",
            frame.code, frame.reason
        ));
    } else {
        logger::log("Flux: server closed WebSocket without a close frame");
    }
}

pub fn run(
    api_key: &str,
    model: &str,
    language_hint: Option<&str>,
    key_terms: &[String],
    selected_device_name: Option<&str>,
    active: Arc<AtomicBool>,
    on_transcript: Arc<dyn Fn(String) + Send + Sync>,
) {
    let url = build_flux_url(model, language_hint, key_terms);
    logger::log(&format!(
        "Flux: starting stream model={} language_hint={} keyterms={} audio_input={}",
        model,
        language_hint.unwrap_or(""),
        key_terms
            .iter()
            .filter(|term| !term.trim().is_empty())
            .count(),
        selected_device_name.unwrap_or("Default system input")
    ));
    log_query_string("Deepgram Flux", &url);

    use tungstenite::client::IntoClientRequest;
    let mut req = match url.into_client_request() {
        Ok(request) => request,
        Err(error) => {
            logger::log(&format!("Flux: bad request: {error}"));
            active.store(false, Ordering::Relaxed);
            return;
        }
    };

    match format!("Token {}", api_key).parse() {
        Ok(value) => {
            req.headers_mut().insert("Authorization", value);
        }
        Err(error) => {
            logger::log(&format!("Flux: bad auth header: {error}"));
            active.store(false, Ordering::Relaxed);
            return;
        }
    }

    let (mut ws, response) = match tungstenite::connect(req) {
        Ok(connection) => connection,
        Err(error) => {
            logger::log(&format!("Flux: connect error: {error}"));
            active.store(false, Ordering::Relaxed);
            return;
        }
    };
    logger::log(&format!("Flux: connected status={}", response.status()));

    let timeout = Some(Duration::from_millis(20));
    match ws.get_mut() {
        tungstenite::stream::MaybeTlsStream::Plain(tcp) => {
            let _ = tcp.set_read_timeout(timeout);
        }
        tungstenite::stream::MaybeTlsStream::NativeTls(tls) => {
            let _ = tls.get_ref().set_read_timeout(timeout);
        }
        _ => {}
    }

    let mut cap = match audio::AudioCapture::new_with_buffer_ms(selected_device_name, 80) {
        Some(capture) => capture,
        None => {
            logger::log("Flux: could not open selected audio capture device");
            active.store(false, Ordering::Relaxed);
            let _ = ws.close(None);
            return;
        }
    };
    cap.start();
    beep::play_start();
    logger::log(&format!(
        "Flux: audio capture started actual_device={} requested_device={}",
        cap.actual_device.actual_name,
        cap.actual_device.requested_name.as_deref().unwrap_or("")
    ));

    let initial_window = unsafe { GetForegroundWindow() };
    let mut pcm_buf: Vec<i16> = Vec::new();
    let mut audio_frames_sent: u64 = 0;
    let mut audio_bytes_sent: u64 = 0;
    let mut text_messages_received: u64 = 0;
    let mut transcripts_delivered: u64 = 0;
    let mut last_stats_log = Instant::now();

    loop {
        if !active.load(Ordering::Relaxed) {
            logger::log("Flux: active flag cleared, stopping");
            break;
        }

        let current_window = unsafe { GetForegroundWindow() };
        if current_window.0 != std::ptr::null_mut() && current_window != initial_window {
            logger::log("Flux: focus changed, stopping");
            active.store(false, Ordering::Relaxed);
            break;
        }

        pcm_buf.clear();
        cap.collect_ready(&mut pcm_buf);
        if !pcm_buf.is_empty() {
            let bytes: Vec<u8> = pcm_buf
                .iter()
                .flat_map(|sample| sample.to_le_bytes())
                .collect();
            let byte_len = bytes.len() as u64;
            if let Err(error) = ws.send(tungstenite::Message::Binary(bytes.into())) {
                logger::log(&format!(
                    "Flux: send error after {audio_frames_sent} frames: {error}"
                ));
                active.store(false, Ordering::Relaxed);
                break;
            }
            audio_frames_sent += 1;
            audio_bytes_sent += byte_len;
        }

        match ws.read() {
            Ok(tungstenite::Message::Text(text)) => {
                text_messages_received += 1;
                log_text_message(&text);
                if let Some(transcript) = parse_transcript(&text) {
                    transcripts_delivered += 1;
                    logger::log(&format!("Flux transcript: {transcript}"));
                    on_transcript(format!("{transcript} "));
                }
            }
            Ok(tungstenite::Message::Close(frame)) => {
                log_close_frame(frame);
                active.store(false, Ordering::Relaxed);
                break;
            }
            Ok(tungstenite::Message::Ping(_)) => {
                logger::verbose("Flux: received ping");
            }
            Ok(tungstenite::Message::Pong(_)) => {
                logger::verbose("Flux: received pong");
            }
            Ok(tungstenite::Message::Binary(bytes)) => {
                logger::verbose(&format!(
                    "Flux: received unexpected binary message bytes={}",
                    bytes.len()
                ));
            }
            Err(tungstenite::Error::Io(ref error))
                if error.kind() == std::io::ErrorKind::WouldBlock
                    || error.kind() == std::io::ErrorKind::TimedOut => {}
            Err(error) => {
                logger::log(&format!(
                    "Flux: read error after {text_messages_received} text messages: {error}"
                ));
                active.store(false, Ordering::Relaxed);
                break;
            }
            Ok(message) => {
                logger::verbose(&format!("Flux: received unhandled message: {message:?}"));
            }
        }

        if last_stats_log.elapsed() >= Duration::from_secs(5) {
            logger::log(&format!(
                "Flux: streaming stats frames_sent={audio_frames_sent} bytes_sent={audio_bytes_sent} text_messages={text_messages_received} transcripts={transcripts_delivered}"
            ));
            last_stats_log = Instant::now();
        }
    }

    let mut drain = Vec::new();
    cap.stop(&mut drain);
    logger::log(&format!(
        "Flux: audio capture stopped drained_samples={} frames_sent={} bytes_sent={} text_messages={} transcripts={}",
        drain.len(),
        audio_frames_sent,
        audio_bytes_sent,
        text_messages_received,
        transcripts_delivered
    ));
    if let Err(error) = ws.send(tungstenite::Message::Text(
        r#"{"type":"CloseStream"}"#.to_string().into(),
    )) {
        logger::log(&format!("Flux: failed to send CloseStream: {error}"));
    } else {
        logger::log("Flux: sent CloseStream");
    }

    let deadline = Instant::now() + Duration::from_millis(500);
    while Instant::now() < deadline {
        match ws.read() {
            Ok(tungstenite::Message::Text(text)) => {
                text_messages_received += 1;
                log_text_message(&text);
                if let Some(transcript) = parse_transcript(&text) {
                    transcripts_delivered += 1;
                    logger::log(&format!("Flux transcript: {transcript}"));
                    on_transcript(format!("{transcript} "));
                }
            }
            Ok(tungstenite::Message::Close(frame)) => {
                log_close_frame(frame);
                break;
            }
            Err(tungstenite::Error::Io(ref error))
                if error.kind() == std::io::ErrorKind::WouldBlock
                    || error.kind() == std::io::ErrorKind::TimedOut => {}
            Err(error) => {
                logger::log(&format!("Flux: final drain read error: {error}"));
                break;
            }
            _ => {}
        }
    }

    let _ = ws.close(None);
    beep::play_end();
    logger::log(&format!(
        "Flux: stream finished text_messages={} transcripts={}",
        text_messages_received, transcripts_delivered
    ));
}

fn build_flux_url(model: &str, language_hint: Option<&str>, key_terms: &[String]) -> String {
    let model = deepgram::normalize_streaming_model(model).unwrap_or("flux-general-en");
    let mut params = vec![
        format!("model={}", url_encode(model)),
        "encoding=linear16".to_string(),
        "sample_rate=48000".to_string(),
    ];

    if model == "flux-general-multi" {
        if let Ok(Some(language_hint)) =
            deepgram::normalize_streaming_language(model, language_hint)
        {
            params.push(format!("language_hint={}", url_encode(&language_hint)));
        }
    }

    for term in key_terms {
        if !term.trim().is_empty() {
            params.push(format!("keyterm={}", url_encode(term.trim())));
        }
    }

    format!("wss://api.deepgram.com/v2/listen?{}", params.join("&"))
}

fn log_query_string(context: &str, url: &str) {
    if let Some((_, query)) = url.split_once('?') {
        logger::verbose(&format!("{context} query: {query}"));
    }
}

fn url_encode(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char)
            }
            b' ' => encoded.push_str("%20"),
            _ => encoded.push_str(&format!("%{:02X}", byte)),
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flux_url_uses_v2_endpoint() {
        let url = build_flux_url("flux-general-en", None, &[]);

        assert!(url.starts_with("wss://api.deepgram.com/v2/listen?"));
        assert!(url.contains("model=flux-general-en"));
        assert!(!url.contains("language_hint="));
    }

    #[test]
    fn flux_multi_url_includes_language_hint_and_keyterms() {
        let url = build_flux_url(
            "flux-general-multi",
            Some("pt-BR"),
            &["Velocity".into(), "Deepgram Flux".into()],
        );

        assert!(url.contains("model=flux-general-multi"));
        assert!(url.contains("language_hint=pt"));
        assert!(url.contains("keyterm=Velocity"));
        assert!(url.contains("keyterm=Deepgram%20Flux"));
    }

    #[test]
    fn flux_parser_reads_end_of_turn_transcripts() {
        let text = r#"{
            "type": "TurnInfo",
            "event": "EndOfTurn",
            "transcript": "Hello from Flux"
        }"#;

        assert_eq!(parse_transcript(text), Some("Hello from Flux".to_string()));
    }

    #[test]
    fn flux_parser_ignores_partial_updates() {
        let text = r#"{
            "type": "TurnInfo",
            "event": "Update",
            "transcript": "partial"
        }"#;

        assert_eq!(parse_transcript(text), None);
    }
}
