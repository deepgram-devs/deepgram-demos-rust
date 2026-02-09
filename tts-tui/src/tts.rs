use anyhow::{Result, anyhow, Context};
use std::path::{Path, PathBuf};
use sha2::{Sha256, Digest};
use reqwest::Client;
use tokio::io::AsyncWriteExt;
use rodio::{Decoder, OutputStream, Sink};
use std::io::Cursor;
use rust_decimal::Decimal;
use std::sync::Arc;

pub fn get_deepgram_api_key() -> Result<String> {
    dotenvy::dotenv().ok();
    std::env::var("DEEPGRAM_API_KEY")
        .map_err(|_| anyhow!("DEEPGRAM_API_KEY not found in .env or environment variables"))
}

pub async fn play_text_with_deepgram(
    api_key: &str,
    text: &str,
    voice_id: &str,
    speed: Decimal,
    cache_dir: &str,
    endpoint: &str,
) -> Result<(String, Arc<Sink>, Arc<OutputStream>)> {
    let cache_file_path = get_cache_file_path(cache_dir, text, voice_id, speed)?;
    let message;
    let (sink, stream);

    if cache_file_path.exists() {
        message = format!("Playing from cache: {}", cache_file_path.display());
        (sink, stream) = play_audio_from_file(&cache_file_path).await?;
    } else {
        message = format!("Fetching from Deepgram and caching: {}", cache_file_path.display());
        let audio_data = fetch_deepgram_tts(api_key, text, voice_id, speed, endpoint).await?;
        save_audio_to_cache(&cache_file_path, &audio_data).await?;
        (sink, stream) = play_audio_from_data(&audio_data).await?;
    }

    Ok((message, sink, stream))
}

fn get_cache_file_path(cache_dir: &str, text: &str, voice_id: &str, speed: Decimal) -> Result<PathBuf> {
    let mut hasher = Sha256::new();
    hasher.update(text);
    hasher.update(voice_id);
    hasher.update(speed.to_string().as_bytes());
    let hash = hasher.finalize();
    let filename = format!("{:x}.mp3", hash);
    let path = PathBuf::from(cache_dir).join(filename);
    Ok(path)
}

async fn fetch_deepgram_tts(api_key: &str, text: &str, voice_id: &str, speed: Decimal, endpoint: &str) -> Result<Vec<u8>> {
    let client = Client::new();

    // Only include speed parameter if it's not 1.0 (default)
    let url = if speed == Decimal::new(10, 1) { // 1.0
        format!("{}?model={}&encoding=mp3", endpoint, voice_id)
    } else {
        format!("{}?model={}&encoding=mp3&speed={}", endpoint, voice_id, speed)
    };

    let res = client
        .post(&url)
        .header("Authorization", format!("Token {}", api_key))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({"text": text}))
        .send()
        .await
        .context(format!("Failed to send request to Deepgram API at {}", url))?;

    // Check status and capture detailed error message if request failed
    if !res.status().is_success() {
        let status = res.status();
        let error_body = res.text().await.unwrap_or_else(|_| "Unable to read error response".to_string());
        return Err(anyhow!(
            "HTTP {} - Deepgram API error: {}",
            status,
            error_body
        ));
    }

    let audio_data = res.bytes().await
        .context("Failed to read audio data from response")?
        .to_vec();
    Ok(audio_data)
}

async fn save_audio_to_cache(path: &Path, data: &[u8]) -> Result<()> {
    let mut file = tokio::fs::File::create(path).await?;
    file.write_all(data).await?;
    Ok(())
}

async fn play_audio_from_file(path: &Path) -> Result<(Arc<Sink>, Arc<OutputStream>)> {
    let file = std::fs::File::open(path)?;
    play_audio_stream(file).await
}

async fn play_audio_from_data(data: &[u8]) -> Result<(Arc<Sink>, Arc<OutputStream>)> {
    let cursor = Cursor::new(data.to_vec());
    play_audio_stream(cursor).await
}

async fn play_audio_stream<R: std::io::Read + std::io::Seek + Send + 'static + Sync>(reader: R) -> Result<(Arc<Sink>, Arc<OutputStream>)> {
    let (stream, stream_handle) = OutputStream::try_default()?;
    let sink = Sink::try_new(&stream_handle)?;

    let source = Decoder::new(reader)?;
    sink.append(source);

    // Return sink and OutputStream without blocking
    // CRITICAL: Both must be kept alive for audio to play!
    // The caller is responsible for storing these and checking when playback finishes
    Ok((Arc::new(sink), Arc::new(stream)))
}
