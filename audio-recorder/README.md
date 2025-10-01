# Audio Recorder

A cross-platform audio recording application built with Rust that supports recording from microphone and other audio input devices.

## Features

- **Cross-platform support**: Works on Windows, macOS, and Linux
- **List audio devices**: View all available audio input devices on your system
- **Record audio**: Record from the default microphone or specify a custom device
- **Flexible output**: Save recordings as WAV files
- **Duration control**: Record for a specific duration or until manually stopped
- **Device selection**: Choose which audio input device to record from

## Installation

Build the application using Cargo:

```bash
cargo build --release
```

## Usage

### List Available Audio Devices

To see all available audio input devices on your system:

```bash
cargo run -- list-devices
```

This will display:

- The default input device
- All available input devices with their supported configurations
- Sample rates and channel information

Example output:

```text
Available audio input devices:

Default Input Device: MacBook Pro Microphone

All Input Devices:
  1. MacBook Pro Microphone (default)
     - 1 channels, 8000 Hz - 96000 Hz, I16
     - 1 channels, 8000 Hz - 96000 Hz, F32
  2. External USB Microphone
     - 2 channels, 44100 Hz - 48000 Hz, I16
```

### Record Audio

#### Record from Default Device

Record from the default microphone until you press Ctrl+C:

```bash
cargo run -- record
```

#### Specify Output File

Record to a specific file:

```bash
cargo run -- record --output my-recording.wav
```

Or using the short form:

```bash
cargo run -- record -o my-recording.wav
```

#### Record for a Specific Duration

Record for 10 seconds:

```bash
cargo run -- record --duration 10
```

Or using the short form:

```bash
cargo run -- record -t 10
```

#### Record from a Specific Device

Record from a specific audio input device (partial name matching):

```bash
cargo run -- record --device "USB Microphone"
```

Or using the short form:

```bash
cargo run -- record -d "USB"
```

#### Combine Options

Record from a specific device for 30 seconds to a custom file:

```bash
cargo run -- record -d "USB" -o recording.wav -t 30
```

## Command Reference

### `list-devices`

Lists all available audio input devices.

```bash
audio-recorder list-devices
```

### `record`

Records audio from an input device.

```bash
audio-recorder record [OPTIONS]
```

**Options:**
- `-o, --output <FILE>` - Output file path (default: output.wav)
- `-d, --device <NAME>` - Specific device name to record from (default: system default)
- `-t, --duration <SECONDS>` - Duration in seconds (default: records until Ctrl+C)
- `-h, --help` - Print help information

## Technical Details

- **Audio Format**: WAV (PCM)
- **Supported Sample Formats**: F32 (32-bit float), I16 (16-bit integer), U16 (16-bit unsigned, converted to I16)
- **Sample Rate**: Uses the device's default sample rate
- **Channels**: Uses the device's default channel configuration

## Dependencies

- `cpal` - Cross-platform audio I/O library
- `hound` - WAV encoding/decoding
- `clap` - Command-line argument parsing
- `anyhow` - Error handling
- `ctrlc` - Signal handling for graceful shutdown

## Platform-Specific Notes

### macOS

- May require microphone permissions in System Preferences
- Uses CoreAudio backend

### Windows

- Uses WASAPI backend
- May require audio device permissions

### Linux

- Uses ALSA or PulseAudio backend
- Ensure audio drivers are properly installed

## Examples

### Quick 5-second test recording

```bash
cargo run -- record -t 5 -o test.wav
```

### Record from external microphone

```bash
# First, list devices to find the exact name
cargo run -- list-devices

# Then record from the specific device
cargo run -- record -d "External" -o external-recording.wav
```

### Long recording session

```bash
# Record until manually stopped with Ctrl+C
cargo run -- record -o long-session.wav
```

## Troubleshooting

**No audio devices found:**

- Ensure your microphone is properly connected
- Check system audio settings
- Verify audio drivers are installed

**Permission denied:**

- On macOS: Grant microphone access in System Preferences > Security & Privacy
- On Linux: Ensure your user is in the `audio` group

**Recording is silent:**

- Check the input device volume in system settings
- Verify the correct device is selected
- Test with `list-devices` to see available configurations

## License

See the main repository LICENSE file for details.
