use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, Stream, StreamConfig};
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::File;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::Duration;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CODEC_TYPE_NULL, CodecType, DecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

const FAST_STREAM_RATE: f64 = 1.25;

fn file_stream_rate(fast_mode: bool) -> f64 {
    if fast_mode { FAST_STREAM_RATE } else { 1.0 }
}

pub(crate) fn codec_name(codec: CodecType) -> &'static str {
    use symphonia::core::codecs::*;
    match codec {
        CODEC_TYPE_MP3 => "MP3",
        CODEC_TYPE_MP2 => "MP2",
        CODEC_TYPE_MP1 => "MP1",
        CODEC_TYPE_AAC => "AAC",
        CODEC_TYPE_FLAC => "FLAC",
        CODEC_TYPE_VORBIS => "Vorbis",
        CODEC_TYPE_OPUS => "Opus",
        CODEC_TYPE_ALAC => "ALAC",
        CODEC_TYPE_PCM_ALAW => "PCM A-law",
        CODEC_TYPE_PCM_MULAW => "PCM μ-law",
        CODEC_TYPE_PCM_S16LE | CODEC_TYPE_PCM_S16BE | CODEC_TYPE_PCM_S24LE
        | CODEC_TYPE_PCM_S24BE | CODEC_TYPE_PCM_S32LE | CODEC_TYPE_PCM_S32BE
        | CODEC_TYPE_PCM_F32LE | CODEC_TYPE_PCM_F32BE | CODEC_TYPE_PCM_F64LE
        | CODEC_TYPE_PCM_F64BE => "PCM",
        _ => "Unknown",
    }
}

pub(crate) fn connection_prefix(connection_id: usize, connection_count: usize) -> String {
    if connection_count > 1 {
        format!("[connection {connection_id}/{connection_count}] ")
    } else {
        String::new()
    }
}

pub(crate) fn start_audio_fanout(
    connection_count: usize,
) -> (
    mpsc::UnboundedSender<Vec<u8>>,
    Vec<AudioReceiver>,
    JoinHandle<()>,
) {
    let (source_tx, mut source_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let mut output_senders = Vec::with_capacity(connection_count);
    let mut output_receivers = Vec::with_capacity(connection_count);

    for _ in 0..connection_count {
        let (tx, rx) = mpsc::unbounded_channel::<AudioEvent>();
        output_senders.push(tx);
        output_receivers.push(AudioReceiver::Unbounded(rx));
    }

    let fanout_task = tokio::spawn(async move {
        let mut output_senders = output_senders;
        while let Some(audio_data) = source_rx.recv().await {
            output_senders.retain(|tx| tx.send(AudioEvent::Data(audio_data.clone())).is_ok());
            if output_senders.is_empty() {
                break;
            }
        }
        for sender in output_senders {
            let _ = sender.send(AudioEvent::End);
        }
    });

    (source_tx, output_receivers, fanout_task)
}

pub(crate) enum AudioReceiver {
    Unbounded(mpsc::UnboundedReceiver<AudioEvent>),
    Bounded(mpsc::Receiver<AudioEvent>),
}

impl AudioReceiver {
    pub(crate) async fn recv(&mut self) -> Option<AudioEvent> {
        match self {
            Self::Unbounded(rx) => rx.recv().await,
            Self::Bounded(rx) => rx.recv().await,
        }
    }
}

pub(crate) enum AudioEvent {
    Data(Vec<u8>),
    End,
}

/// File streaming uses bounded channels end-to-end so a fast decoder cannot
/// finish far ahead of the WebSocket sender and delay the Finalize message.
pub(crate) fn start_bounded_audio_fanout(
    connection_count: usize,
    capacity: usize,
) -> (mpsc::Sender<Vec<u8>>, Vec<AudioReceiver>, JoinHandle<()>) {
    let (source_tx, mut source_rx) = mpsc::channel::<Vec<u8>>(capacity);
    let mut output_senders = Vec::with_capacity(connection_count);
    let mut output_receivers = Vec::with_capacity(connection_count);

    for _ in 0..connection_count {
        let (tx, rx) = mpsc::channel::<AudioEvent>(capacity);
        output_senders.push(tx);
        output_receivers.push(AudioReceiver::Bounded(rx));
    }

    let fanout_task = tokio::spawn(async move {
        while let Some(audio_data) = source_rx.recv().await {
            let mut active_senders = Vec::with_capacity(output_senders.len());
            for sender in output_senders {
                if sender
                    .send(AudioEvent::Data(audio_data.clone()))
                    .await
                    .is_ok()
                {
                    active_senders.push(sender);
                }
            }
            output_senders = active_senders;
            if output_senders.is_empty() {
                break;
            }
        }
        for sender in output_senders {
            let _ = sender.send(AudioEvent::End).await;
        }
    });

    (source_tx, output_receivers, fanout_task)
}

pub(crate) struct AudioCapture {
    device: Device,
    pub(crate) config: StreamConfig,
    sample_format: SampleFormat,
}

impl AudioCapture {
    pub(crate) fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or("No input device available")?;

        println!("Input device: {}", device.name()?);

        let supported_config = device.default_input_config()?;
        println!("Default input config: {:?}", supported_config);

        let sample_format = supported_config.sample_format();
        let config: StreamConfig = supported_config.into();

        Ok(AudioCapture {
            device,
            config,
            sample_format,
        })
    }

    pub(crate) fn start_capture(
        &self,
        tx: mpsc::UnboundedSender<Vec<u8>>,
    ) -> Result<Stream, Box<dyn std::error::Error>> {
        let config = self.config.clone();

        let stream = match self.sample_format {
            SampleFormat::F32 => self.build_stream::<f32>(config, tx)?,
            SampleFormat::I16 => self.build_stream::<i16>(config, tx)?,
            SampleFormat::U16 => self.build_stream::<u16>(config, tx)?,
            _ => return Err("Unsupported sample format".into()),
        };

        stream.play()?;
        Ok(stream)
    }

    fn build_stream<T>(
        &self,
        config: StreamConfig,
        tx: mpsc::UnboundedSender<Vec<u8>>,
    ) -> Result<Stream, Box<dyn std::error::Error>>
    where
        T: cpal::Sample + cpal::SizedSample + Send + 'static,
        f32: cpal::FromSample<T>,
    {
        let stream = self.device.build_input_stream(
            &config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                // Convert samples to f32 and then to i16 for Deepgram
                let mut audio_data = Vec::with_capacity(data.len() * 2);

                for &sample in data.iter() {
                    let f32_sample: f32 = cpal::Sample::from_sample(sample);
                    let i16_sample = (f32_sample * i16::MAX as f32) as i16;
                    audio_data.extend_from_slice(&i16_sample.to_le_bytes());
                }

                if let Err(_e) = tx.send(audio_data) {
                    // Audio capture stopped, this is expected when shutting down
                }
            },
            |err| eprintln!("Audio stream error: {}", err),
            None,
        )?;

        Ok(stream)
    }
}

pub(crate) struct AudioFileReader {
    path: PathBuf,
}

/// Return a probe hint based on the file contents when the extension is known
/// to be unreliable. In particular, AAC/ADTS files are sometimes delivered
/// with a `.wav` suffix, which causes Symphonia to select the WAV demuxer.
fn probe_extension(path: &Path) -> Option<String> {
    let extension = path.extension().and_then(|ext| ext.to_str())?;

    if !extension.eq_ignore_ascii_case("wav") {
        return Some(extension.to_owned());
    }

    let header = std::fs::read(path).ok()?;
    if header.starts_with(b"RIFF") && header.get(8..12) == Some(b"WAVE") {
        return Some("wav".to_owned());
    }

    // ADTS sync words: 0xfff followed by MPEG-4/2 layer and protection bits.
    if header.len() >= 2 && header[0] == 0xff && (header[1] & 0xf6) == 0xf0 {
        return Some("aac".to_owned());
    }

    // Do not force a WAV demuxer for a file that is not a RIFF/WAVE file.
    None
}

/// Return the number of bytes in a WAV data chunk.
///
/// Some recording tools leave the data chunk length as zero while appending
/// the actual samples to the file. In that case, use the bytes available after
/// the chunk header instead of trusting the invalid declared length.
pub(crate) fn wav_data_len(bytes: &[u8]) -> Option<u64> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }

    let mut offset = 12usize;
    while offset.checked_add(8)? <= bytes.len() {
        let chunk_size = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().ok()?) as u64;
        let data_start = offset + 8;
        let available = bytes.len().saturating_sub(data_start) as u64;

        if &bytes[offset..offset + 4] == b"data" {
            return Some(if chunk_size == 0 || chunk_size > available {
                available
            } else {
                chunk_size
            });
        }

        let next = data_start
            .checked_add(chunk_size as usize)?
            .checked_add((chunk_size as usize) & 1)?;
        if next > bytes.len() {
            break;
        }
        offset = next;
    }

    None
}

/// Repair a malformed WAV data chunk in memory so Symphonia can stream it.
/// Returns `None` when the file is valid or is not a WAV file.
pub(crate) fn repair_wav_header(mut bytes: Vec<u8>) -> Option<Vec<u8>> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }

    let mut offset = 12usize;
    while offset.checked_add(8)? <= bytes.len() {
        let chunk_size = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().ok()?) as u64;
        let data_start = offset + 8;
        let available = bytes.len().saturating_sub(data_start) as u64;

        if &bytes[offset..offset + 4] == b"data" {
            if chunk_size != 0 && chunk_size <= available {
                return None;
            }
            let data_len = u32::try_from(available).ok()?;
            bytes[offset + 4..offset + 8].copy_from_slice(&data_len.to_le_bytes());
            let riff_len = u32::try_from(bytes.len().saturating_sub(8)).ok()?;
            bytes[4..8].copy_from_slice(&riff_len.to_le_bytes());
            return Some(bytes);
        }

        let next = data_start
            .checked_add(chunk_size as usize)?
            .checked_add((chunk_size as usize) & 1)?;
        if next > bytes.len() {
            break;
        }
        offset = next;
    }

    None
}

fn fallback_wav_frame_count(
    path: &Path,
    codec_params: &symphonia::core::codecs::CodecParameters,
) -> Option<u64> {
    use symphonia::core::codecs::*;

    let is_pcm = matches!(
        codec_params.codec,
        CODEC_TYPE_PCM_S16LE
            | CODEC_TYPE_PCM_S16BE
            | CODEC_TYPE_PCM_S24LE
            | CODEC_TYPE_PCM_S24BE
            | CODEC_TYPE_PCM_S32LE
            | CODEC_TYPE_PCM_S32BE
            | CODEC_TYPE_PCM_F32LE
            | CODEC_TYPE_PCM_F32BE
            | CODEC_TYPE_PCM_F64LE
            | CODEC_TYPE_PCM_F64BE
            | CODEC_TYPE_PCM_ALAW
            | CODEC_TYPE_PCM_MULAW
    );
    if !is_pcm
        || !path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("wav"))
    {
        return None;
    }

    let channels = codec_params.channels?.count() as u64;
    let bits_per_sample = codec_params.bits_per_sample? as u64;
    let bytes_per_frame = channels.checked_mul((bits_per_sample + 7) / 8)?;
    let data_len = wav_data_len(&std::fs::read(path).ok()?)?;
    Some(data_len / bytes_per_frame)
}

impl AudioFileReader {
    pub(crate) fn new(path: PathBuf) -> Self {
        AudioFileReader { path }
    }

    pub(crate) async fn stream_file(
        &self,
        tx: mpsc::Sender<Vec<u8>>,
        config_tx: oneshot::Sender<(u32, u16)>,
        ready_rx: Option<oneshot::Receiver<()>>,
        fast_mode: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let repaired_wav = if self
            .path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("wav"))
        {
            std::fs::read(&self.path).ok().and_then(repair_wav_header)
        } else {
            None
        };
        let source: Box<dyn MediaSource> = match repaired_wav {
            Some(bytes) => Box::new(Cursor::new(bytes)),
            None => Box::new(File::open(&self.path)?),
        };
        let mss = MediaSourceStream::new(source, Default::default());

        let mut hint = Hint::new();
        if let Some(extension) = probe_extension(&self.path) {
            hint.with_extension(&extension);
        }

        let meta_opts: MetadataOptions = Default::default();
        let fmt_opts: FormatOptions = Default::default();

        let probed = symphonia::default::get_probe().format(&hint, mss, &fmt_opts, &meta_opts)?;

        let mut format = probed.format;

        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or("No supported audio tracks found")?;

        let track_id = track.id;
        let codec_params = &track.codec_params;

        let sample_rate = codec_params.sample_rate.ok_or("Sample rate not found")?;
        let channels = codec_params
            .channels
            .map(|channels| channels.count())
            .or_else(|| {
                codec_params
                    .channel_layout
                    .map(|layout| layout.into_channels().count())
            })
            .ok_or_else(|| {
                "Audio channel metadata not found (the file may be mislabeled; use the actual AAC/WAV extension)"
            })? as u16;
        let total_frames = codec_params
            .n_frames
            .filter(|frames| *frames > 0)
            .or_else(|| fallback_wav_frame_count(&self.path, codec_params));

        // --- File metadata ---
        let codec_str = codec_name(codec_params.codec);

        let channel_str = match channels {
            1 => "mono".to_string(),
            2 => "stereo".to_string(),
            n => format!("{n}ch"),
        };

        let bit_depth_str = codec_params
            .bits_per_sample
            .map(|b| format!(", {b}-bit"))
            .unwrap_or_default();

        let duration_str = total_frames
            .map(|f| {
                let s = f / sample_rate as u64;
                format!("{}:{:02}", s / 60, s % 60)
            })
            .unwrap_or_else(|| "unknown".to_string());

        // Bitrate: for compressed formats derive from file size + duration;
        // for uncompressed (PCM) calculate directly from the stream parameters.
        let bitrate_str = {
            let from_file = total_frames.and_then(|f| {
                let dur = f as f64 / sample_rate as f64;
                if dur > 0.0 {
                    std::fs::metadata(&self.path).ok().map(|m| {
                        format!(
                            "{} kbps",
                            (m.len() as f64 * 8.0 / dur / 1000.0).round() as u32
                        )
                    })
                } else {
                    None
                }
            });
            let from_params = codec_params
                .bits_per_sample
                .map(|b| format!("{} kbps", sample_rate * channels as u32 * b / 1000));
            from_file
                .or(from_params)
                .unwrap_or_else(|| "unknown".to_string())
        };

        println!("File:    {}", self.path.display());
        println!("Format:  {codec_str}, {bitrate_str}");
        println!("Audio:   {sample_rate} Hz, {channel_str}{bit_depth_str}");
        println!("Length:  {duration_str}");

        // Send the audio configuration immediately so Deepgram client can start
        let _ = config_tx.send((sample_rate, channels));

        // Wait for WebSocket to be ready before streaming (ensures request ID
        // is printed before the progress bar appears).
        if let Some(rx) = ready_rx {
            let _ = rx.await;
            if fast_mode {
                println!("WebSocket ready, starting audio stream at {FAST_STREAM_RATE}x...");
            }
        }

        // Set up progress bar
        let pb = if let Some(total) = total_frames {
            let total_secs = total / sample_rate as u64;
            let pb = ProgressBar::new(total_secs);
            pb.set_style(
                ProgressStyle::with_template("{bar:40.cyan/blue} {msg}")
                    .unwrap()
                    .progress_chars("█▓░"),
            );
            pb.set_message(format!("0:00 / {}:{:02}", total_secs / 60, total_secs % 60));
            Some((pb, total_secs))
        } else {
            None
        };

        let dec_opts: DecoderOptions = Default::default();
        let mut decoder = symphonia::default::get_codecs().make(&codec_params, &dec_opts)?;

        let mut sample_buf = None;
        let mut frames_sent: u64 = 0;
        let mut pacing_delay = Duration::ZERO;
        let stream_rate = file_stream_rate(fast_mode);

        loop {
            let packet = match format.next_packet() {
                Ok(packet) => packet,
                Err(SymphoniaError::ResetRequired) => {
                    decoder.reset();
                    continue;
                }
                Err(SymphoniaError::IoError(e))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    break;
                }
                Err(err) => return Err(Box::new(err)),
            };

            if packet.track_id() != track_id {
                continue;
            }

            match decoder.decode(&packet) {
                Ok(decoded) => {
                    let frame_count = decoded.frames() as u64;

                    if sample_buf.is_none() {
                        let spec = *decoded.spec();
                        let duration = decoded.capacity() as u64;
                        sample_buf = Some(SampleBuffer::<i16>::new(duration, spec));
                    }

                    if let Some(buf) = &mut sample_buf {
                        buf.copy_interleaved_ref(decoded);

                        let samples = buf.samples();
                        let mut audio_data = Vec::with_capacity(samples.len() * 2);

                        for &sample in samples {
                            audio_data.extend_from_slice(&sample.to_le_bytes());
                        }

                        // Pace before sending the next chunk. Sleeping after
                        // sending the final chunk leaves the input channel open
                        // and delays the Deepgram Finalize message.
                        if !pacing_delay.is_zero() {
                            tokio::time::sleep(pacing_delay).await;
                        }

                        if tx.send(audio_data).await.is_err() {
                            break;
                        }

                        frames_sent += frame_count;

                        // Update progress bar
                        if let Some((ref pb, total_secs)) = pb {
                            let current_secs = frames_sent / sample_rate as u64;
                            pb.set_position(current_secs);
                            pb.set_message(format!(
                                "{}:{:02} / {}:{:02}",
                                current_secs / 60,
                                current_secs % 60,
                                total_secs / 60,
                                total_secs % 60,
                            ));
                        }

                        // Pace before the next chunk without delaying end-of-input.
                        // Deepgram accepts streaming audio at up to 1.25x realtime.
                        pacing_delay = Duration::from_secs_f64(
                            frame_count as f64 / sample_rate as f64 / stream_rate,
                        );
                    }
                }
                Err(SymphoniaError::IoError(_)) => continue,
                Err(SymphoniaError::DecodeError(_)) => continue,
                Err(err) => return Err(Box::new(err)),
            }
        }

        if let Some((pb, _)) = pb {
            pb.finish_and_clear();
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AudioEvent, file_stream_rate, probe_extension, start_audio_fanout,
        start_bounded_audio_fanout,
    };
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(suffix: &str) -> std::path::PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("dg-stt-probe-{nonce}-{counter}{suffix}"))
    }

    #[test]
    fn recognizes_adts_aac_with_wav_extension() {
        let path = temp_path(".wav");
        fs::write(&path, [0xff, 0xf1, 0x50, 0x80]).unwrap();

        assert_eq!(probe_extension(&path).as_deref(), Some("aac"));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn keeps_wav_hint_for_riff_wave_files() {
        let path = temp_path(".wav");
        let mut header = b"RIFF".to_vec();
        header.extend_from_slice(&[0; 4]);
        header.extend_from_slice(b"WAVE");
        fs::write(&path, header).unwrap();

        assert_eq!(probe_extension(&path).as_deref(), Some("wav"));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn fast_mode_uses_deepgrams_maximum_streaming_rate() {
        assert_eq!(file_stream_rate(false), 1.0);
        assert_eq!(file_stream_rate(true), 1.25);
    }

    #[tokio::test]
    async fn bounded_file_fanout_preserves_order_and_sends_end_to_every_connection() {
        let (tx, mut receivers, fanout_task) = start_bounded_audio_fanout(2, 4);

        tx.send(vec![1]).await.unwrap();
        tx.send(vec![2]).await.unwrap();
        drop(tx);
        fanout_task.await.unwrap();

        for receiver in &mut receivers {
            assert!(matches!(receiver.recv().await, Some(AudioEvent::Data(data)) if data == [1]));
            assert!(matches!(receiver.recv().await, Some(AudioEvent::Data(data)) if data == [2]));
            assert!(matches!(receiver.recv().await, Some(AudioEvent::End)));
        }
    }

    #[tokio::test]
    async fn microphone_fanout_sends_end_after_source_closes() {
        let (tx, mut receivers, fanout_task) = start_audio_fanout(1);
        let receiver = receivers.first_mut().unwrap();

        tx.send(vec![7, 8]).unwrap();
        drop(tx);

        assert!(matches!(receiver.recv().await, Some(AudioEvent::Data(data)) if data == [7, 8]));
        assert!(matches!(receiver.recv().await, Some(AudioEvent::End)));
        fanout_task.await.unwrap();
    }
}
