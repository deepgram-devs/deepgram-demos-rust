use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use tungstenite::{Message, WebSocket};

use crate::{config, deepgram, logger, state};

const LISTEN_ADDR: &str = "0.0.0.0";
const READ_TIMEOUT: Duration = Duration::from_millis(20);
const IDLE_SLEEP: Duration = Duration::from_millis(100);
const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(5);
const SAMPLE_RATE: u32 = 16_000;

#[derive(serde::Deserialize)]
struct DeepgramResultMsg {
    #[serde(rename = "type")]
    msg_type: String,
    channel: Option<DeepgramChannel>,
    is_final: Option<bool>,
    event: Option<String>,
    transcript: Option<String>,
    #[serde(default)]
    words: Vec<DeepgramWord>,
}

#[derive(serde::Deserialize)]
struct DeepgramChannel {
    alternatives: Vec<DeepgramAlternative>,
}

#[derive(serde::Deserialize)]
struct DeepgramAlternative {
    transcript: String,
}

#[derive(serde::Deserialize)]
struct DeepgramWord {
    word: String,
}

pub fn spawn_server(app: Arc<state::AppState>) {
    std::thread::spawn(move || run_server(app));
}

fn run_server(app: Arc<state::AppState>) {
    let active_client = Arc::new(AtomicBool::new(false));
    let mut listener: Option<(u16, TcpListener)> = None;
    let mut last_bind_error: Option<String> = None;

    loop {
        let cfg = app.config();
        if !cfg.remote_audio_enabled {
            listener = None;
            std::thread::sleep(Duration::from_secs(1));
            continue;
        }

        if listener
            .as_ref()
            .is_none_or(|(port, _)| *port != cfg.remote_audio_port)
        {
            listener = match bind_listener(cfg.remote_audio_port) {
                Ok(bound) => {
                    last_bind_error = None;
                    logger::log(&format!(
                        "Remote audio: listening on {LISTEN_ADDR}:{}",
                        cfg.remote_audio_port
                    ));
                    Some((cfg.remote_audio_port, bound))
                }
                Err(error) => {
                    let message = format!(
                        "Remote audio: failed to listen on port {}: {error}",
                        cfg.remote_audio_port
                    );
                    if last_bind_error.as_deref() != Some(&message) {
                        logger::log(&message);
                        app.set_error(message.clone());
                        last_bind_error = Some(message);
                    }
                    std::thread::sleep(Duration::from_secs(2));
                    continue;
                }
            };
        }

        let Some((_, listener_ref)) = listener.as_ref() else {
            std::thread::sleep(IDLE_SLEEP);
            continue;
        };

        match listener_ref.accept() {
            Ok((stream, addr)) => {
                if active_client.swap(true, Ordering::Relaxed) {
                    logger::log(&format!(
                        "Remote audio: rejected {addr}; another mobile client is active"
                    ));
                    send_busy_close(stream);
                    continue;
                }

                logger::log(&format!("Remote audio: accepted mobile client {addr}"));
                let session_app = Arc::clone(&app);
                let session_active = Arc::clone(&active_client);
                std::thread::spawn(move || {
                    handle_client(stream, session_app);
                    session_active.store(false, Ordering::Relaxed);
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(IDLE_SLEEP);
            }
            Err(error) => {
                logger::log(&format!("Remote audio: accept error: {error}"));
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    }
}

fn bind_listener(port: u16) -> Result<TcpListener, String> {
    let listener = TcpListener::bind((LISTEN_ADDR, port)).map_err(|error| error.to_string())?;
    listener
        .set_nonblocking(true)
        .map_err(|error| error.to_string())?;
    Ok(listener)
}

fn send_busy_close(stream: TcpStream) {
    if let Ok(mut ws) = tungstenite::accept(stream) {
        let _ = ws.send(Message::Text(
            r#"{"type":"error","message":"Velocity already has an active remote audio client"}"#
                .to_string()
                .into(),
        ));
        let _ = ws.close(None);
    }
}

fn handle_client(stream: TcpStream, app: Arc<state::AppState>) {
    let cfg = app.config();
    if !cfg.remote_audio_enabled {
        return;
    }

    let api_key = match cfg.api_key.clone().filter(|key| !key.trim().is_empty()) {
        Some(key) => key,
        None => {
            app.set_error("Remote audio requires a Deepgram API key".to_string());
            return;
        }
    };

    let mut mobile_ws = match tungstenite::accept(stream) {
        Ok(ws) => ws,
        Err(error) => {
            logger::log(&format!("Remote audio: client handshake failed: {error}"));
            return;
        }
    };
    set_ws_read_timeout(&mut mobile_ws, READ_TIMEOUT);

    let mut deepgram_ws = match connect_deepgram(&cfg, &api_key) {
        Ok(ws) => ws,
        Err(error) => {
            logger::log(&format!("Remote audio: Deepgram connect failed: {error}"));
            let _ = mobile_ws.send(Message::Text(
                format!(r#"{{"type":"error","message":"Deepgram connect failed: {error}"}}"#)
                    .into(),
            ));
            let _ = mobile_ws.close(None);
            return;
        }
    };
    set_deepgram_read_timeout(&mut deepgram_ws, READ_TIMEOUT);

    app.capture_transcript_target();
    let _ = mobile_ws.send(Message::Text(
        format!(
            r#"{{"type":"ready","encoding":"linear16","sample_rate":{SAMPLE_RATE},"channels":1}}"#
        )
        .into(),
    ));

    let mut last_keepalive = Instant::now();
    loop {
        if !app.config().remote_audio_enabled {
            logger::log("Remote audio: disabled while client was connected");
            break;
        }

        match mobile_ws.read() {
            Ok(Message::Binary(bytes)) => {
                update_meter(&app, &bytes);
                if let Err(error) = deepgram_ws.send(Message::Binary(bytes)) {
                    logger::log(&format!("Remote audio: send to Deepgram failed: {error}"));
                    break;
                }
            }
            Ok(Message::Text(text)) => {
                logger::verbose(&format!("Remote audio client message: {text}"));
            }
            Ok(Message::Close(_)) => {
                logger::log("Remote audio: mobile client closed WebSocket");
                break;
            }
            Ok(Message::Ping(payload)) => {
                let _ = mobile_ws.send(Message::Pong(payload));
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(ref error))
                if error.kind() == std::io::ErrorKind::WouldBlock
                    || error.kind() == std::io::ErrorKind::TimedOut => {}
            Err(error) => {
                logger::log(&format!("Remote audio: client read failed: {error}"));
                break;
            }
        }

        if last_keepalive.elapsed() >= KEEPALIVE_INTERVAL {
            if let Err(error) =
                deepgram_ws.send(Message::Text(r#"{"type":"KeepAlive"}"#.to_string().into()))
            {
                logger::log(&format!("Remote audio: keepalive failed: {error}"));
                break;
            }
            last_keepalive = Instant::now();
        }

        match deepgram_ws.read() {
            Ok(Message::Text(text)) => {
                if let Some(transcript) = parse_transcript(&text) {
                    logger::log(&format!("Remote audio transcript: {transcript}"));
                    app.push_history_and_deliver(format!("{transcript} "));
                } else {
                    logger::verbose(&format!("Remote audio Deepgram message: {text}"));
                }
            }
            Ok(Message::Close(frame)) => {
                logger::log(&format!(
                    "Remote audio: Deepgram closed WebSocket: {frame:?}"
                ));
                break;
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(ref error))
                if error.kind() == std::io::ErrorKind::WouldBlock
                    || error.kind() == std::io::ErrorKind::TimedOut => {}
            Err(error) => {
                logger::log(&format!("Remote audio: Deepgram read failed: {error}"));
                break;
            }
        }
    }

    app.set_meter_level(0);
    let _ = deepgram_ws.send(Message::Text(
        r#"{"type":"CloseStream"}"#.to_string().into(),
    ));
    let _ = deepgram_ws.close(None);
    let _ = mobile_ws.close(None);
    logger::log("Remote audio: session ended");
}

fn connect_deepgram(
    cfg: &config::Config,
    api_key: &str,
) -> Result<WebSocket<tungstenite::stream::MaybeTlsStream<TcpStream>>, String> {
    let url = build_deepgram_url(cfg);
    use tungstenite::client::IntoClientRequest;
    let mut req = url
        .into_client_request()
        .map_err(|error| format!("bad request: {error}"))?;
    let auth = format!("Token {api_key}")
        .parse()
        .map_err(|error| format!("bad auth header: {error}"))?;
    req.headers_mut().insert("Authorization", auth);
    let (ws, response) = tungstenite::connect(req).map_err(|error| error.to_string())?;
    logger::log(&format!(
        "Remote audio: connected to Deepgram status={} model={}",
        response.status(),
        cfg.streaming_model
    ));
    Ok(ws)
}

fn build_deepgram_url(cfg: &config::Config) -> String {
    let is_flux = deepgram::is_flux_model(&cfg.streaming_model);
    let mut params = vec![
        format!("model={}", url_encode(&cfg.streaming_model)),
        "encoding=linear16".to_string(),
        format!("sample_rate={SAMPLE_RATE}"),
        "channels=1".to_string(),
    ];
    if let Some(language) = cfg
        .streaming_language
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        params.push(format!("language={}", url_encode(language)));
    }
    if cfg.smart_format && !is_flux {
        params.push("smart_format=true".to_string());
    }
    let key_term_param = if cfg.streaming_model.trim().eq_ignore_ascii_case("nova-2") {
        "keyword"
    } else {
        "keyterm"
    };
    for term in &cfg.key_terms {
        let term = term.trim();
        if !term.is_empty() {
            params.push(format!("{key_term_param}={}", url_encode(term)));
        }
    }

    let endpoint = if is_flux {
        "wss://api.deepgram.com/v2/listen"
    } else {
        "wss://api.deepgram.com/v1/listen"
    };
    format!("{endpoint}?{}", params.join("&"))
}

fn parse_transcript(text: &str) -> Option<String> {
    let msg: DeepgramResultMsg = serde_json::from_str(text).ok()?;
    if msg.msg_type == "Results" && msg.is_final == Some(true) {
        return msg
            .channel?
            .alternatives
            .into_iter()
            .next()
            .map(|alternative| alternative.transcript)
            .filter(|transcript| !transcript.trim().is_empty());
    }
    if msg.msg_type == "TurnInfo" && msg.event.as_deref() == Some("EndOfTurn") {
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
        if !transcript.trim().is_empty() {
            return Some(transcript);
        }
    }
    None
}

fn set_ws_read_timeout(ws: &mut WebSocket<TcpStream>, timeout: Duration) {
    let _ = ws.get_mut().set_read_timeout(Some(timeout));
}

fn set_deepgram_read_timeout(
    ws: &mut WebSocket<tungstenite::stream::MaybeTlsStream<TcpStream>>,
    timeout: Duration,
) {
    match ws.get_mut() {
        tungstenite::stream::MaybeTlsStream::Plain(tcp) => {
            let _ = tcp.set_read_timeout(Some(timeout));
        }
        tungstenite::stream::MaybeTlsStream::NativeTls(tls) => {
            let _ = tls.get_ref().set_read_timeout(Some(timeout));
        }
        _ => {}
    }
}

fn update_meter(app: &state::AppState, bytes: &[u8]) {
    let peak = bytes
        .chunks_exact(2)
        .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]).unsigned_abs() as u32)
        .max()
        .unwrap_or(0);
    app.set_meter_level(((peak * 100) / i16::MAX as u32).min(100) as u8);
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
    fn remote_audio_url_uses_default_streaming_model() {
        let cfg = config::Config::default();
        let url = build_deepgram_url(&cfg);
        assert!(url.starts_with("wss://api.deepgram.com/v1/listen?"));
        assert!(url.contains("encoding=linear16"));
        assert!(url.contains("sample_rate=16000"));
    }

    #[test]
    fn remote_audio_parses_flux_turn_info() {
        let transcript = parse_transcript(
            r#"{"type":"TurnInfo","event":"EndOfTurn","transcript":"hello mobile"}"#,
        );
        assert_eq!(transcript.as_deref(), Some("hello mobile"));
    }
}
