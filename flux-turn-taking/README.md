# Rust Flux WebSocket Client

A Rust application that captures audio from your microphone and streams it to the Deepgram Flux API via WebSocket for real-time speech recognition.

**NOTE**: This is a not a bidirectional Voice Agent (VA) example. It is purely using the Flux model for speech-to-text transcription.

## Features

- Real-time audio capture from microphone
- WebSocket connection to Deepgram Flux API
- Streams 44.1kHz, 16-bit linear PCM audio data
- Displays all WebSocket message types and response data
- Handles various message types (text, binary, close, ping, pong)
- Clean shutdown with Ctrl+C

## Prerequisites

- Rust (latest stable version)
- A Deepgram API key with Flux API access
- A working microphone

## Setup

1. Clone or download this repository
2. Set your Deepgram API key as an environment variable:

```bash
export DEEPGRAM_API_KEY="your_api_key_here"
```

## Usage

The application supports two modes: streaming from microphone or from an audio file.

### Build the Application

```bash
cargo build --release
```

### Microphone Mode

Stream audio from your microphone to Deepgram Flux API:

```bash
cargo run -- microphone
```

Or with the built binary:

```bash
./target/release/rust-flux microphone
```

**Options:**

- `--endpoint <URL>` - Custom endpoint base URL (e.g., `ws://localhost:8119/`)
- `--sample-rate <HZ>` - Sample rate in Hz (default: 44100)
- `--encoding <FORMAT>` - Audio encoding format (default: linear16)

**Example with custom options:**

```bash
cargo run -- microphone --sample-rate 16000 --encoding linear16
```

### File Mode

Stream audio from a file to Deepgram Flux API:

```bash
cargo run -- file --file path/to/audio.wav
```

Or with the built binary:

```bash
./target/release/rust-flux file --file path/to/audio.wav
```

**Options:**

- `--file <PATH>` - Path to the audio file to transcribe (required)
- `--endpoint <URL>` - Custom endpoint base URL (e.g., `ws://localhost:8119/`)
- `--sample-rate <HZ>` - Sample rate in Hz (default: 44100)
- `--encoding <FORMAT>` - Audio encoding format (default: linear16)

**Example with custom options:**

```bash
cargo run -- file --file audio.wav --sample-rate 16000 --encoding mulaw
```

### Help

To see all available commands and options:

```bash
cargo run -- --help
cargo run -- microphone --help
cargo run -- file --help
```

### What Happens

The application will:

- Connect to your microphone (microphone mode) or read the specified file (file mode)
- Establish a WebSocket connection to Deepgram Flux API
- Start streaming audio data
- Display all responses from the API including message types

To stop the application, press `Ctrl+C`

## Output Format

The application displays WebSocket messages in the following format:

```text
📨 Message Type: [message_type]
📄 Response Data: [formatted_json_data]
---
```

Message types you might see:

- `Results` - Speech recognition results
- `Metadata` - Connection and configuration information
- `Binary` - Binary data responses
- `Close` - Connection close messages
- `Ping`/`Pong` - WebSocket keepalive messages

## Configuration

The application is configured to use:

- **Model**: `flux-general-en` (Deepgram's Flux model for English)
- **Sample Rate**: 44.1kHz
- **Encoding**: linear16 (16-bit PCM)
- **Channels**: 1 (mono)

## Dependencies

- `tokio` - Async runtime
- `tokio-tungstenite` - WebSocket client
- `cpal` - Cross-platform audio I/O
- `serde` & `serde_json` - JSON serialization
- `futures-util` - Async utilities
- `log` & `env_logger` - Logging
- `url` - URL parsing

## Troubleshooting

### No input device available

Make sure you have a microphone connected and accessible to your system.

### DEEPGRAM_API_KEY not set

Ensure you've set the environment variable with your Deepgram API key:

```bash
export DEEPGRAM_API_KEY="your_api_key_here"
```

### WebSocket connection errors

- Verify your API key is valid and has Flux API access
- Check your internet connection
- Ensure the Deepgram Flux API endpoint is accessible

### Audio capture issues

- Check microphone permissions on your system
- Try running with elevated privileges if needed
- Verify your default audio input device is working

## API Reference

This application connects to the Deepgram Flux API WebSocket endpoint:

```text
wss://api.preview.deepgram.com/v2/listen?model=flux-general-en&sample_rate=44100&encoding=linear16
```

For more information about the Flux API, visit: [Flux Early Access](https://developers.deepgram.com/flux-early-access)
