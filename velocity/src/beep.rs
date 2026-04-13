use minimp3::{Decoder, Error, Frame};
use windows::core::PSTR;
use windows::Win32::Media::Audio::*;

static START_MP3: &[u8] = include_bytes!("../assets/record-start.mp3");
static END_MP3: &[u8] = include_bytes!("../assets/record-end.mp3");

pub fn play_start() {
    play_mp3(START_MP3);
}

pub fn play_end() {
    play_mp3(END_MP3);
}

fn play_mp3(data: &'static [u8]) {
    let mut decoder = Decoder::new(std::io::Cursor::new(data));
    let mut samples: Vec<i16> = Vec::new();
    let mut sample_rate = 48_000u32;
    let mut channels = 1u16;

    loop {
        match decoder.next_frame() {
            Ok(Frame {
                data: frame_samples,
                sample_rate: sr,
                channels: ch,
                ..
            }) => {
                sample_rate = sr as u32;
                channels = ch as u16;
                samples.extend_from_slice(&frame_samples);
            }
            Err(Error::Eof) => break,
            Err(_) => break,
        }
    }

    if samples.is_empty() {
        return;
    }

    unsafe { play_pcm(&mut samples, sample_rate, channels) };
}

unsafe fn play_pcm(samples: &mut Vec<i16>, sample_rate: u32, channels: u16) {
    let block_align = channels * 2;
    let format = WAVEFORMATEX {
        wFormatTag: WAVE_FORMAT_PCM as u16,
        nChannels: channels,
        nSamplesPerSec: sample_rate,
        nAvgBytesPerSec: sample_rate * block_align as u32,
        nBlockAlign: block_align,
        wBitsPerSample: 16,
        cbSize: 0,
    };

    let mut hwo = std::mem::zeroed::<HWAVEOUT>();
    let r = waveOutOpen(
        Some(&mut hwo),
        WAVE_MAPPER,
        &format,
        Some(0),
        Some(0),
        CALLBACK_NULL,
    );
    if r != 0 {
        return;
    }

    let byte_len = (samples.len() * 2) as u32;
    let mut hdr = WAVEHDR {
        lpData: PSTR(samples.as_mut_ptr() as *mut u8),
        dwBufferLength: byte_len,
        ..std::mem::zeroed()
    };

    waveOutPrepareHeader(hwo, &mut hdr, std::mem::size_of::<WAVEHDR>() as u32);
    waveOutWrite(hwo, &mut hdr, std::mem::size_of::<WAVEHDR>() as u32);

    // Poll until the buffer is marked done (WHDR_DONE = bit 0)
    while hdr.dwFlags & 1 == 0 {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }

    waveOutUnprepareHeader(hwo, &mut hdr, std::mem::size_of::<WAVEHDR>() as u32);
    waveOutClose(hwo);
}
