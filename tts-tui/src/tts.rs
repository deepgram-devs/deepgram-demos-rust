use anyhow::{Result, anyhow, Context};
use std::path::{Path, PathBuf};
use sha2::{Sha256, Digest};
use reqwest::Client;
use tokio::io::AsyncWriteExt;
use rodio::{Decoder, OutputStream, Sink, Source};
use rodio::buffer::SamplesBuffer;
use std::io::Cursor;
use rust_decimal::Decimal;
use std::sync::Arc;
use std::time::Duration;
use claxon;

pub fn get_deepgram_api_key() -> Result<String> {
    dotenvy::dotenv().ok();
    std::env::var("DEEPGRAM_API_KEY")
        .map_err(|_| anyhow!("DEEPGRAM_API_KEY not found in .env or environment variables"))
}

pub async fn fetch_audio_for_playback(
    api_key: &str,
    text: &str,
    voice_id: &str,
    speed: Decimal,
    sample_rate: u32,
    encoding: &str,
    extension: &str,
    cache_dir: &str,
    endpoint: &str,
) -> Result<(String, Vec<u8>, bool)> {
    let cache_file_path = get_cache_file_path(cache_dir, text, voice_id, speed, sample_rate, encoding, extension)?;
    let message;
    let audio_data;
    let is_cached;

    let short_name = cache_file_path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| {
            let chars: Vec<char> = n.chars().collect();
            if chars.len() > 12 {
                format!("…{}", &chars[chars.len() - 12..].iter().collect::<String>())
            } else {
                n.to_string()
            }
        })
        .unwrap_or_else(|| "?".to_string());

    if cache_file_path.exists() {
        message = format!("Playing from cache: {}", short_name);
        audio_data = tokio::fs::read(&cache_file_path).await?;
        is_cached = true;
    } else {
        message = format!("Fetching from Deepgram, caching as: {}", short_name);
        audio_data = fetch_deepgram_tts(api_key, text, voice_id, speed, sample_rate, encoding, endpoint).await?;
        save_audio_to_cache(&cache_file_path, &audio_data).await?;
        is_cached = false;
    }

    Ok((message, audio_data, is_cached))
}

pub fn play_audio_data_sync(data: &[u8], encoding: &str, sample_rate: u32) -> Result<(Arc<Sink>, Arc<OutputStream>, u64)> {
    let (stream, stream_handle) = OutputStream::try_default()?;
    let sink = Sink::try_new(&stream_handle)?;

    // mulaw and alaw are returned as raw bytes by the Deepgram API with no container,
    // so rodio's Decoder cannot recognize them. Decode to linear PCM manually.
    let duration_ms = match encoding {
        "mulaw" => {
            let samples: Vec<i16> = data.iter().map(|&b| decode_ulaw(b)).collect();
            let duration_ms = data.len() as u64 * 1000 / sample_rate as u64;
            sink.append(SamplesBuffer::new(1, sample_rate, samples));
            duration_ms
        }
        "alaw" => {
            let samples: Vec<i16> = data.iter().map(|&b| decode_alaw(b)).collect();
            let duration_ms = data.len() as u64 * 1000 / sample_rate as u64;
            sink.append(SamplesBuffer::new(1, sample_rate, samples));
            duration_ms
        }
        _ => {
            // All other formats are in a recognised container (MP3, WAV, FLAC, AAC).
            let source = Decoder::new(Cursor::new(data.to_vec()))?;
            let duration_ms = source.total_duration()
                .map(|d| d.as_millis() as u64)
                .unwrap_or_else(|| audio_duration_ms(data, encoding, sample_rate));
            sink.append(source);
            duration_ms
        }
    };

    Ok((Arc::new(sink), Arc::new(stream), duration_ms))
}

/// Decode a single G.711 μ-law byte to a signed 16-bit linear PCM sample.
fn decode_ulaw(byte: u8) -> i16 {
    let byte = !byte;
    let t = (((byte & 0x0F) as i32) << 3) + 0x84;
    let t = t << ((byte & 0x70) >> 4);
    if byte & 0x80 != 0 { (0x84 - t) as i16 } else { (t - 0x84) as i16 }
}

/// Decode a single G.711 A-law byte to a signed 16-bit linear PCM sample.
fn decode_alaw(byte: u8) -> i16 {
    let byte = byte ^ 0x55;
    let mut t = ((byte & 0x0F) as i32) << 1;
    t += 1;
    let exponent = (byte & 0x70) >> 4;
    if exponent != 0 {
        t += 0x20;
        t <<= exponent - 1;
    }
    if byte & 0x80 != 0 { t as i16 } else { -(t as i16) }
}

/// Calculate audio duration in milliseconds based on the encoding and raw byte length.
fn audio_duration_ms(data: &[u8], encoding: &str, sample_rate: u32) -> u64 {
    match encoding {
        "mp3" => {
            // mp3_duration handles both CBR and VBR accurately
            mp3_duration::from_read(&mut Cursor::new(data))
                .map(|d: Duration| d.as_millis() as u64)
                .unwrap_or_else(|_| bitrate_estimate_ms(data, 128))
        }
        "linear16" => {
            // 16-bit PCM mono in a WAV container; subtract the 44-byte header
            let pcm_bytes = data.len().saturating_sub(44) as u64;
            // 2 bytes per sample, 1 channel
            pcm_bytes * 1000 / (sample_rate as u64 * 2)
        }
        "mulaw" | "alaw" => {
            // Raw 8-bit encoded audio: 1 byte per sample, mono
            data.len() as u64 * 1000 / sample_rate as u64
        }
        "flac" => flac_duration_ms(data).unwrap_or_else(|| bitrate_estimate_ms(data, 400)),
        "aac" => bitrate_estimate_ms(data, 128),
        _ => bitrate_estimate_ms(data, 128),
    }
}

/// Get the exact FLAC duration using claxon.
///
/// Prefers the STREAMINFO `total_samples` field (O(1)).  When the encoder set
/// it to 0 (allowed for streaming FLAC), falls back to summing every audio
/// block's sample count — still exact, just O(n blocks).
fn flac_duration_ms(data: &[u8]) -> Option<u64> {
    let mut reader = claxon::FlacReader::new(Cursor::new(data)).ok()?;
    let sample_rate = reader.streaminfo().sample_rate as u64;
    if sample_rate == 0 {
        return None;
    }

    // Fast path: total_samples is encoded in STREAMINFO.
    if let Some(samples) = reader.streaminfo().samples {
        if samples > 0 {
            return Some(samples * 1000 / sample_rate);
        }
    }

    // Slow path: STREAMINFO has total_samples = 0 (streaming FLAC).
    // Count samples by reading every block header without fully decoding PCM.
    let mut total_samples = 0u64;
    while let Ok(Some(block)) = reader.blocks().read_next_or_eof(Vec::new()) {
        total_samples += block.duration() as u64;
    }
    if total_samples > 0 {
        Some(total_samples * 1000 / sample_rate)
    } else {
        None
    }
}

/// Rough duration estimate: bits / kbps → milliseconds.
fn bitrate_estimate_ms(data: &[u8], kbps: u64) -> u64 {
    (data.len() as u64 * 8) / kbps
}

fn get_cache_file_path(cache_dir: &str, text: &str, voice_id: &str, speed: Decimal, sample_rate: u32, encoding: &str, extension: &str) -> Result<PathBuf> {
    let mut hasher = Sha256::new();
    hasher.update(text);
    hasher.update(voice_id);
    hasher.update(speed.to_string().as_bytes());
    hasher.update(sample_rate.to_string().as_bytes());
    hasher.update(encoding.as_bytes());
    let hash = hasher.finalize();
    let filename = format!("{:x}.{}", hash, extension);
    let path = PathBuf::from(cache_dir).join(filename);
    Ok(path)
}

async fn fetch_deepgram_tts(api_key: &str, text: &str, voice_id: &str, speed: Decimal, sample_rate: u32, encoding: &str, endpoint: &str) -> Result<Vec<u8>> {
    let client = Client::new();

    // Build URL — omit parameters that match API defaults (speed 1.0, sample_rate = format default)
    let mut params = format!("model={}&encoding={}", voice_id, encoding);
    if speed != Decimal::new(10, 1) {
        params.push_str(&format!("&speed={}", speed));
    }
    // MP3 and AAC have fixed sample rates; omit the parameter for those encodings
    if encoding != "mp3" && encoding != "aac" {
        params.push_str(&format!("&sample_rate={}", sample_rate));
    }
    let url = format!("{}?{}", endpoint, params);

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

