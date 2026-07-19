use anyhow::{anyhow, Context, Result};
use claxon;
use reqwest::{Client, Url};
use rodio::buffer::SamplesBuffer;
use rodio::{Decoder, OutputStream, Sink, Source};
use rust_decimal::Decimal;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;

#[derive(Clone, Debug)]
pub enum TtsBackend {
    Deepgram {
        api_key: Option<String>,
        endpoint: String,
    },
    SageMaker {
        endpoint_name: String,
        region: String,
    },
}

impl TtsBackend {
    fn cache_namespace(&self) -> String {
        match self {
            Self::Deepgram { endpoint, .. } => format!("deepgram:{}", endpoint),
            Self::SageMaker {
                endpoint_name,
                region,
            } => format!("sagemaker:{}:{}", region, endpoint_name),
        }
    }
}

pub async fn fetch_audio_for_playback(
    backend: &TtsBackend,
    text: &str,
    voice_id: &str,
    speed: Decimal,
    sample_rate: u32,
    encoding: &str,
    normalize_volume: bool,
    extension: &str,
    cache_dir: &str,
    force_regenerate: bool,
) -> Result<(String, Vec<u8>, bool)> {
    let cache_file_path = get_cache_file_path(
        cache_dir,
        &backend.cache_namespace(),
        text,
        voice_id,
        speed,
        sample_rate,
        encoding,
        normalize_volume,
        extension,
    )?;
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

    if !force_regenerate && cache_file_path.exists() {
        message = format!("Playing from cache: {}", short_name);
        audio_data = tokio::fs::read(&cache_file_path).await?;
        is_cached = true;
    } else {
        message = if force_regenerate {
            format!("Regenerating (bypassing cache): {}", short_name)
        } else {
            format!("Fetching from Deepgram, caching as: {}", short_name)
        };
        audio_data = match backend {
            TtsBackend::Deepgram { api_key, endpoint } => {
                fetch_deepgram_tts(
                    api_key.as_deref(),
                    text,
                    voice_id,
                    speed,
                    sample_rate,
                    encoding,
                    normalize_volume,
                    endpoint,
                )
                .await?
            }
            TtsBackend::SageMaker {
                endpoint_name,
                region,
            } => {
                crate::sagemaker::fetch_sagemaker_tts(
                    endpoint_name,
                    region,
                    text,
                    voice_id,
                    speed,
                    sample_rate,
                    encoding,
                    normalize_volume,
                )
                .await?
            }
        };
        save_audio_to_cache(&cache_file_path, &audio_data).await?;
        is_cached = false;
    }

    Ok((message, audio_data, is_cached))
}

pub fn play_audio_data_sync(
    data: &[u8],
    encoding: &str,
    sample_rate: u32,
) -> Result<(Arc<Sink>, Arc<OutputStream>, u64)> {
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
            let duration_ms = if encoding == "linear16" {
                wav_duration_ms(data)
                    .unwrap_or_else(|| audio_duration_ms(data, encoding, sample_rate))
            } else {
                source
                    .total_duration()
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or_else(|| audio_duration_ms(data, encoding, sample_rate))
            };
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
    if byte & 0x80 != 0 {
        (0x84 - t) as i16
    } else {
        (t - 0x84) as i16
    }
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
    if byte & 0x80 != 0 {
        t as i16
    } else {
        -(t as i16)
    }
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
            // Parse the RIFF/WAV structure for an exact result.
            // Fall back to the naive estimate only if parsing fails.
            wav_duration_ms(data).unwrap_or_else(|| {
                // Last-resort estimate: assume 16-bit mono and a 44-byte header.
                let pcm_bytes = data.len().saturating_sub(44) as u64;
                pcm_bytes * 1000 / (sample_rate as u64 * 2)
            })
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

/// Parse a WAV/RIFF file and compute the exact duration in milliseconds.
///
/// Walks all RIFF chunks to locate `fmt ` (for sample rate, channel count, and
/// bit depth) and `data` (for the number of audio bytes).  This correctly
/// handles WAV files that carry extra chunks such as the `fact` chunk that
/// Deepgram includes in its linear16 output, which would break any approach
/// that assumes a fixed 44-byte header offset.
fn wav_duration_ms(data: &[u8]) -> Option<u64> {
    // Minimum: "RIFF" (4) + size (4) + "WAVE" (4) + one chunk header (8)
    if data.len() < 20 {
        return None;
    }
    if &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return None;
    }

    let mut offset = 12usize;
    let mut num_channels: Option<u16> = None;
    let mut sample_rate: Option<u32> = None;
    let mut bits_per_sample: Option<u16> = None;
    let mut data_bytes: Option<u64> = None;

    while offset + 8 <= data.len() {
        let chunk_id = &data[offset..offset + 4];
        let chunk_size = u32::from_le_bytes(data[offset + 4..offset + 8].try_into().ok()?) as usize;
        offset += 8;

        if chunk_id == b"fmt " && chunk_size >= 16 && offset + 16 <= data.len() {
            // Byte layout inside the fmt chunk:
            //  0-1   AudioFormat   (1 = PCM, 0xFFFE = extensible, etc.)
            //  2-3   NumChannels
            //  4-7   SampleRate
            //  8-11  ByteRate
            // 12-13  BlockAlign
            // 14-15  BitsPerSample
            num_channels = Some(u16::from_le_bytes(
                data[offset + 2..offset + 4].try_into().ok()?,
            ));
            sample_rate = Some(u32::from_le_bytes(
                data[offset + 4..offset + 8].try_into().ok()?,
            ));
            bits_per_sample = Some(u16::from_le_bytes(
                data[offset + 14..offset + 16].try_into().ok()?,
            ));
        } else if chunk_id == b"data" {
            // Deepgram returns WAV in streaming style: the data chunk size field
            // is 0xFFFFFFFF because the length is unknown when the header is
            // written.  Clamp to the bytes that are actually present in the
            // buffer so we get the real payload size in both the streaming and
            // the normal (pre-sized) case.
            let remaining = data.len().saturating_sub(offset);
            data_bytes = Some(chunk_size.min(remaining) as u64);
        }

        // WAV chunks are padded to an even byte boundary.
        let padded = chunk_size + (chunk_size & 1);
        offset = offset.saturating_add(padded);
    }

    let channels = num_channels? as u64;
    let sr = sample_rate? as u64;
    let bps = bits_per_sample? as u64;
    let bytes = data_bytes?;

    if sr == 0 || channels == 0 || bps == 0 {
        return None;
    }

    let bytes_per_sample = (bps + 7) / 8; // ceiling division: 24-bit = 3 bytes
    let total_samples = bytes / (channels * bytes_per_sample);
    Some(total_samples * 1000 / sr)
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

fn get_cache_file_path(
    cache_dir: &str,
    backend_namespace: &str,
    text: &str,
    voice_id: &str,
    speed: Decimal,
    sample_rate: u32,
    encoding: &str,
    normalize_volume: bool,
    extension: &str,
) -> Result<PathBuf> {
    let mut hasher = Sha256::new();
    hasher.update(backend_namespace);
    hasher.update(text);
    hasher.update(voice_id);
    hasher.update(speed.to_string().as_bytes());
    hasher.update(sample_rate.to_string().as_bytes());
    hasher.update(encoding.as_bytes());
    hasher.update(normalize_volume.to_string().as_bytes());
    let hash = hasher.finalize();
    let filename = format!("{:x}.{}", hash, extension);
    let path = PathBuf::from(cache_dir).join(filename);
    Ok(path)
}

async fn fetch_deepgram_tts(
    api_key: Option<&str>,
    text: &str,
    voice_id: &str,
    speed: Decimal,
    sample_rate: u32,
    encoding: &str,
    normalize_volume: bool,
    endpoint: &str,
) -> Result<Vec<u8>> {
    let client = Client::new();
    let url = build_deepgram_tts_url(
        endpoint,
        voice_id,
        speed,
        sample_rate,
        encoding,
        normalize_volume,
    )?;

    let mut request = client
        .post(url.clone())
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({"text": text}));

    if let Some(api_key) = api_key {
        request = request.header("Authorization", format!("Token {}", api_key));
    }

    let res = request
        .send()
        .await
        .context(format!("Failed to send request to Deepgram API at {}", url))?;

    // Check status and capture detailed error message if request failed
    if !res.status().is_success() {
        let status = res.status();
        let error_body = res
            .text()
            .await
            .unwrap_or_else(|_| "Unable to read error response".to_string());
        return Err(anyhow!(
            "HTTP {} - Deepgram API error: {}",
            status,
            error_body
        ));
    }

    let audio_data = res
        .bytes()
        .await
        .context("Failed to read audio data from response")?
        .to_vec();
    Ok(audio_data)
}

fn build_deepgram_tts_url(
    endpoint: &str,
    voice_id: &str,
    speed: Decimal,
    sample_rate: u32,
    encoding: &str,
    normalize_volume: bool,
) -> Result<Url> {
    let mut url = Url::parse(endpoint)
        .with_context(|| format!("Invalid Deepgram TTS endpoint URL: {}", endpoint))?;

    match url.scheme() {
        "http" | "https" => {}
        scheme => {
            return Err(anyhow!(
                "Unsupported Deepgram TTS endpoint scheme '{}'. Use http or https.",
                scheme
            ));
        }
    }

    if url.path().is_empty() || url.path() == "/" {
        url.set_path("/v1/speak");
    }

    {
        let mut pairs = url.query_pairs_mut();
        pairs.append_pair("model", voice_id);
        pairs.append_pair("encoding", encoding);
        if speed != Decimal::new(10, 1) {
            pairs.append_pair("speed", &speed.to_string());
        }
        // MP3 and AAC have fixed sample rates; omit the parameter for those encodings.
        if encoding != "mp3" && encoding != "aac" {
            pairs.append_pair("sample_rate", &sample_rate.to_string());
        }
        if normalize_volume {
            pairs.append_pair("normalize_volume", "true");
        }
    }

    Ok(url)
}

async fn save_audio_to_cache(path: &Path, data: &[u8]) -> Result<()> {
    let mut file = tokio::fs::File::create(path).await?;
    file.write_all(data).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deepgram_url_adds_tts_path_for_host_only_https_endpoint() {
        let url = build_deepgram_tts_url(
            "https://api.eu.deepgram.com",
            "aura-2-thalia-en",
            Decimal::new(10, 1),
            22050,
            "mp3",
            false,
        )
        .unwrap();

        assert_eq!(
            url.as_str(),
            "https://api.eu.deepgram.com/v1/speak?model=aura-2-thalia-en&encoding=mp3"
        );
    }

    #[test]
    fn deepgram_url_adds_tts_path_for_host_only_http_endpoint() {
        let url = build_deepgram_tts_url(
            "http://localhost:8080/",
            "aura-2-thalia-en",
            Decimal::new(10, 1),
            22050,
            "mp3",
            false,
        )
        .unwrap();

        assert_eq!(
            url.as_str(),
            "http://localhost:8080/v1/speak?model=aura-2-thalia-en&encoding=mp3"
        );
    }

    #[test]
    fn deepgram_url_preserves_full_tts_endpoint_path() {
        let url = build_deepgram_tts_url(
            "https://api.deepgram.com/v1/speak",
            "aura-2-thalia-en",
            Decimal::new(12, 1),
            24000,
            "linear16",
            false,
        )
        .unwrap();

        assert_eq!(
            url.as_str(),
            "https://api.deepgram.com/v1/speak?model=aura-2-thalia-en&encoding=linear16&speed=1.2&sample_rate=24000"
        );
    }

    #[test]
    fn deepgram_url_rejects_unsupported_schemes() {
        let err = build_deepgram_tts_url(
            "ftp://api.deepgram.com",
            "aura-2-thalia-en",
            Decimal::new(10, 1),
            22050,
            "mp3",
            false,
        )
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("Unsupported Deepgram TTS endpoint scheme"));
    }

    #[test]
    fn deepgram_url_adds_volume_normalization_when_enabled() {
        let url = build_deepgram_tts_url(
            "https://api.deepgram.com/v1/speak",
            "aura-2-thalia-en",
            Decimal::new(10, 1),
            24000,
            "linear16",
            true,
        )
        .unwrap();

        assert_eq!(
            url.as_str(),
            "https://api.deepgram.com/v1/speak?model=aura-2-thalia-en&encoding=linear16&sample_rate=24000&normalize_volume=true"
        );
    }
}
