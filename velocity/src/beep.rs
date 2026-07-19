use windows::Win32::Media::Audio::*;
use windows::core::PSTR;

static START_WAV: &[u8] = include_bytes!("../assets/record-start.wav");
static END_WAV: &[u8] = include_bytes!("../assets/record-end.wav");

pub fn play_start() {
    play_wav(START_WAV);
}

pub fn play_end() {
    play_wav(END_WAV);
}

fn play_wav(data: &'static [u8]) {
    let Some((mut samples, sample_rate, channels)) = decode_wav(data) else {
        return;
    };

    if samples.is_empty() {
        return;
    }

    unsafe { play_pcm(&mut samples, sample_rate, channels) };
}

fn decode_wav(data: &'static [u8]) -> Option<(Vec<i16>, u32, u16)> {
    let cursor = std::io::Cursor::new(data);
    let mut reader = hound::WavReader::new(cursor).ok()?;
    let spec = reader.spec();
    if spec.sample_format != hound::SampleFormat::Int || spec.bits_per_sample != 16 {
        return None;
    }

    let samples = reader
        .samples::<i16>()
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    Some((samples, spec.sample_rate, spec.channels))
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
