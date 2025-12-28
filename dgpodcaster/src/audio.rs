use anyhow::Result;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

pub fn concatenate_audio_files(input_files: &[PathBuf], output_file: &str) -> Result<()> {
    let mut all_samples: Vec<i16> = Vec::new();

    for file_path in input_files {
        let mut file = File::open(file_path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        let samples: Vec<i16> = buffer
            .chunks_exact(2)
            .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();

        all_samples.extend(samples);
    }

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = hound::WavWriter::create(output_file, spec)?;

    for sample in all_samples {
        writer.write_sample(sample)?;
    }

    writer.finalize()?;

    std::fs::remove_dir_all("temp_audio")?;

    Ok(())
}
