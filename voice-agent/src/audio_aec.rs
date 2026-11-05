//! CPAL device integration for the optional AEC3 path.

use std::{
    collections::VecDeque,
    error::Error,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, SyncSender},
        Arc, Mutex,
    },
};

use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Device, SampleFormat, Stream, StreamConfig,
};
use log::{error, info};

use super::{aec, AudioCapture};

const PLAYBACK_QUEUE_CAPACITY: usize = 64;

pub(super) fn start_capture(
    capture: &AudioCapture,
    sender: aec::AecPacketSender,
    mic_enabled: Arc<AtomicBool>,
    mute_on_playback: bool,
) -> Result<Stream, Box<dyn Error>> {
    let config = capture.config.clone();
    let stream = match capture.sample_format {
        SampleFormat::I8 => build_capture_stream::<i8>(
            &capture.device,
            config,
            sender,
            mic_enabled,
            mute_on_playback,
        )?,
        SampleFormat::I16 => build_capture_stream::<i16>(
            &capture.device,
            config,
            sender,
            mic_enabled,
            mute_on_playback,
        )?,
        SampleFormat::I24 => build_capture_stream::<cpal::I24>(
            &capture.device,
            config,
            sender,
            mic_enabled,
            mute_on_playback,
        )?,
        SampleFormat::I32 => build_capture_stream::<i32>(
            &capture.device,
            config,
            sender,
            mic_enabled,
            mute_on_playback,
        )?,
        SampleFormat::I64 => build_capture_stream::<i64>(
            &capture.device,
            config,
            sender,
            mic_enabled,
            mute_on_playback,
        )?,
        SampleFormat::U8 => build_capture_stream::<u8>(
            &capture.device,
            config,
            sender,
            mic_enabled,
            mute_on_playback,
        )?,
        SampleFormat::U16 => build_capture_stream::<u16>(
            &capture.device,
            config,
            sender,
            mic_enabled,
            mute_on_playback,
        )?,
        SampleFormat::U32 => build_capture_stream::<u32>(
            &capture.device,
            config,
            sender,
            mic_enabled,
            mute_on_playback,
        )?,
        SampleFormat::U64 => build_capture_stream::<u64>(
            &capture.device,
            config,
            sender,
            mic_enabled,
            mute_on_playback,
        )?,
        SampleFormat::F32 => build_capture_stream::<f32>(
            &capture.device,
            config,
            sender,
            mic_enabled,
            mute_on_playback,
        )?,
        SampleFormat::F64 => build_capture_stream::<f64>(
            &capture.device,
            config,
            sender,
            mic_enabled,
            mute_on_playback,
        )?,
        _ => return Err("Unsupported AEC microphone sample format".into()),
    };

    stream.play()?;
    Ok(stream)
}

fn build_capture_stream<T>(
    device: &Device,
    config: StreamConfig,
    sender: aec::AecPacketSender,
    mic_enabled: Arc<AtomicBool>,
    mute_on_playback: bool,
) -> Result<Stream, Box<dyn Error>>
where
    T: cpal::Sample + cpal::SizedSample + Send + 'static,
    f32: cpal::FromSample<T>,
{
    let channels = config.channels;
    let sample_rate = config.sample_rate.0;
    let mut sample_position = 0u64;
    let mut fallback_conversion_remainder = 0u64;
    let mut fallback_frame_remainder = 0u64;
    let error_sender = sender.clone();
    let stream = device.build_input_stream(
        &config,
        move |data: &[T], info: &cpal::InputCallbackInfo| {
            let input_frames = data.len() / channels.max(1) as usize;
            let first_sample = sample_position;
            sample_position = sample_position.saturating_add(input_frames as u64);

            if sender.worker_failed() {
                sender.fallback_capture_silence(
                    input_frames,
                    sample_rate,
                    &mut fallback_conversion_remainder,
                    &mut fallback_frame_remainder,
                );
                return;
            }

            let samples = if mute_on_playback && !mic_enabled.load(Ordering::Relaxed) {
                vec![0.0; input_frames]
            } else {
                aec::downmix_to_mono(data, channels)
            };
            if matches!(
                sender.submit(
                    aec::StreamDirection::Capture,
                    info.timestamp().capture,
                    first_sample,
                    sample_rate,
                    samples,
                ),
                Err(aec::SubmitError::WorkerStopped)
            ) {
                sender.fallback_capture_silence(
                    input_frames,
                    sample_rate,
                    &mut fallback_conversion_remainder,
                    &mut fallback_frame_remainder,
                );
            }
        },
        move |err| {
            error!("AEC microphone stream error: {}", err);
            error_sender.mark_discontinuity();
        },
        None,
    )?;
    Ok(stream)
}

pub(super) struct AudioPlayer {
    _stream: Stream,
    tx: SyncSender<PlaybackPacket>,
    resampler: Mutex<aec::StreamingResampler>,
    generation: Arc<AtomicU64>,
}

struct PlaybackPacket {
    generation: u64,
    samples: Vec<f32>,
}

#[derive(Clone)]
struct OutputControl {
    generation: Arc<AtomicU64>,
    mic_enabled: Arc<AtomicBool>,
    mute_on_playback: bool,
}

impl AudioPlayer {
    pub(super) fn new(
        render_sender: aec::AecPacketSender,
        generation: Arc<AtomicU64>,
        mic_enabled: Arc<AtomicBool>,
        mute_on_playback: bool,
    ) -> Result<Self, Box<dyn Error>> {
        let device = cpal::default_host()
            .default_output_device()
            .ok_or("No output device available for AEC")?;
        let supported_config = device.default_output_config()?;
        let sample_format = supported_config.sample_format();
        let config: StreamConfig = supported_config.into();
        let sample_rate = config.sample_rate.0;
        let channels = config.channels;
        if channels == 0 || sample_rate == 0 {
            return Err("AEC output device reported an invalid audio format".into());
        }

        let (tx, rx) = mpsc::sync_channel::<PlaybackPacket>(PLAYBACK_QUEUE_CAPACITY);
        let output_control = OutputControl {
            generation: generation.clone(),
            mic_enabled,
            mute_on_playback,
        };
        let stream = match sample_format {
            SampleFormat::I8 => build_output_stream::<i8>(
                &device,
                config,
                rx,
                render_sender,
                output_control.clone(),
            )?,
            SampleFormat::I16 => build_output_stream::<i16>(
                &device,
                config,
                rx,
                render_sender,
                output_control.clone(),
            )?,
            SampleFormat::I24 => build_output_stream::<cpal::I24>(
                &device,
                config,
                rx,
                render_sender,
                output_control.clone(),
            )?,
            SampleFormat::I32 => build_output_stream::<i32>(
                &device,
                config,
                rx,
                render_sender,
                output_control.clone(),
            )?,
            SampleFormat::I64 => build_output_stream::<i64>(
                &device,
                config,
                rx,
                render_sender,
                output_control.clone(),
            )?,
            SampleFormat::U8 => build_output_stream::<u8>(
                &device,
                config,
                rx,
                render_sender,
                output_control.clone(),
            )?,
            SampleFormat::U16 => build_output_stream::<u16>(
                &device,
                config,
                rx,
                render_sender,
                output_control.clone(),
            )?,
            SampleFormat::U32 => build_output_stream::<u32>(
                &device,
                config,
                rx,
                render_sender,
                output_control.clone(),
            )?,
            SampleFormat::U64 => build_output_stream::<u64>(
                &device,
                config,
                rx,
                render_sender,
                output_control.clone(),
            )?,
            SampleFormat::F32 => build_output_stream::<f32>(
                &device,
                config,
                rx,
                render_sender,
                output_control.clone(),
            )?,
            SampleFormat::F64 => build_output_stream::<f64>(
                &device,
                config,
                rx,
                render_sender,
                output_control.clone(),
            )?,
            _ => return Err("Unsupported AEC speaker sample format".into()),
        };
        stream.play()?;
        info!(
            "AEC speaker reference active at {} Hz, {} channel(s)",
            sample_rate, channels
        );

        Ok(Self {
            _stream: stream,
            tx,
            resampler: Mutex::new(aec::StreamingResampler::new(
                aec::VOICE_AGENT_SAMPLE_RATE,
                sample_rate,
            )),
            generation,
        })
    }

    pub(super) fn clear(&self) {
        if let Ok(mut resampler) = self.resampler.lock() {
            resampler.reset();
        }
    }

    pub(super) fn play_audio(&self, audio_data: Vec<u8>) -> Result<(), Box<dyn Error>> {
        if audio_data.is_empty() {
            return Ok(());
        }
        if audio_data.len() % 2 != 0 {
            return Err("AEC playback received an incomplete linear16 sample".into());
        }

        let input: Vec<f32> = audio_data
            .chunks_exact(2)
            .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / 32768.0)
            .collect();
        let samples = self
            .resampler
            .lock()
            .map_err(|_| "AEC playback resampler lock poisoned")?
            .push(&input);
        if samples.is_empty() {
            return Ok(());
        }

        let generation = self.generation.load(Ordering::Acquire);
        self.tx
            .try_send(PlaybackPacket {
                generation,
                samples,
            })
            .map_err(|_| "AEC speaker queue is full; audio chunk dropped".into())
    }
}

fn build_output_stream<T>(
    device: &Device,
    config: StreamConfig,
    playback_rx: mpsc::Receiver<PlaybackPacket>,
    render_sender: aec::AecPacketSender,
    control: OutputControl,
) -> Result<Stream, Box<dyn Error>>
where
    T: cpal::Sample + cpal::SizedSample + cpal::FromSample<f32> + Copy + Send + 'static,
    f32: cpal::FromSample<T>,
{
    let OutputControl {
        generation,
        mic_enabled,
        mute_on_playback,
    } = control;
    let channels = config.channels as usize;
    let sample_rate = config.sample_rate.0;
    let mut pending_samples = VecDeque::<f32>::new();
    let mut current_generation = generation.load(Ordering::Acquire);
    let mut sample_position = 0u64;
    let mut silent_frames = 0usize;
    let error_sender = render_sender.clone();
    let stream = device.build_output_stream(
        &config,
        move |output: &mut [T], info: &cpal::OutputCallbackInfo| {
            let latest_generation = generation.load(Ordering::Acquire);
            if latest_generation != current_generation {
                pending_samples.clear();
                current_generation = latest_generation;
            }
            while let Ok(packet) = playback_rx.try_recv() {
                if packet.generation == current_generation {
                    pending_samples.extend(packet.samples);
                }
            }

            let current_generation_now = generation.load(Ordering::Acquire);
            if current_generation_now != current_generation {
                pending_samples.clear();
                current_generation = current_generation_now;
            }

            let frames = output.len() / channels;
            let mut render_reference = Vec::with_capacity(frames);
            let mut played_audio = false;
            for frame_index in 0..frames {
                let sample = match pending_samples.pop_front() {
                    Some(sample) => {
                        played_audio = true;
                        sample.clamp(-1.0, 1.0)
                    }
                    None => 0.0,
                };
                let converted: T = cpal::Sample::from_sample(sample);
                let start = frame_index * channels;
                let mut downmix = 0.0f32;
                for output_sample in &mut output[start..start + channels] {
                    *output_sample = converted;
                    let actual: f32 = cpal::Sample::from_sample(*output_sample);
                    downmix += actual;
                }
                render_reference.push(downmix / channels as f32);
            }
            for output_sample in &mut output[frames * channels..] {
                *output_sample = cpal::Sample::from_sample(0.0f32);
            }

            if mute_on_playback {
                if played_audio {
                    mic_enabled.store(false, Ordering::Relaxed);
                    silent_frames = 0;
                } else if !mic_enabled.load(Ordering::Relaxed) {
                    silent_frames = silent_frames.saturating_add(frames);
                    if silent_frames >= sample_rate as usize * 600 / 1_000 {
                        mic_enabled.store(true, Ordering::Relaxed);
                        silent_frames = 0;
                    }
                }
            } else {
                mic_enabled.store(true, Ordering::Relaxed);
            }

            let first_sample = sample_position;
            sample_position = sample_position.saturating_add(frames as u64);
            let _ = render_sender.submit(
                aec::StreamDirection::Render,
                info.timestamp().playback,
                first_sample,
                sample_rate,
                render_reference,
            );
        },
        move |err| {
            error!("AEC speaker stream error: {}", err);
            error_sender.mark_discontinuity();
        },
        None,
    )?;
    Ok(stream)
}
