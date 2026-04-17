use serde::Deserialize;

use crate::logger;

#[derive(Deserialize)]
struct Response {
    results: Results,
}

#[derive(Deserialize)]
struct Results {
    channels: Vec<Channel>,
}

#[derive(Deserialize)]
struct Channel {
    alternatives: Vec<Alternative>,
}

#[derive(Deserialize)]
struct Alternative {
    transcript: String,
}

/// Sends WAV audio bytes to the Deepgram pre-recorded API and returns the transcript.
pub fn transcribe(
    wav_bytes: Vec<u8>,
    api_key: &str,
    smart_format: bool,
    model: &str,
    key_terms: &[String],
) -> Option<String> {
    let client = reqwest::blocking::Client::new();
    let url = build_listen_url(model, smart_format, key_terms);
    log_query_string("Deepgram listen", &url);

    let resp = client
        .post(&url)
        .header("Authorization", format!("Token {}", api_key))
        .header("Content-Type", "audio/wav")
        .body(wav_bytes)
        .send();

    match resp {
        Ok(r) => {
            let status = r.status();
            let text = r.text().unwrap_or_default();
            if !status.is_success() {
                logger::verbose(&format!("Deepgram HTTP {status}: {text}"));
            }
            match serde_json::from_str::<Response>(&text) {
                Ok(parsed) => parsed
                    .results
                    .channels
                    .into_iter()
                    .next()
                    .and_then(|c| c.alternatives.into_iter().next())
                    .map(|a| a.transcript)
                    .filter(|t| !t.is_empty()),
                Err(e) => {
                    logger::verbose(&format!(
                        "Deepgram parse error: {e}\nResponse body: {text}"
                    ));
                    None
                }
            }
        }
        Err(e) => {
            logger::verbose(&format!("Deepgram request error: {e}"));
            None
        }
    }
}

fn build_listen_url(model: &str, smart_format: bool, key_terms: &[String]) -> String {
    let mut params = vec![format!("model={}", url_encode(model))];
    let key_term_param = deepgram_key_term_param(model);
    if smart_format {
        params.push("smart_format=true".to_string());
    }
    for term in key_terms {
        if !term.trim().is_empty() {
            params.push(format!("{key_term_param}={}", url_encode(term.trim())));
        }
    }

    format!("https://api.deepgram.com/v1/listen?{}", params.join("&"))
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
    fn build_listen_url_includes_key_terms() {
        let url = build_listen_url(
            "nova-3",
            true,
            &["Deepgram".into(), "Rust SDK".into()],
        );

        assert!(url.contains("model=nova-3"));
        assert!(url.contains("smart_format=true"));
        assert!(url.contains("keyterm=Deepgram"));
        assert!(url.contains("keyterm=Rust%20SDK"));
    }

    #[test]
    fn build_listen_url_uses_keyword_for_nova_2() {
        let url = build_listen_url("nova-2", false, &["Deepgram".into()]);

        assert!(url.contains("model=nova-2"));
        assert!(url.contains("keyword=Deepgram"));
        assert!(!url.contains("keyterm=Deepgram"));
    }
}
