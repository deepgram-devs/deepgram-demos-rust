use std::mem::size_of;

use windows::core::PSTR;
use windows::Win32::Media::Audio::*;

// 100 ms of audio at 48000 Hz, 16-bit mono
const BUF_BYTES: usize = 9_600;
const NUM_BUFS: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioInputDevice {
    pub id: Option<u32>,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct ResolvedAudioInputDevice {
    pub requested_name: Option<String>,
    pub actual_name: String,
    pub device_id: Option<u32>,
    pub fell_back_to_default: bool,
}

/// Holds an open waveIn device with pre-allocated, pre-pinned buffers.
/// Call `start()` at the beginning of each recording and `stop()` at the end.
/// The device stays open between recordings so there is no re-initialization
/// delay when the user triggers the hotkey.
pub struct AudioCapture {
    hwi: HWAVEIN,
    pub actual_device: ResolvedAudioInputDevice,
    // Boxed so their heap addresses are stable; hdrs holds raw pointers into them.
    bufs: Vec<Box<[u8; BUF_BYTES]>>,
    hdrs: Vec<WAVEHDR>,
}

// HWAVEIN is a Windows handle — safe to send across threads.
unsafe impl Send for AudioCapture {}

impl AudioCapture {
    pub fn new(selected_name: Option<&str>) -> Option<Self> {
        unsafe {
            let format = WAVEFORMATEX {
                wFormatTag: WAVE_FORMAT_PCM as u16,
                nChannels: 1,
                nSamplesPerSec: 48_000,
                nAvgBytesPerSec: 96_000,
                nBlockAlign: 2,
                wBitsPerSample: 16,
                cbSize: 0,
            };

            let resolved = resolve_input_device(selected_name);
            let mut hwi = std::mem::zeroed::<HWAVEIN>();
            let r = waveInOpen(
                Some(&mut hwi),
                resolved.device_id.unwrap_or(WAVE_MAPPER),
                &format,
                Some(0),
                Some(0),
                CALLBACK_NULL,
            );
            if r != 0 {
                crate::logger::verbose(&format!("waveInOpen failed: {r}"));
                return None;
            }

            let mut bufs: Vec<Box<[u8; BUF_BYTES]>> =
                (0..NUM_BUFS).map(|_| Box::new([0u8; BUF_BYTES])).collect();

            // Build headers pointing at the stable Box allocations. These pointers
            // remain valid as long as `bufs` is not resized or dropped.
            let hdrs: Vec<WAVEHDR> = bufs
                .iter_mut()
                .map(|b| WAVEHDR {
                    lpData: PSTR(b.as_mut_ptr()),
                    dwBufferLength: BUF_BYTES as u32,
                    ..std::mem::zeroed()
                })
                .collect();

            Some(AudioCapture {
                hwi,
                actual_device: resolved,
                bufs,
                hdrs,
            })
        }
    }

    /// Prepares buffers and starts the device. Call immediately when recording begins.
    pub fn start(&mut self) {
        unsafe {
            for (buf, hdr) in self.bufs.iter_mut().zip(self.hdrs.iter_mut()) {
                // Reset to a clean state while preserving the data pointer.
                *hdr = WAVEHDR {
                    lpData: PSTR(buf.as_mut_ptr()),
                    dwBufferLength: BUF_BYTES as u32,
                    ..std::mem::zeroed()
                };
                waveInPrepareHeader(self.hwi, hdr, size_of::<WAVEHDR>() as u32);
                waveInAddBuffer(self.hwi, hdr, size_of::<WAVEHDR>() as u32);
            }
            waveInStart(self.hwi);
        }
    }

    /// Drains any filled buffers into `out` and re-queues them. Call in a polling loop.
    pub fn collect_ready(&mut self, out: &mut Vec<i16>) {
        unsafe {
            for (buf, hdr) in self.bufs.iter_mut().zip(self.hdrs.iter_mut()) {
                if hdr.dwFlags & 1 == 0 {
                    // WHDR_DONE not set yet
                    continue;
                }
                let n = hdr.dwBytesRecorded as usize / 2;
                if n > 0 {
                    let ptr = hdr.lpData.0 as *const i16;
                    out.extend_from_slice(std::slice::from_raw_parts(ptr, n));
                }
                // Unprep, reset, re-prepare, re-queue.
                waveInUnprepareHeader(self.hwi, hdr, size_of::<WAVEHDR>() as u32);
                *hdr = WAVEHDR {
                    lpData: PSTR(buf.as_mut_ptr()),
                    dwBufferLength: BUF_BYTES as u32,
                    ..std::mem::zeroed()
                };
                waveInPrepareHeader(self.hwi, hdr, size_of::<WAVEHDR>() as u32);
                waveInAddBuffer(self.hwi, hdr, size_of::<WAVEHDR>() as u32);
            }
        }
    }

    /// Stops the device and collects any remaining samples into `out`.
    pub fn stop(&mut self, out: &mut Vec<i16>) {
        unsafe {
            waveInStop(self.hwi);
            waveInReset(self.hwi); // forces all pending buffers to DONE

            for hdr in self.hdrs.iter_mut() {
                if hdr.dwBytesRecorded > 0 {
                    let n = hdr.dwBytesRecorded as usize / 2;
                    let ptr = hdr.lpData.0 as *const i16;
                    out.extend_from_slice(std::slice::from_raw_parts(ptr, n));
                }
                waveInUnprepareHeader(self.hwi, hdr, size_of::<WAVEHDR>() as u32);
            }
        }
    }
}

impl Drop for AudioCapture {
    fn drop(&mut self) {
        unsafe { waveInClose(self.hwi); }
    }
}

/// Wraps raw i16 PCM samples into a WAV byte buffer.
pub fn encode_wav(samples: &[i16], sample_rate: u32) -> Vec<u8> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(&mut buf, spec).expect("wav writer");
        for &s in samples {
            writer.write_sample(s).expect("write sample");
        }
        writer.finalize().expect("wav finalize");
    }
    buf.into_inner()
}

pub fn list_input_devices() -> Vec<AudioInputDevice> {
    let mut devices = vec![AudioInputDevice {
        id: None,
        name: "Default system input".to_string(),
    }];

    unsafe {
        let count = waveInGetNumDevs();
        for id in 0..count {
            let mut caps = WAVEINCAPSW::default();
            if waveInGetDevCapsW(id as usize, &mut caps, size_of::<WAVEINCAPSW>() as u32) == 0 {
                let name = std::ptr::addr_of!(caps.szPname).read_unaligned();
                devices.push(AudioInputDevice {
                    id: Some(id),
                    name: wide_to_string(&name),
                });
            }
        }
    }

    devices
}

pub fn resolve_input_device(selected_name: Option<&str>) -> ResolvedAudioInputDevice {
    let requested_name = selected_name
        .map(|name| name.trim())
        .filter(|name| !name.is_empty())
        .map(|name| name.to_string());

    let devices = list_input_devices();
    if let Some(requested) = &requested_name {
        if let Some(device) = devices
            .iter()
            .find(|device| device.name.eq_ignore_ascii_case(requested))
        {
            return ResolvedAudioInputDevice {
                requested_name,
                actual_name: device.name.clone(),
                device_id: device.id,
                fell_back_to_default: false,
            };
        }
    }

    let fallback = devices
        .into_iter()
        .next()
        .unwrap_or(AudioInputDevice {
            id: None,
            name: "Default system input".to_string(),
        });

    ResolvedAudioInputDevice {
        requested_name,
        actual_name: fallback.name,
        device_id: fallback.id,
        fell_back_to_default: selected_name
            .map(|name| !name.trim().is_empty())
            .unwrap_or(false),
    }
}

fn wide_to_string(wide: &[u16]) -> String {
    let end = wide.iter().position(|c| *c == 0).unwrap_or(wide.len());
    String::from_utf16_lossy(&wide[..end])
}

pub fn peak_level(samples: &[i16]) -> u16 {
    samples
        .iter()
        .map(|sample| sample.unsigned_abs())
        .max()
        .unwrap_or(0)
}

pub fn peak_level_percent(samples: &[i16]) -> u8 {
    let peak = peak_level(samples) as f32;
    ((peak / i16::MAX as f32) * 100.0).clamp(0.0, 100.0) as u8
}

pub struct AudioMeter {
    capture: AudioCapture,
    scratch: Vec<i16>,
}

impl AudioMeter {
    pub fn new(selected_name: Option<&str>) -> Option<Self> {
        let mut capture = AudioCapture::new(selected_name)?;
        capture.start();
        Some(Self {
            capture,
            scratch: Vec::new(),
        })
    }

    pub fn sample_level(&mut self) -> u8 {
        self.scratch.clear();
        self.capture.collect_ready(&mut self.scratch);
        peak_level_percent(&self.scratch)
    }
}

impl Drop for AudioMeter {
    fn drop(&mut self) {
        let mut drain = Vec::new();
        self.capture.stop(&mut drain);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peak_level_percent_handles_empty_samples() {
        assert_eq!(peak_level_percent(&[]), 0);
    }

    #[test]
    fn peak_level_percent_scales_samples() {
        let samples = [0i16, i16::MAX / 2];
        assert!(peak_level_percent(&samples) >= 49);
    }
}
