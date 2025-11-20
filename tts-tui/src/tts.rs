use anyhow::{Result, anyhow};
use std::path::{Path, PathBuf};
use sha2::{Sha256, Digest};
use reqwest::Client;
use tokio::io::AsyncWriteExt;
use rodio::{Decoder, OutputStream, Sink};
use std::io::Cursor;

const DEEPGRAM_API_URL: &str = "https://api.deepgram.com/v1/speak";

pub fn get_deepgram_api_key() -> Result<String> {
    dotenvy::dotenv().ok();
    std::env::var("DEEPGRAM_API_KEY")
        .map_err(|_| anyhow!("DEEPGRAM_API_KEY not found in .env or environment variables"))
}

pub async fn play_text_with_deepgram(
    api_key: &str,
    text: &str,
    voice_id: &str,
    cache_dir: &str,
) -> Result<String> {
    let cache_file_path = get_cache_file_path(cache_dir, text, voice_id)?;
    let message;

    if cache_file_path.exists() {
        message = format!("Playing from cache: {}", cache_file_path.display());
        play_audio_from_file(&cache_file_path).await?;
    } else {
        message = format!("Fetching from Deepgram and caching: {}", cache_file_path.display());
        let audio_data = fetch_deepgram_tts(api_key, text, voice_id).await?;
        save_audio_to_cache(&cache_file_path, &audio_data).await?;
        play_audio_from_data(&audio_data).await?;
    }

    Ok(message)
}

fn get_cache_file_path(cache_dir: &str, text: &str, voice_id: &str) -> Result<PathBuf> {
    let mut hasher = Sha256::new();
    hasher.update(text);
    hasher.update(voice_id);
    let hash = hasher.finalize();
    let filename = format!("{:x}.mp3", hash);
    let path = PathBuf::from(cache_dir).join(filename);
    Ok(path)
}

async fn fetch_deepgram_tts(api_key: &str, text: &str, voice_id: &str) -> Result<Vec<u8>> {
    let client = Client::new();
    let url = format!("{}?model={}&encoding=mp3", DEEPGRAM_API_URL, voice_id);

    let res = client
        .post(&url)
        .header("Authorization", format!("Token {}", api_key))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({"text": text}))
        .send()
        .await?
        .error_for_status()?;

    let audio_data = res.bytes().await?.to_vec();
    Ok(audio_data)
}

async fn save_audio_to_cache(path: &Path, data: &[u8]) -> Result<()> {
    let mut file = tokio::fs::File::create(path).await?;
    file.write_all(data).await?;
    Ok(())
}

async fn play_audio_from_file(path: &Path) -> Result<()> {
    let file = std::fs::File::open(path)?;
    play_audio_stream(file).await
}

async fn play_audio_from_data(data: &[u8]) -> Result<()> {
    let cursor = Cursor::new(data.to_vec());
    play_audio_stream(cursor).await
}

async fn play_audio_stream<R: std::io::Read + std::io::Seek + Send + 'static + Sync>(reader: R) -> Result<()> {
    let (_stream, stream_handle) = OutputStream::try_default()?;
    let sink = Sink::try_new(&stream_handle)?;

    let source = Decoder::new(reader)?;
    sink.append(source);
    sink.sleep_until_end();
    Ok(())
}
