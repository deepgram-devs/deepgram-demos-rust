use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Host, SampleFormat, StreamConfig};
use hound::{WavSpec, WavWriter};
use mp3lame_encoder::{Builder, FlushNoGap, InterleavedPcm};
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq)]
enum AudioFormat {
    Wav,
    Mp3,
}

impl AudioFormat {
    fn from_path(path: &str) -> Self {
        let path = Path::new(path);
        match path.extension().and_then(|s| s.to_str()) {
            Some(ext) if ext.eq_ignore_ascii_case("mp3") => AudioFormat::Mp3,
            _ => AudioFormat::Wav,
        }
    }
}

#[derive(Parser)]
#[command(name = "audio-recorder")]
#[command(about = "A cross-platform audio recording application", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List available audio input devices
    ListDevices,
    /// Record audio from an input device
    Record {
        /// Output file path (default: output.wav)
        #[arg(short, long, default_value = "output.wav")]
        output: String,

        /// Specific device name to record from (default: system default input device)
        #[arg(short, long)]
        device: Option<String>,

        /// Duration in seconds (default: records until Ctrl+C)
        #[arg(short = 't', long)]
        duration: Option<u64>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::ListDevices => list_devices(),
        Commands::Record {
            output,
            device,
            duration,
        } => record_audio(&output, device.as_deref(), duration),
    }
}

fn list_devices() -> Result<()> {
    let host = cpal::default_host();
    
    println!("Available audio input devices:\n");
    
    // Get default input device
    let default_device = host.default_input_device();
    let default_name = default_device
        .as_ref()
        .and_then(|d| d.name().ok())
        .unwrap_or_else(|| "Unknown".to_string());
    
    println!("Default Input Device: {}", default_name);
    println!("\nAll Input Devices:");
    
    let devices = host
        .input_devices()
        .context("Failed to get input devices")?;
    
    for (index, device) in devices.enumerate() {
        let name = device.name().unwrap_or_else(|_| "Unknown".to_string());
        let is_default = Some(&name) == default_device.as_ref().and_then(|d| d.name().ok()).as_ref();
        
        if is_default {
            println!("  {}. {} (default)", index + 1, name);
        } else {
            println!("  {}. {}", index + 1, name);
        }
        
        // Try to get supported configurations
        if let Ok(configs) = device.supported_input_configs() {
            for config in configs {
                println!("     - {} channels, {} Hz - {} Hz, {:?}",
                    config.channels(),
                    config.min_sample_rate().0,
                    config.max_sample_rate().0,
                    config.sample_format()
                );
            }
        }
    }
    
    Ok(())
}

fn record_audio(output_path: &str, device_name: Option<&str>, duration: Option<u64>) -> Result<()> {
    let host = cpal::default_host();
    
    // Select the input device
    let device = if let Some(name) = device_name {
        find_device_by_name(&host, name)?
    } else {
        host.default_input_device()
            .context("No default input device available")?
    };
    
    let device_name = device.name().unwrap_or_else(|_| "Unknown".to_string());
    println!("Recording from device: {}", device_name);
    
    // Get the default input config
    let config = device
        .default_input_config()
        .context("Failed to get default input config")?;
    
    println!("Using config: {} channels, {} Hz, {:?}",
        config.channels(),
        config.sample_rate().0,
        config.sample_format()
    );
    
    // Detect output format from file extension
    let format = AudioFormat::from_path(output_path);
    let format_name = match format {
        AudioFormat::Wav => "WAV",
        AudioFormat::Mp3 => "MP3",
    };
    
    println!("Recording to: {} (format: {})", output_path, format_name);
    if let Some(secs) = duration {
        println!("Duration: {} seconds", secs);
    } else {
        println!("Press Ctrl+C to stop recording");
    }
    
    let stream_config: StreamConfig = config.clone().into();
    
    // Start recording based on format and sample format
    match format {
        AudioFormat::Wav => {
            match config.sample_format() {
                SampleFormat::F32 => record_wav_with_format::<f32>(&device, &stream_config, output_path, duration),
                SampleFormat::I16 => record_wav_with_format::<i16>(&device, &stream_config, output_path, duration),
                SampleFormat::U16 => {
                    println!("Note: Converting U16 samples to I16 for WAV file compatibility");
                    record_wav_with_format_u16(&device, &stream_config, output_path, duration)
                },
                _ => Err(anyhow::anyhow!("Unsupported sample format: {:?}", config.sample_format())),
            }
        }
        AudioFormat::Mp3 => {
            match config.sample_format() {
                SampleFormat::F32 => record_mp3_with_format_f32(&device, &stream_config, output_path, duration),
                SampleFormat::I16 => record_mp3_with_format_i16(&device, &stream_config, output_path, duration),
                SampleFormat::U16 => record_mp3_with_format_u16(&device, &stream_config, output_path, duration),
                _ => Err(anyhow::anyhow!("Unsupported sample format: {:?}", config.sample_format())),
            }
        }
    }
}

fn find_device_by_name(host: &Host, name: &str) -> Result<Device> {
    let devices = host
        .input_devices()
        .context("Failed to get input devices")?;
    
    for device in devices {
        if let Ok(device_name) = device.name() {
            if device_name.to_lowercase().contains(&name.to_lowercase()) {
                return Ok(device);
            }
        }
    }
    
    Err(anyhow::anyhow!("Device '{}' not found", name))
}

fn record_wav_with_format<T>(
    device: &Device,
    config: &StreamConfig,
    output_path: &str,
    duration: Option<u64>,
) -> Result<()>
where
    T: cpal::Sample + cpal::SizedSample + hound::Sample,
{
    let spec = WavSpec {
        channels: config.channels,
        sample_rate: config.sample_rate.0,
        bits_per_sample: (std::mem::size_of::<T>() * 8) as u16,
        sample_format: if std::mem::size_of::<T>() == 4 {
            hound::SampleFormat::Float
        } else {
            hound::SampleFormat::Int
        },
    };
    
    let writer = WavWriter::create(output_path, spec)
        .context("Failed to create WAV file")?;
    let writer = Arc::new(Mutex::new(Some(writer)));
    
    let writer_clone = writer.clone();
    let start_time = std::time::Instant::now();
    
    let err_fn = |err| eprintln!("Error during recording: {}", err);
    
    let stream = device.build_input_stream(
        config,
        move |data: &[T], _: &cpal::InputCallbackInfo| {
            // Check if we should stop based on duration
            if let Some(max_duration) = duration {
                if start_time.elapsed().as_secs() >= max_duration {
                    return;
                }
            }
            
            if let Ok(mut guard) = writer_clone.lock() {
                if let Some(writer) = guard.as_mut() {
                    for &sample in data {
                        let _ = writer.write_sample(sample);
                    }
                }
            }
        },
        err_fn,
        None,
    )
    .context("Failed to build input stream")?;
    
    stream.play().context("Failed to start recording")?;
    
    // If duration is specified, wait for that duration
    if let Some(secs) = duration {
        std::thread::sleep(std::time::Duration::from_secs(secs));
    } else {
        // Otherwise, wait for Ctrl+C
        println!("\nRecording... Press Ctrl+C to stop");
        let (tx, rx) = std::sync::mpsc::channel();
        ctrlc::set_handler(move || {
            let _ = tx.send(());
        })
        .context("Error setting Ctrl+C handler")?;
        
        rx.recv().context("Error waiting for Ctrl+C")?;
    }
    
    drop(stream);
    
    // Finalize the WAV file
    if let Ok(mut guard) = writer.lock() {
        if let Some(writer) = guard.take() {
            writer.finalize().context("Failed to finalize WAV file")?;
        }
    }
    
    println!("\nRecording saved to: {}", output_path);
    Ok(())
}

fn record_mp3_with_format_i16(
    device: &Device,
    config: &StreamConfig,
    output_path: &str,
    duration: Option<u64>,
) -> Result<()> {

    println!("MP3 encoding: Using i16 format");

    // Create MP3 encoder
    let mut builder = Builder::new().expect("Failed to create MP3 encoder");
    builder.set_num_channels(1).expect("Failed to set channels");
    builder.set_sample_rate(config.sample_rate.0/2).expect("Failed to set sample rate");
    builder.set_brate(mp3lame_encoder::Bitrate::Kbps64).expect("Failed to set bitrate");
    builder.set_quality(mp3lame_encoder::Quality::VeryNice).expect("Failed to set quality");
    builder.set_mode(mp3lame_encoder::Mode::Mono).expect("Failed to set channel mode");
    let mut encoder = builder.build().expect("Failed to build MP3 encoder");
    
    let mut file = File::create(output_path)
        .context("Failed to create MP3 file")?;
    
    let buffer = Arc::new(Mutex::new(Vec::new()));
    let buffer_clone = buffer.clone();
    let start_time = std::time::Instant::now();
    
    let err_fn = |err| eprintln!("Error during recording: {}", err);
    
    let stream = device.build_input_stream(
        config,
        move |data: &[i16], _: &cpal::InputCallbackInfo| {
            if let Some(max_duration) = duration {
                if start_time.elapsed().as_secs() >= max_duration {
                    return;
                }
            }
            
            if let Ok(mut guard) = buffer_clone.lock() {
                guard.extend_from_slice(data);
            }
        },
        err_fn,
        None,
    )
    .context("Failed to build input stream")?;
    
    stream.play().context("Failed to start recording")?;
    
    if let Some(secs) = duration {
        std::thread::sleep(std::time::Duration::from_secs(secs));
    } else {
        println!("\nRecording... Press Ctrl+C to stop");
        let (tx, rx) = std::sync::mpsc::channel();
        ctrlc::set_handler(move || {
            let _ = tx.send(());
        })
        .context("Error setting Ctrl+C handler")?;
        
        rx.recv().context("Error waiting for Ctrl+C")?;
    }
    
    drop(stream);
    
    // Encode to MP3
    let samples = buffer.lock().unwrap();
    let input = InterleavedPcm(samples.as_slice());
    let mut mp3_buffer = Vec::new();
    mp3_buffer.reserve(mp3lame_encoder::max_required_buffer_size(samples.len()));
    
    let encoded_size = encoder.encode(input, mp3_buffer.spare_capacity_mut())
        .map_err(|e| anyhow::anyhow!("Failed to encode MP3: {:?}", e))?;
    unsafe { mp3_buffer.set_len(encoded_size); }
    
    let encoded_size = encoder.flush::<FlushNoGap>(mp3_buffer.spare_capacity_mut())
        .map_err(|e| anyhow::anyhow!("Failed to flush MP3 encoder: {:?}", e))?;
    unsafe { mp3_buffer.set_len(mp3_buffer.len() + encoded_size); }
    
    file.write_all(&mp3_buffer)
        .context("Failed to write MP3 file")?;
    
    println!("\nRecording saved to: {}", output_path);
    Ok(())
}

fn record_mp3_with_format_f32(
    device: &Device,
    config: &StreamConfig,
    output_path: &str,
    duration: Option<u64>,
) -> Result<()> {

    println!("MP3 encoding: Using f32 format");

    // Create MP3 encoder
    let mut builder = Builder::new().expect("Failed to create MP3 encoder");
    builder.set_num_channels(1).expect("Failed to set channels");
    builder.set_sample_rate(config.sample_rate.0/2).expect("Failed to set sample rate");
    builder.set_brate(mp3lame_encoder::Bitrate::Kbps64).expect("Failed to set bitrate");
    builder.set_quality(mp3lame_encoder::Quality::VeryNice).expect("Failed to set quality");
    let mut encoder = builder.build().expect("Failed to build MP3 encoder");
    
    let mut file = File::create(output_path)
        .context("Failed to create MP3 file")?;
    
    let buffer = Arc::new(Mutex::new(Vec::new()));
    let buffer_clone = buffer.clone();
    let start_time = std::time::Instant::now();
    
    let err_fn = |err| eprintln!("Error during recording: {}", err);
    
    let stream = device.build_input_stream(
        config,
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            if let Some(max_duration) = duration {
                if start_time.elapsed().as_secs() >= max_duration {
                    return;
                }
            }
            
            if let Ok(mut guard) = buffer_clone.lock() {
                // Convert f32 samples to i16
                for &sample in data {
                    let sample_i16 = (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
                    guard.push(sample_i16);
                }
            }
        },
        err_fn,
        None,
    )
    .context("Failed to build input stream")?;
    
    stream.play().context("Failed to start recording")?;
    
    if let Some(secs) = duration {
        std::thread::sleep(std::time::Duration::from_secs(secs));
    } else {
        println!("\nRecording... Press Ctrl+C to stop");
        let (tx, rx) = std::sync::mpsc::channel();
        ctrlc::set_handler(move || {
            let _ = tx.send(());
        })
        .context("Error setting Ctrl+C handler")?;
        
        rx.recv().context("Error waiting for Ctrl+C")?;
    }
    
    drop(stream);
    
    // Encode to MP3
    let samples = buffer.lock().unwrap();
    let input = InterleavedPcm(samples.as_slice());
    let mut mp3_buffer = Vec::new();
    mp3_buffer.reserve(mp3lame_encoder::max_required_buffer_size(samples.len()));
    
    let encoded_size = encoder.encode(input, mp3_buffer.spare_capacity_mut())
        .map_err(|e| anyhow::anyhow!("Failed to encode MP3: {:?}", e))?;
    unsafe { mp3_buffer.set_len(encoded_size); }
    
    let encoded_size = encoder.flush::<FlushNoGap>(mp3_buffer.spare_capacity_mut())
        .map_err(|e| anyhow::anyhow!("Failed to flush MP3 encoder: {:?}", e))?;
    unsafe { mp3_buffer.set_len(mp3_buffer.len() + encoded_size); }
    
    file.write_all(&mp3_buffer)
        .context("Failed to write MP3 file")?;
    
    println!("\nRecording saved to: {}", output_path);
    Ok(())
}

fn record_mp3_with_format_u16(
    device: &Device,
    config: &StreamConfig,
    output_path: &str,
    duration: Option<u64>,
) -> Result<()> {
    
    println!("MP3 encoding: Using u16 format");

    // Create MP3 encoder
    let mut builder = Builder::new().expect("Failed to create MP3 encoder");
    builder.set_num_channels(1).expect("Failed to set channels");
    builder.set_sample_rate(config.sample_rate.0/2).expect("Failed to set sample rate");
    builder.set_brate(mp3lame_encoder::Bitrate::Kbps64).expect("Failed to set bitrate");
    builder.set_quality(mp3lame_encoder::Quality::VeryNice).expect("Failed to set quality");
    let mut encoder = builder.build().expect("Failed to build MP3 encoder");
    
    let mut file = File::create(output_path)
        .context("Failed to create MP3 file")?;
    
    let buffer = Arc::new(Mutex::new(Vec::new()));
    let buffer_clone = buffer.clone();
    let start_time = std::time::Instant::now();
    
    let err_fn = |err| eprintln!("Error during recording: {}", err);
    
    let stream = device.build_input_stream(
        config,
        move |data: &[u16], _: &cpal::InputCallbackInfo| {
            if let Some(max_duration) = duration {
                if start_time.elapsed().as_secs() >= max_duration {
                    return;
                }
            }
            
            if let Ok(mut guard) = buffer_clone.lock() {
                // Convert u16 to i16
                for &sample in data {
                    let converted = (sample as i32 - 32768) as i16;
                    guard.push(converted);
                }
            }
        },
        err_fn,
        None,
    )
    .context("Failed to build input stream")?;
    
    stream.play().context("Failed to start recording")?;
    
    if let Some(secs) = duration {
        std::thread::sleep(std::time::Duration::from_secs(secs));
    } else {
        println!("\nRecording... Press Ctrl+C to stop");
        let (tx, rx) = std::sync::mpsc::channel();
        ctrlc::set_handler(move || {
            let _ = tx.send(());
        })
        .context("Error setting Ctrl+C handler")?;
        
        rx.recv().context("Error waiting for Ctrl+C")?;
    }
    
    drop(stream);
    
    // Encode to MP3
    let samples = buffer.lock().unwrap();
    let input = InterleavedPcm(samples.as_slice());
    let mut mp3_buffer = Vec::new();
    mp3_buffer.reserve(mp3lame_encoder::max_required_buffer_size(samples.len()));
    
    let encoded_size = encoder.encode(input, mp3_buffer.spare_capacity_mut())
        .map_err(|e| anyhow::anyhow!("Failed to encode MP3: {:?}", e))?;
    unsafe { mp3_buffer.set_len(encoded_size); }
    
    let encoded_size = encoder.flush::<FlushNoGap>(mp3_buffer.spare_capacity_mut())
        .map_err(|e| anyhow::anyhow!("Failed to flush MP3 encoder: {:?}", e))?;
    unsafe { mp3_buffer.set_len(mp3_buffer.len() + encoded_size); }
    
    file.write_all(&mp3_buffer)
        .context("Failed to write MP3 file")?;
    
    println!("\nRecording saved to: {}", output_path);
    Ok(())
}

fn record_wav_with_format_u16(
    device: &Device,
    config: &StreamConfig,
    output_path: &str,
    duration: Option<u64>,
) -> Result<()> {
    let spec = WavSpec {
        channels: config.channels,
        sample_rate: config.sample_rate.0,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    
    let writer = WavWriter::create(output_path, spec)
        .context("Failed to create WAV file")?;
    let writer = Arc::new(Mutex::new(Some(writer)));
    
    let writer_clone = writer.clone();
    let start_time = std::time::Instant::now();
    
    let err_fn = |err| eprintln!("Error during recording: {}", err);
    
    let stream = device.build_input_stream(
        config,
        move |data: &[u16], _: &cpal::InputCallbackInfo| {
            // Check if we should stop based on duration
            if let Some(max_duration) = duration {
                if start_time.elapsed().as_secs() >= max_duration {
                    return;
                }
            }
            
            if let Ok(mut guard) = writer_clone.lock() {
                if let Some(writer) = guard.as_mut() {
                    for &sample in data {
                        // Convert u16 to i16 by subtracting 32768
                        let converted = (sample as i32 - 32768) as i16;
                        let _ = writer.write_sample(converted);
                    }
                }
            }
        },
        err_fn,
        None,
    )
    .context("Failed to build input stream")?;
    
    stream.play().context("Failed to start recording")?;
    
    // If duration is specified, wait for that duration
    if let Some(secs) = duration {
        std::thread::sleep(std::time::Duration::from_secs(secs));
    } else {
        // Otherwise, wait for Ctrl+C
        println!("\nRecording... Press Ctrl+C to stop");
        let (tx, rx) = std::sync::mpsc::channel();
        ctrlc::set_handler(move || {
            let _ = tx.send(());
        })
        .context("Error setting Ctrl+C handler")?;
        
        rx.recv().context("Error waiting for Ctrl+C")?;
    }
    
    drop(stream);
    
    // Finalize the WAV file
    if let Ok(mut guard) = writer.lock() {
        if let Some(writer) = guard.take() {
            writer.finalize().context("Failed to finalize WAV file")?;
        }
    }
    
    println!("\nRecording saved to: {}", output_path);
    Ok(())
}
