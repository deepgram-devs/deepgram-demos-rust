use anyhow::Result;
use reqwest;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
struct GeminiRequest {
    contents: Vec<Content>,
}

#[derive(Debug, Serialize)]
struct Content {
    parts: Vec<Part>,
}

#[derive(Debug, Serialize)]
struct Part {
    text: String,
}

#[derive(Debug, Deserialize)]
struct GeminiResponse {
    candidates: Vec<Candidate>,
}

#[derive(Debug, Deserialize)]
struct Candidate {
    content: ContentResponse,
}

#[derive(Debug, Deserialize)]
struct ContentResponse {
    parts: Vec<PartResponse>,
}

#[derive(Debug, Deserialize)]
struct PartResponse {
    text: String,
}

pub async fn generate_podcast_script(topic: &str, speaker_count: usize) -> Result<String> {
    let api_key = std::env::var("GEMINI_API_KEY")
        .map_err(|_| anyhow::anyhow!("GEMINI_API_KEY environment variable not set"))?;

    let prompt_text = format!(
        "Generate a podcast script that has {} distinct speakers. \
        Only output the script itself with the speaker ID prefix before each utterance. \
        Do not output anything else except the podcast script. \
        The podcast should be approximately three minutes long. \
        Each speaker should have approximately equal time spent speaking. \
        The script should be conversational in nature, so that it sounds like the podcast speakers \
        are being personal and sharing ideas with each other. \
        Format each line as 'Speaker N: [text]' where N is the speaker number (1-{}). \
        The topic of the podcast is: {}.",
        speaker_count,
        speaker_count,
        topic
    );

    let client = reqwest::Client::new();
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-3-flash-preview:generateContent?key={}",
        api_key
    );

    let request_body = GeminiRequest {
        contents: vec![Content {
            parts: vec![Part {
                text: prompt_text,
            }],
        }],
    };

    let response = client
        .post(&url)
        .json(&request_body)
        .send()
        .await?;

    if !response.status().is_success() {
        let error_text = response.text().await?;
        anyhow::bail!("Gemini API error: {}", error_text);
    }

    let gemini_response: GeminiResponse = response.json().await?;

    let script = gemini_response
        .candidates
        .get(0)
        .and_then(|c| c.content.parts.get(0))
        .map(|p| p.text.clone())
        .ok_or_else(|| anyhow::anyhow!("No response text from Gemini"))?;

    Ok(script)
}
