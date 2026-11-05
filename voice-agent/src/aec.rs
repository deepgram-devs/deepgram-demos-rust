use std::{
    collections::VecDeque,
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender, TrySendError},
        Arc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use aec3::{nodes::audio::AudioFormat, pipelines::linear};
use cpal::StreamInstant;
use tokio::sync::mpsc as tokio_mpsc;

pub const AEC_SAMPLE_RATE: u32 = 48_000;
pub const AEC_FRAME_SAMPLES: usize = 480;
pub const VOICE_AGENT_SAMPLE_RATE: u32 = 24_000;
const VOICE_AGENT_FRAME_SAMPLES: usize = 240;
const VOICE_AGENT_FRAME_BYTES: usize = VOICE_AGENT_FRAME_SAMPLES * 2;
const PACKET_QUEUE_CAPACITY: usize = 64;
const MAX_STREAM_GAP: Duration = Duration::from_millis(100);
const MAX_REFERENCE_AGE: Duration = Duration::from_millis(250);

#[derive(Clone, Copy, Debug)]
pub enum StreamDirection {
    Capture,
    Render,
}

pub struct AecPacket {
    pub direction: StreamDirection,
    pub timestamp: StreamInstant,
    pub first_sample: u64,
    pub sample_rate: u32,
    pub samples: Vec<f32>,
    discontinuity: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum SubmitError {
    QueueFull,
    WorkerStopped,
}

#[derive(Clone)]
pub struct AecPacketSender {
    tx: SyncSender<AecPacket>,
    discontinuity: Arc<AtomicBool>,
    worker_failed: Arc<AtomicBool>,
    silence_tx: tokio_mpsc::Sender<Vec<u8>>,
}

impl AecPacketSender {
    pub fn submit(
        &self,
        direction: StreamDirection,
        timestamp: StreamInstant,
        first_sample: u64,
        sample_rate: u32,
        samples: Vec<f32>,
    ) -> Result<(), SubmitError> {
        if self.worker_failed.load(Ordering::Acquire) {
            return Err(SubmitError::WorkerStopped);
        }

        let packet = AecPacket {
            direction,
            timestamp,
            first_sample,
            sample_rate,
            samples,
            discontinuity: self.discontinuity.swap(false, Ordering::AcqRel),
        };

        match self.tx.try_send(packet) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => {
                self.discontinuity.store(true, Ordering::Release);
                Err(SubmitError::QueueFull)
            }
            Err(TrySendError::Disconnected(_)) => {
                self.worker_failed.store(true, Ordering::Release);
                Err(SubmitError::WorkerStopped)
            }
        }
    }

    pub fn mark_discontinuity(&self) {
        self.discontinuity.store(true, Ordering::Release);
    }

    pub fn worker_failed(&self) -> bool {
        self.worker_failed.load(Ordering::Acquire)
    }

    /// Emit zeroed 10 ms frames if the DSP worker has stopped. This keeps the
    /// outbound stream fail-closed and never sends raw microphone audio.
    pub fn fallback_capture_silence(
        &self,
        input_frames: usize,
        input_rate: u32,
        conversion_remainder: &mut u64,
        frame_remainder: &mut u64,
    ) {
        if input_rate == 0 {
            return;
        }

        let numerator =
            input_frames as u64 * VOICE_AGENT_SAMPLE_RATE as u64 + *conversion_remainder;
        let wire_samples = numerator / input_rate as u64;
        *conversion_remainder = numerator % input_rate as u64;
        *frame_remainder += wire_samples;

        while *frame_remainder >= VOICE_AGENT_FRAME_SAMPLES as u64 {
            let _ = self.silence_tx.try_send(vec![0; VOICE_AGENT_FRAME_BYTES]);
            *frame_remainder -= VOICE_AGENT_FRAME_SAMPLES as u64;
        }
    }
}

pub struct AecWorker {
    sender: AecPacketSender,
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl AecWorker {
    pub fn spawn(
        initial_delay_ms: Option<u16>,
        audio_tx: tokio_mpsc::Sender<Vec<u8>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let (tx, rx) = mpsc::sync_channel(PACKET_QUEUE_CAPACITY);
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let discontinuity = Arc::new(AtomicBool::new(false));
        let worker_failed = Arc::new(AtomicBool::new(false));
        let sender = AecPacketSender {
            tx,
            discontinuity,
            worker_failed: Arc::clone(&worker_failed),
            silence_tx: audio_tx.clone(),
        };
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let join = thread::Builder::new()
            .name("voice-agent-aec".into())
            .spawn(move || {
                let result = catch_unwind(AssertUnwindSafe(|| {
                    run_worker(rx, audio_tx, initial_delay_ms, &worker_stop, ready_tx)
                }));

                match result {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) if worker_stop.load(Ordering::Acquire) => {
                        log::debug!("AEC worker stopped: {error}");
                    }
                    Ok(Err(error)) => {
                        log::error!("AEC worker failed closed: {error}");
                        worker_failed.store(true, Ordering::Release);
                    }
                    Err(_) => {
                        log::error!("AEC worker panicked; microphone processing failed closed");
                        worker_failed.store(true, Ordering::Release);
                    }
                }
            })?;

        match ready_rx.recv() {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                stop.store(true, Ordering::Release);
                let _ = join.join();
                return Err(
                    std::io::Error::other(format!("Unable to initialize AEC: {error}")).into(),
                );
            }
            Err(error) => {
                stop.store(true, Ordering::Release);
                let _ = join.join();
                return Err(std::io::Error::other(format!(
                    "AEC worker initialization failed: {error}"
                ))
                .into());
            }
        }

        Ok(Self {
            sender,
            stop,
            join: Some(join),
        })
    }

    pub fn sender(&self) -> AecPacketSender {
        self.sender.clone()
    }
}

impl Drop for AecWorker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn build_pipeline(initial_delay_ms: Option<u16>) -> Result<linear::LinearPipeline, String> {
    let format = AudioFormat::ten_ms(AEC_SAMPLE_RATE, 1);
    let mut builder = linear::builder(format, format)
        .enable_high_pass_filter(false)
        .enable_noise_suppression(false)
        .enable_gain_controller2(false)
        .enable_post_filter(false);
    if let Some(delay_ms) = initial_delay_ms {
        builder = builder.initial_delay_ms(i32::from(delay_ms));
    }
    builder.build().map_err(|error| error.to_string())
}

fn run_worker(
    rx: mpsc::Receiver<AecPacket>,
    audio_tx: tokio_mpsc::Sender<Vec<u8>>,
    initial_delay_ms: Option<u16>,
    stop: &AtomicBool,
    ready_tx: SyncSender<Result<(), String>>,
) -> Result<(), String> {
    let pipeline = match build_pipeline(initial_delay_ms) {
        Ok(pipeline) => pipeline,
        Err(error) => {
            let _ = ready_tx.send(Err(error.clone()));
            return Err(error);
        }
    };
    ready_tx
        .send(Ok(()))
        .map_err(|_| "AEC startup receiver closed".to_string())?;
    let mut engine = AecEngine::new(pipeline, audio_tx);
    while !stop.load(Ordering::Acquire) {
        match rx.recv_timeout(Duration::from_millis(20)) {
            Ok(packet) => engine.process_packet(packet)?,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    Ok(())
}

struct StreamCursor {
    expected_sample: Option<u64>,
    last_timestamp: Option<StreamInstant>,
    last_frames: usize,
    last_rate: u32,
}

#[derive(Default)]
struct Continuity {
    discontinuous: bool,
    stale: bool,
    gap_samples: u64,
}

impl StreamCursor {
    fn observe(&mut self, packet: &AecPacket) -> Continuity {
        let mut result = Continuity::default();
        if let Some(expected) = self.expected_sample {
            if packet.first_sample < expected {
                result.stale = true;
            } else if packet.first_sample > expected {
                result.discontinuous = true;
                result.gap_samples = packet.first_sample - expected;
            }
        }

        if let Some(last_timestamp) = self.last_timestamp {
            if packet.timestamp < last_timestamp {
                result.stale = true;
            } else if self.last_rate > 0 {
                let expected_time = last_timestamp
                    .add(Duration::from_secs_f64(
                        self.last_frames as f64 / self.last_rate as f64,
                    ))
                    .unwrap_or(last_timestamp);
                let delta = packet
                    .timestamp
                    .duration_since(&expected_time)
                    .or_else(|| expected_time.duration_since(&packet.timestamp))
                    .unwrap_or_default();
                if delta > MAX_STREAM_GAP {
                    result.discontinuous = true;
                }
            }
        }

        if self.last_rate != 0 && self.last_rate != packet.sample_rate {
            result.discontinuous = true;
        }

        if !result.stale {
            self.expected_sample = Some(packet.first_sample + packet.samples.len() as u64);
            self.last_timestamp = Some(packet.timestamp);
            self.last_frames = packet.samples.len();
            self.last_rate = packet.sample_rate;
        }
        result
    }
}

struct AecEngine {
    pipeline: linear::LinearPipeline,
    audio_tx: tokio_mpsc::Sender<Vec<u8>>,
    capture_cursor: StreamCursor,
    render_cursor: StreamCursor,
    capture_resampler: Option<StreamingResampler>,
    render_resampler: Option<StreamingResampler>,
    capture_buffer: VecDeque<f32>,
    render_buffer: VecDeque<f32>,
    forced_silence_samples: usize,
    render_ready: bool,
    render_end: Option<StreamInstant>,
}

impl AecEngine {
    fn new(pipeline: linear::LinearPipeline, audio_tx: tokio_mpsc::Sender<Vec<u8>>) -> Self {
        Self {
            pipeline,
            audio_tx,
            capture_cursor: StreamCursor {
                expected_sample: None,
                last_timestamp: None,
                last_frames: 0,
                last_rate: 0,
            },
            render_cursor: StreamCursor {
                expected_sample: None,
                last_timestamp: None,
                last_frames: 0,
                last_rate: 0,
            },
            capture_resampler: None,
            render_resampler: None,
            capture_buffer: VecDeque::new(),
            render_buffer: VecDeque::new(),
            forced_silence_samples: 0,
            render_ready: false,
            render_end: None,
        }
    }

    fn process_packet(&mut self, packet: AecPacket) -> Result<(), String> {
        if packet.sample_rate == 0 {
            self.reset()?;
            if matches!(packet.direction, StreamDirection::Capture) {
                self.emit_silence_frames(1)?;
            }
            return Ok(());
        }

        let continuity = match packet.direction {
            StreamDirection::Capture => self.capture_cursor.observe(&packet),
            StreamDirection::Render => self.render_cursor.observe(&packet),
        };

        if continuity.stale {
            self.reset()?;
            if matches!(packet.direction, StreamDirection::Capture) {
                self.push_capture_samples(
                    &vec![0.0; packet.samples.len()],
                    packet.sample_rate,
                    true,
                )?;
            }
            return Ok(());
        }

        let discontinuous = packet.discontinuity || continuity.discontinuous;
        if discontinuous {
            if matches!(packet.direction, StreamDirection::Capture) && continuity.gap_samples > 0 {
                let missing_48k = continuity.gap_samples as f64 * AEC_SAMPLE_RATE as f64
                    / packet.sample_rate as f64;
                let frames = (missing_48k / AEC_FRAME_SAMPLES as f64).ceil() as usize;
                self.emit_silence_frames(frames)?;
            }
            self.reset()?;
            return match packet.direction {
                StreamDirection::Capture => self.push_capture_samples(
                    &vec![0.0; packet.samples.len()],
                    packet.sample_rate,
                    true,
                ),
                StreamDirection::Render => self.process_render_samples(&packet),
            };
        }

        match packet.direction {
            StreamDirection::Capture => self.process_capture_samples(&packet),
            StreamDirection::Render => self.process_render_samples(&packet),
        }
    }

    fn process_render_samples(&mut self, packet: &AecPacket) -> Result<(), String> {
        if packet.samples.iter().any(|sample| !sample.is_finite()) {
            self.reset()?;
            return Ok(());
        }

        let converted = resample_packet(
            &mut self.render_resampler,
            packet.sample_rate,
            &packet.samples,
        );
        self.render_buffer.extend(converted);

        while self.render_buffer.len() >= AEC_FRAME_SAMPLES {
            let frame: Vec<f32> = self.render_buffer.drain(..AEC_FRAME_SAMPLES).collect();
            if let Err(error) = self.pipeline.handle_render_frame(&frame) {
                self.reset()?;
                log::warn!("AEC render reference rejected; output will remain silent until resynchronized: {error}");
                return Ok(());
            }
            self.render_ready = true;
        }

        self.render_end = packet.timestamp.add(Duration::from_secs_f64(
            packet.samples.len() as f64 / packet.sample_rate as f64,
        ));
        Ok(())
    }

    fn process_capture_samples(&mut self, packet: &AecPacket) -> Result<(), String> {
        if packet.samples.iter().any(|sample| !sample.is_finite()) {
            self.reset()?;
            return self.push_capture_samples(
                &vec![0.0; packet.samples.len()],
                packet.sample_rate,
                true,
            );
        }

        if !self.render_ready || self.render_is_stale(packet.timestamp) {
            if self.render_ready {
                self.reset()?;
            }
            return self.push_capture_samples(
                &vec![0.0; packet.samples.len()],
                packet.sample_rate,
                true,
            );
        }

        self.push_capture_samples(&packet.samples, packet.sample_rate, false)
    }

    fn push_capture_samples(
        &mut self,
        samples: &[f32],
        sample_rate: u32,
        force_silence: bool,
    ) -> Result<(), String> {
        let converted = resample_packet(&mut self.capture_resampler, sample_rate, samples);
        if force_silence {
            self.forced_silence_samples += converted.len();
        }
        self.capture_buffer.extend(converted);

        while self.capture_buffer.len() >= AEC_FRAME_SAMPLES {
            let frame: Vec<f32> = self.capture_buffer.drain(..AEC_FRAME_SAMPLES).collect();
            if self.forced_silence_samples > 0 {
                self.forced_silence_samples = self
                    .forced_silence_samples
                    .saturating_sub(AEC_FRAME_SAMPLES);
                self.emit_silence_frames(1)?;
                continue;
            }

            let mut output = vec![0.0; AEC_FRAME_SAMPLES];
            match self.pipeline.process_capture_frame(&frame, &mut output) {
                Ok(true)
                    if output.len() == AEC_FRAME_SAMPLES
                        && output.iter().all(|sample| sample.is_finite()) =>
                {
                    self.send_processed_frame(&output)?;
                }
                Ok(false) => {
                    self.emit_silence_frames(1)?;
                    self.reset()?;
                }
                Ok(true) => {
                    self.emit_silence_frames(1)?;
                    self.reset()?;
                }
                Err(error) => {
                    self.emit_silence_frames(1)?;
                    self.reset()?;
                    return Err(format!("AEC capture processing failed: {error}"));
                }
            }
        }
        Ok(())
    }

    fn render_is_stale(&self, capture_time: StreamInstant) -> bool {
        let Some(render_end) = self.render_end else {
            return true;
        };
        if let Some(age) = capture_time.duration_since(&render_end) {
            age > MAX_REFERENCE_AGE
        } else {
            render_end
                .duration_since(&capture_time)
                .is_none_or(|future| future > MAX_REFERENCE_AGE)
        }
    }

    fn send_processed_frame(&self, samples: &[f32]) -> Result<(), String> {
        let mut encoded = Vec::with_capacity(VOICE_AGENT_FRAME_BYTES);
        for pair in samples.chunks_exact(2) {
            let sample = ((pair[0] + pair[1]) * 0.5).clamp(-1.0, 1.0);
            let pcm = if sample <= -1.0 {
                i16::MIN
            } else {
                (sample * i16::MAX as f32).round() as i16
            };
            encoded.extend_from_slice(&pcm.to_le_bytes());
        }
        self.audio_tx
            .try_send(encoded)
            .map_err(|error| format!("AEC audio output queue unavailable: {error}"))
    }

    fn emit_silence_frames(&self, frames: usize) -> Result<(), String> {
        for _ in 0..frames {
            self.audio_tx
                .try_send(vec![0; VOICE_AGENT_FRAME_BYTES])
                .map_err(|error| format!("AEC silence output queue unavailable: {error}"))?;
        }
        Ok(())
    }

    fn reset(&mut self) -> Result<(), String> {
        self.pipeline
            .reset_aec3()
            .map_err(|error| format!("AEC reset failed: {error}"))?;
        self.capture_resampler = None;
        self.render_resampler = None;
        self.capture_buffer.clear();
        self.render_buffer.clear();
        self.forced_silence_samples = 0;
        self.render_ready = false;
        self.render_end = None;
        Ok(())
    }
}

fn resample_packet(
    resampler: &mut Option<StreamingResampler>,
    input_rate: u32,
    samples: &[f32],
) -> Vec<f32> {
    if resampler
        .as_ref()
        .is_none_or(|current| current.input_rate() != input_rate)
    {
        *resampler = Some(StreamingResampler::new(input_rate, AEC_SAMPLE_RATE));
    }
    resampler
        .as_mut()
        .expect("resampler was initialized")
        .push(samples)
}

pub struct StreamingResampler {
    input_rate: u32,
    output_rate: u32,
    input: VecDeque<f32>,
    position: f64,
}

impl StreamingResampler {
    pub fn new(input_rate: u32, output_rate: u32) -> Self {
        Self {
            input_rate,
            output_rate,
            input: VecDeque::new(),
            position: 0.0,
        }
    }

    pub fn input_rate(&self) -> u32 {
        self.input_rate
    }

    pub fn reset(&mut self) {
        self.input.clear();
        self.position = 0.0;
    }

    pub fn push(&mut self, samples: &[f32]) -> Vec<f32> {
        if self.input_rate == 0 || self.output_rate == 0 {
            return Vec::new();
        }
        self.input.extend(samples.iter().copied());
        let step = self.input_rate as f64 / self.output_rate as f64;
        let mut output = Vec::with_capacity(
            ((samples.len() as f64 * self.output_rate as f64 / self.input_rate as f64).ceil()
                as usize)
                .saturating_add(1),
        );

        while self.position + 1.0 < self.input.len() as f64 {
            let index = self.position.floor() as usize;
            let fraction = (self.position - index as f64) as f32;
            let first = self.input[index];
            let second = self.input[index + 1];
            output.push(first + (second - first) * fraction);
            self.position += step;
        }

        let consumed = (self.position.floor() as usize).min(self.input.len().saturating_sub(1));
        for _ in 0..consumed {
            self.input.pop_front();
        }
        self.position -= consumed as f64;
        output
    }
}

pub fn downmix_to_mono<T>(samples: &[T], channels: u16) -> Vec<f32>
where
    T: cpal::Sample,
    f32: cpal::FromSample<T>,
{
    let channels = channels as usize;
    if channels == 0 {
        return Vec::new();
    }
    samples
        .chunks_exact(channels)
        .map(|frame| {
            let sum = frame
                .iter()
                .copied()
                .map(|sample| {
                    let converted: f32 = cpal::Sample::from_sample(sample);
                    converted
                })
                .sum::<f32>();
            sum / channels as f32
        })
        .collect()
}
