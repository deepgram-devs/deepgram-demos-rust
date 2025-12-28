use anyhow::Result;
use reqwest;
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;

pub async fn generate_audio_clips(
    utterances: &[crate::Utterance],
    voice_map: &HashMap<usize, String>,
) -> Result<Vec<PathBuf>> {
    let api_key = std::env::var("DEEPGRAM_API_KEY")
        .map_err(|_| anyhow::anyhow!("DEEPGRAM_API_KEY environment variable not set"))?;

    let client = reqwest::Client::new();
    let mut audio_files = Vec::new();

    std::fs::create_dir_all("temp_audio")?;

    for (i, utterance) in utterances.iter().enumerate() {
        let voice = voice_map.get(&utterance.speaker_id)
            .ok_or_else(|| anyhow::anyhow!("No voice assigned for speaker {}", utterance.speaker_id))?;

        let url = format!(
            "https://api.deepgram.com/v1/speak?model={}&encoding=linear16&sample_rate=48000",
            voice
        );

        let request_body = json!({
            "text": utterance.text
        });

        let response = client
            .post(&url)
            .header("Authorization", format!("Token {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            anyhow::bail!("Deepgram API error: {}", error_text);
        }

        let audio_data = response.bytes().await?;

        let file_path = PathBuf::from(format!("temp_audio/clip_{:04}.raw", i));
        std::fs::write(&file_path, audio_data)?;

        audio_files.push(file_path);
    }

    Ok(audio_files)
}
