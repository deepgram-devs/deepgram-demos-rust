use webrtc_vad::{Vad, SampleRate, VadMode};

// webrtc-vad requires 16 kHz; our capture runs at 48 kHz (3× ratio).
const CAPTURE_HZ: usize = 48_000;
const VAD_HZ: usize = 16_000;
const DOWNSAMPLE_RATIO: usize = CAPTURE_HZ / VAD_HZ;

// 10 ms frames at 16 kHz = 160 samples.
const FRAME_SAMPLES_16K: usize = VAD_HZ / 100;

pub struct VadTracker {
    vad: Vad,
    /// Accumulates downsampled samples until we have a full 10 ms frame.
    pending: Vec<i16>,
    /// Consecutive milliseconds of non-voice detected.
    silence_ms: u32,
    /// How many ms of silence trigger auto-stop.
    threshold_ms: u32,
}

impl VadTracker {
    pub fn new(threshold_ms: u32) -> Self {
        let mut vad = Vad::new_with_rate_and_mode(SampleRate::Rate16kHz, VadMode::Quality);
        // Aggressive mode reduces false positives in quiet environments.
        vad.set_mode(VadMode::Aggressive);
        Self {
            vad,
            pending: Vec::with_capacity(FRAME_SAMPLES_16K * 2),
            silence_ms: 0,
            threshold_ms,
        }
    }

    /// Feed a chunk of 48 kHz i16 samples captured during one poll cycle.
    /// Returns `true` when accumulated silence exceeds `threshold_ms`.
    pub fn push(&mut self, samples: &[i16]) -> bool {
        // Downsample by taking every Nth sample (simple decimation).
        // For speech VAD purposes this is sufficient; a low-pass filter before
        // decimation would be more correct but adds complexity and latency.
        self.pending.extend(samples.iter().step_by(DOWNSAMPLE_RATIO).copied());

        while self.pending.len() >= FRAME_SAMPLES_16K {
            let frame: Vec<i16> = self.pending.drain(..FRAME_SAMPLES_16K).collect();
            let is_voice = self.vad.is_voice_segment(&frame).unwrap_or(true);
            if is_voice {
                self.silence_ms = 0;
            } else {
                // Each frame is exactly 10 ms.
                self.silence_ms += 10;
                if self.silence_ms >= self.threshold_ms {
                    return true;
                }
            }
        }

        false
    }

    pub fn reset(&mut self) {
        self.silence_ms = 0;
        self.pending.clear();
    }
}
