# Rust Flux WebSocket Client

A Rust application that streams audio to the Deepgram Flux API via WebSocket for real-time speech recognition with turn detection and incremental transcription display.

**NOTE**: This is not a bidirectional Voice Agent (VA) example. It is purely using the Flux model for speech-to-text transcription with advanced turn-taking capabilities.

## Features

### Core Functionality

- **Real-time audio capture** from microphone
- **Multi-format audio file streaming** with automatic decoding (WAV, MP3, AAC)
- **Real-time playback speed** for file streaming (simulates live audio)
- **WebSocket connection** to Deepgram Flux API
- **Multi-threaded stress testing** support (configurable concurrent connections)

### Transcription Display

- **Incremental word printing** - words appear as they're recognized
- **Color-coded speaker turns** - different colors for each turn_index
- **Turn detection** - automatic line breaks when speakers change
- **Real-time feedback** - see transcriptions as they happen
- **Verbose mode** - optional full JSON response output for debugging

### Technical Features

- Automatic audio format detection and decoding
- Configurable sample rates and encoding formats
- Custom endpoint support for local/staging environments
- Deepgram Request ID tracking
- Comprehensive logging to file
- Clean shutdown with Ctrl+C
- Inactivity timeout handling

## Prerequisites

- Rust (latest stable version)
- A Deepgram API key with Flux API access
- A working microphone (for microphone mode)
- Audio files in supported formats (for file mode)

## Setup

1. Clone or download this repository
2. Set your Deepgram API key as an environment variable:

```bash
export DEEPGRAM_API_KEY="your_api_key_here"
```

To compile and install the application with the Rust toolchain into `$HOME/.cargo/bin`, use this command:

```bash
cargo install --path flux-turn-taking
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
./target/release/dg-flux microphone
```

**Options:**

- `--endpoint <URL>` - Custom endpoint base URL (e.g., `ws://localhost:8119/`)
- `--sample-rate <HZ>` - Sample rate in Hz (default: 44100)
- `--encoding <FORMAT>` - Audio encoding format (default: linear16)
- `--threads <N>` - Number of concurrent connections (default: 1)
- `--inactivity-timeout <MS>` - Inactivity timeout in milliseconds (default: 10000)
- `--verbose` - Print statistics table instead of all messages

**Example with custom options:**

```bash
cargo run -- microphone --sample-rate 16000 --threads 2 --verbose
```

### File Mode

Stream audio from a file to Deepgram Flux API with real-time playback simulation:

```bash
cargo run -- file --path audio.mp3
```

Or with the built binary:

```bash
./target/release/dg-flux file --path audio.wav
```

**Supported Audio Formats:**

- WAV (`.wav`)
- MP3 (`.mp3`)
- AAC (`.aac`)

**Options:**

- `--path <PATH>` - Path to the audio file to transcribe (required)
- `--endpoint <URL>` - Custom endpoint base URL (e.g., `ws://localhost:8119/`)
- `--encoding <FORMAT>` - Audio encoding format (always linear16 for decoded audio)
- `--threads <N>` - Number of concurrent connections (default: 1)
- `--inactivity-timeout <MS>` - Inactivity timeout in milliseconds (default: 10000)
- `--verbose` - Print full JSON responses instead of incremental transcription

**Example commands:**

```bash
# Basic usage
cargo run -- file --path recording.mp3

# With custom endpoint
cargo run -- file --path audio.wav --endpoint ws://localhost:8119/

# Multiple concurrent connections for stress testing
cargo run -- file --path audio.wav --threads 4

# Verbose mode to see full JSON responses
cargo run -- file --path audio.aac --verbose
```

### Help

To see all available commands and options:

```bash
cargo run -- --help
cargo run -- microphone --help
cargo run -- file --help
```

## Output Examples

### Default Mode (Incremental Transcription)

In default mode, words appear incrementally as they're recognized, with different colors for each speaker turn:

```text
📁 Streaming file to Deepgram Flux API...
File: recording.mp3
Audio: 48000 Hz, 1 channel(s), 51.90s duration
Spawning 1 worker thread(s)...
📝 Writing logs to: flux-turn-taking.log
===
Transcription results:

🎵 Streaming at real-time speed (100 ms chunks)...
[Thread 0] 🔗 Deepgram Request ID: fd2790cb-9de9-4207-93ea-4349d1b74867
Here is some text that is being transcribed by Deepgram's Flux model.
```

Each line represents a different speaker turn, displayed in a different color in the terminal.

### Verbose Mode

With `--verbose`, see the full JSON responses from the Flux API:

```text
[Thread 0] 📨 Event: TurnInfo
[Thread 0] 📄 Response Data: {
  "audio_window_end": 1.44,
  "audio_window_start": 0.0,
  "end_of_turn_confidence": 0.0066,
  "request_id": "fd2790cb-9de9-4207-93ea-4349d1b74867",
  "sequence_id": 6,
  "transcript": "Here",
  "turn_index": 0,
  "words": [
    {
      "confidence": 0.7778,
      "word": "Here"
    }
  ]
}
---
```

## How It Works

### File Mode Operation

1. **Audio Decoding**: The application uses the Symphonia library to decode audio files in various formats
2. **Format Detection**: Sample rate, channels, and duration are automatically detected
3. **Real-time Streaming**: Audio is chunked and streamed at real-time speed (100ms chunks by default)
4. **WebSocket Communication**: Audio data is sent as binary messages to the Flux API
5. **Incremental Display**: As TurnInfo events arrive, new words are appended to the current line
6. **Turn Detection**: When turn_index changes, a new line starts with a different color
7. **Completion**: When audio streaming completes, the connection closes gracefully

### Event Types

The Flux API sends these event types:

- `Connected` - Initial connection confirmation
- `TurnInfo` - Incremental transcription updates with words
- `EndOfTurn` - Turn completion (when detected)
- `SpeechStarted` - Speech detection events
- `Metadata` - Connection and configuration information

## Configuration

### Default Settings

- **Model**: `flux-general-en` (Deepgram's Flux model for English)
- **Sample Rate**: Auto-detected from file (file mode) or 44.1kHz (microphone mode)
- **Encoding**: linear16 (16-bit PCM)
- **Channels**: 1 (mono, converted if needed)
- **Chunk Duration**: 100ms for real-time simulation

### Color Scheme

Different speaker turns cycle through these colors:

- Cyan
- Green
- Yellow
- Magenta
- Blue
- White

## Dependencies

- `tokio` - Async runtime
- `tokio-tungstenite` - WebSocket client
- `cpal` - Cross-platform audio I/O
- `symphonia` - Audio decoding library (supports multiple formats)
- `serde` & `serde_json` - JSON serialization
- `futures-util` - Async utilities
- `log` & `env_logger` - Logging
- `url` - URL parsing
- `clap` - Command-line argument parsing
- `crossterm` - Terminal manipulation for colors
- `tabled` - Table formatting for statistics

## Logging

All application logs are written to `flux-turn-taking.log` in the current directory. This includes:

- Connection events
- Audio streaming progress
- Parsing information
- Error messages
- Deepgram Request IDs

Set the `RUST_LOG` environment variable to control log levels:

```bash
export RUST_LOG=info  # or debug, trace, warn, error
```

## Troubleshooting

### No input device available (Microphone Mode)

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
- Check the log file for detailed error messages

### Audio capture issues (Microphone Mode)

- Check microphone permissions on your system
- Try running with elevated privileges if needed
- Verify your default audio input device is working

### File not found or unsupported format

- Ensure the file path is correct
- Check that the file is in a supported format (WAV, MP3, AAC)
- Verify the file is not corrupted

### No words appearing in output

- Check `flux-turn-taking.log` for parsing errors
- Try running with `--verbose` to see the raw API responses
- Ensure your audio file contains speech
- Verify the Flux API is returning TurnInfo events

## Performance and Load Testing

The application supports multiple concurrent connections for stress testing:

```bash
# Run with 10 concurrent connections
cargo run -- file --path audio.mp3 --threads 10
```

In microphone mode with multiple threads, a statistics table shows throughput for each connection:

```text
┌────────┬─────────────┬─────────────┬─────────┬──────────────┬─────────────┬──────────┬───────┐
│ Thread │ Bytes Sent  │ Bytes Recv  │ Results │ SpeechStarted│ UtteranceEnd│ Metadata │ Other │
├────────┼─────────────┼─────────────┼─────────┼──────────────┼─────────────┼──────────┼───────┤
│ 0      │ 1048576     │ 45231       │ 142     │ 3            │ 5           │ 1        │ 0     │
└────────┴─────────────┴─────────────┴─────────┴──────────────┴─────────────┴──────────┴───────┘
```

## API Reference

This application connects to the Deepgram Flux API WebSocket endpoint:

```text
wss://api.deepgram.com/v2/listen?model=flux-general-en&sample_rate={rate}&encoding=linear16
```

For more information about the Flux API, visit: [Deepgram Flux Documentation](https://developers.deepgram.com/docs/flux)
