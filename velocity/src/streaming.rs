use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

use crate::{audio, beep, logger};

// ── Deepgram streaming response types ────────────────────────────────────────

#[derive(serde::Deserialize)]
struct StreamMsg {
    #[serde(rename = "type")]
    msg_type: String,
    channel: Option<StreamChannel>,
    is_final: Option<bool>,
}

#[derive(serde::Deserialize)]
struct StreamChannel {
    alternatives: Vec<StreamAlt>,
}

#[derive(serde::Deserialize)]
struct StreamAlt {
    transcript: String,
}

fn parse_transcript(text: &str) -> Option<String> {
    let msg: StreamMsg = serde_json::from_str(text).ok()?;
    if msg.msg_type == "Results" && msg.is_final == Some(true) {
        msg.channel?
            .alternatives
            .into_iter()
            .next()
            .map(|a| a.transcript)
            .filter(|t| !t.is_empty())
    } else {
        None
    }
}

// ── Session entry point ───────────────────────────────────────────────────────

/// Opens a Deepgram streaming WebSocket, captures audio, types transcripts,
/// and runs until `active` is set to false or the user changes focus.
///
/// `capture` is the shared audio device — the same one used by the regular
/// recording mode. Recording and streaming are mutually exclusive so there is
/// no contention, but sharing a single `waveInOpen` handle avoids the
/// MMSYSERR_ALLOCATED error that occurs when a second open is attempted.
pub fn run(
    api_key: &str,
    smart_format: bool,
    model: &str,
    key_terms: &[String],
    selected_device_name: Option<&str>,
    active: Arc<AtomicBool>,
    on_transcript: Arc<dyn Fn(String) + Send + Sync>,
) {
    let url = build_streaming_url(model, smart_format, key_terms);
    log_query_string("Deepgram streaming", &url);

    // Build the request via IntoClientRequest so tungstenite populates all
    // required WebSocket handshake headers (Sec-WebSocket-Key, Upgrade, etc.)
    // before we add our custom Authorization header on top.
    use tungstenite::client::IntoClientRequest;
    let mut req = match url.into_client_request() {
        Ok(r) => r,
        Err(e) => {
            logger::verbose(&format!("Streaming: bad request: {e}"));
            active.store(false, Ordering::Relaxed);
            return;
        }
    };
    match format!("Token {}", api_key).parse() {
        Ok(v) => { req.headers_mut().insert("Authorization", v); }
        Err(e) => {
            logger::verbose(&format!("Streaming: bad auth header: {e}"));
            active.store(false, Ordering::Relaxed);
            return;
        }
    }

    let (mut ws, _) = match tungstenite::connect(req) {
        Ok(c) => c,
        Err(e) => {
            logger::verbose(&format!("Streaming: connect error: {e}"));
            active.store(false, Ordering::Relaxed);
            return;
        }
    };

    // Set a short read timeout so the event loop stays responsive while we
    // also need to send audio and KeepAlive messages.
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

    let mut cap = match audio::AudioCapture::new(selected_device_name) {
        Some(capture) => capture,
        None => {
            logger::verbose("Streaming: could not open selected audio capture device");
            active.store(false, Ordering::Relaxed);
            let _ = ws.close(None);
            return;
        }
    };
    cap.start();
    beep::play_start();

    let initial_window = unsafe { GetForegroundWindow() };
    let mut last_keepalive = Instant::now();
    let mut pcm_buf: Vec<i16> = Vec::new();

    loop {
        if !active.load(Ordering::Relaxed) {
            break;
        }

        // Stop if the user switched to a different window.
        let current_window = unsafe { GetForegroundWindow() };
        if current_window.0 != std::ptr::null_mut() && current_window != initial_window {
            logger::verbose("Streaming: focus changed, stopping");
            active.store(false, Ordering::Relaxed);
            break;
        }

        // Collect audio that is ready and send it as a binary frame.
        pcm_buf.clear();
        cap.collect_ready(&mut pcm_buf);
        if !pcm_buf.is_empty() {
            let bytes: Vec<u8> = pcm_buf.iter().flat_map(|s| s.to_le_bytes()).collect();
            if let Err(e) = ws.send(tungstenite::Message::Binary(bytes.into())) {
                logger::verbose(&format!("Streaming: send error: {e}"));
                active.store(false, Ordering::Relaxed);
                break;
            }
        }

        // Send a KeepAlive message every 5 seconds.
        if last_keepalive.elapsed() >= Duration::from_secs(5) {
            if let Err(e) = ws.send(tungstenite::Message::Text(
                r#"{"type":"KeepAlive"}"#.to_string().into(),
            )) {
                logger::verbose(&format!("Streaming: keepalive error: {e}"));
                active.store(false, Ordering::Relaxed);
                break;
            }
            last_keepalive = Instant::now();
        }

        // Try to read a message from Deepgram (returns quickly on timeout).
        match ws.read() {
            Ok(tungstenite::Message::Text(txt)) => {
                if let Some(transcript) = parse_transcript(&txt) {
                    logger::verbose(&format!("Streaming transcript: {transcript}"));
                    on_transcript(format!("{transcript} "));
                }
            }
            Ok(tungstenite::Message::Close(_)) => {
                logger::verbose("Streaming: server closed WebSocket");
                active.store(false, Ordering::Relaxed);
                break;
            }
            Err(tungstenite::Error::Io(ref e))
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                // No message yet — continue the loop.
            }
            Err(e) => {
                logger::verbose(&format!("Streaming: read error: {e}"));
                active.store(false, Ordering::Relaxed);
                break;
            }
            _ => {}
        }
    }

    // Graceful shutdown: stop audio capture, then close the WebSocket.
    let mut drain = Vec::new();
    cap.stop(&mut drain);
    let _ = ws.send(tungstenite::Message::Text(
        r#"{"type":"CloseStream"}"#.to_string().into(),
    ));
    std::thread::sleep(Duration::from_millis(300));
    let _ = ws.close(None);

    beep::play_end();
}

fn build_streaming_url(model: &str, smart_format: bool, key_terms: &[String]) -> String {
    let mut params = vec![
        format!("model={}", url_encode(model)),
        "encoding=linear16".to_string(),
        "sample_rate=48000".to_string(),
        "channels=1".to_string(),
    ];
    let key_term_param = deepgram_key_term_param(model);
    if smart_format {
        params.push("smart_format=true".to_string());
    }
    for term in key_terms {
        if !term.trim().is_empty() {
            params.push(format!("{key_term_param}={}", url_encode(term.trim())));
        }
    }

    format!("wss://api.deepgram.com/v1/listen?{}", params.join("&"))
}

fn deepgram_key_term_param(model: &str) -> &'static str {
    match model.trim().to_ascii_lowercase().as_str() {
        "nova-2" => "keyword",
        _ => "keyterm",
    }
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
    fn streaming_url_includes_keywords() {
        let url = build_streaming_url("nova-3", true, &["alpha".into(), "beta test".into()]);
        assert!(url.contains("keyterm=alpha"));
        assert!(url.contains("keyterm=beta%20test"));
        assert!(url.contains("smart_format=true"));
    }

    #[test]
    fn streaming_url_uses_keyword_for_nova_2() {
        let url = build_streaming_url("nova-2", false, &["alpha".into()]);
        assert!(url.contains("keyword=alpha"));
        assert!(!url.contains("keyterm=alpha"));
    }
}
