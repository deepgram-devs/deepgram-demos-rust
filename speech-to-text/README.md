# Deepgram Streaming Rust

A real-time speech-to-text application using Rust that connects to the Deepgram API via WebSocket and captures audio from your default microphone.

## Features

- Real-time audio capture from the default microphone using CPAL
- WebSocket connection to Deepgram API for live transcription
- Cross-platform support (Windows, macOS, Linux)
- Displays transcription results with confidence scores in real-time

## Prerequisites

1. **Rust**: Make sure you have Rust installed. If not, install it from [rustup.rs](https://rustup.rs/)

2. **Deepgram API Key**: You need a Deepgram API key. Sign up at [deepgram.com](https://deepgram.com) to get one.

3. **Audio Input Device**: Ensure you have a working microphone connected to your system.

## Setup

1. Clone this repository or download the source code.

2. Create a `.env` file in the project root and add your Deepgram API key:

```toml
DEEPGRAM_API_KEY=your_api_key_here
```

1. Install dependencies:

```bash
cargo build
```

## Usage

The application now supports two modes: microphone streaming and file streaming.

### Microphone Mode

Stream audio from your microphone for real-time transcription:

```bash
cargo run -- microphone
```

The application will:

1. Initialize your default microphone
2. Connect to the Deepgram WebSocket API
3. Start capturing and streaming audio
4. Display transcription results in real-time

Press `Ctrl+C` to stop the application.

### File Mode

Transcribe audio from a file (supports MP3, WAV, and FLAC formats):

```bash
cargo run -- file --file path/to/audio.mp3
```

#### Real-time Streaming (Default)

By default, the file is streamed at real-time rate, simulating live audio:

```bash
cargo run -- file --file recording.wav
```

#### Fast Streaming

Use the `--fast` flag to stream the file as quickly as possible (may not be much faster than live)):

```bash
cargo run -- file --file podcast.mp3 --fast
```

### Examples

```bash
# Transcribe from microphone
cargo run -- microphone

# Transcribe a WAV file at real-time rate
cargo run -- file --file recording.wav

# Transcribe an MP3 file as fast as possible
cargo run -- file --file podcast.mp3 --fast

# Transcribe a FLAC file
cargo run -- file --file music.flac
```

### Help

View all available options:

```bash
cargo run -- --help
cargo run -- microphone --help
cargo run -- file --help
```

## How It Works

1. **Audio Capture**: Uses the CPAL library to capture audio from the default input device
2. **Audio Processing**: Converts audio samples to 16-bit linear PCM format required by Deepgram
3. **WebSocket Connection**: Establishes a secure WebSocket connection to Deepgram's streaming API
4. **Real-time Streaming**: Continuously sends audio data to Deepgram and receives transcription results
5. **Display Results**: Shows transcribed text with confidence scores in the terminal

## Configuration

The application automatically detects your microphone's sample rate and channel configuration. It supports:

- Sample formats: F32, I16, U16
- Multiple channels (mono/stereo)
- Various sample rates

## Dependencies

- `tokio`: Async runtime
- `tokio-tungstenite`: WebSocket client with TLS support
- `cpal`: Cross-platform audio library
- `serde`: JSON serialization/deserialization
- `dotenv`: Environment variable loading
- `futures-util`: Stream utilities

## Troubleshooting

### No Input Device Available

Make sure you have a microphone connected and it's set as the default input device in your system settings.

### WebSocket Connection Issues

- Verify your Deepgram API key is correct
- Check your internet connection
- Ensure the API key has sufficient credits

### Audio Permission Issues

On some systems, you may need to grant microphone permissions to the terminal or the application.

## License

See `LICENSE.md`
