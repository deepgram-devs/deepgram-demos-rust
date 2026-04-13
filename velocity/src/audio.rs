use std::mem::size_of;

use windows::core::PSTR;
use windows::Win32::Media::Audio::*;

// 100 ms of audio at 48000 Hz, 16-bit mono
const BUF_BYTES: usize = 9_600;
const NUM_BUFS: usize = 4;

/// Holds an open waveIn device with pre-allocated, pre-pinned buffers.
/// Call `start()` at the beginning of each recording and `stop()` at the end.
/// The device stays open between recordings so there is no re-initialization
/// delay when the user triggers the hotkey.
pub struct AudioCapture {
    hwi: HWAVEIN,
    // Boxed so their heap addresses are stable; hdrs holds raw pointers into them.
    bufs: Vec<Box<[u8; BUF_BYTES]>>,
    hdrs: Vec<WAVEHDR>,
}

// HWAVEIN is a Windows handle — safe to send across threads.
unsafe impl Send for AudioCapture {}

impl AudioCapture {
    pub fn new() -> Option<Self> {
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

            let mut hwi = std::mem::zeroed::<HWAVEIN>();
            let r = waveInOpen(
                Some(&mut hwi),
                WAVE_MAPPER,
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

            Some(AudioCapture { hwi, bufs, hdrs })
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
