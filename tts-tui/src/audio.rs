use anyhow::{Context, Result};
use rodio::mixer::Mixer;
use rodio::{DeviceSinkBuilder, MixerDeviceSink};

/// Owns the OS audio stream and the mixer used by playback players.
///
/// The stream must stay alive for the lifetime of the application. Keeping it
/// here prevents the output device from being reopened for every utterance.
pub struct AudioOutput {
    pub _device: MixerDeviceSink,
    pub mixer: Mixer,
}

impl AudioOutput {
    pub fn open_default() -> Result<Self> {
        let device = DeviceSinkBuilder::open_default_sink()
            .context("Failed to open the default audio output device")?;
        let mixer = device.mixer().clone();
        Ok(Self {
            _device: device,
            mixer,
        })
    }
}
