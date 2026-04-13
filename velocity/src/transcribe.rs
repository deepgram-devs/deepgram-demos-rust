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
pub fn transcribe(wav_bytes: Vec<u8>, api_key: &str, smart_format: bool, model: &str) -> Option<String> {
    let client = reqwest::blocking::Client::new();

    let mut url = format!("https://api.deepgram.com/v1/listen?model={}", model);
    if smart_format {
        url.push_str("&smart_format=true");
    }

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
