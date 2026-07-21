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

- **Single-connection transcript by default** - prints the transcript from the first connection (connection 0); select another with `--connection`
- **Message-type prefix** - every printed line is prefixed with the Flux event that produced it (`StartOfTurn`, `Update`, `EagerEndOfTurn`, `TurnResumed`, `EndOfTurn`)
- **Confidence scores** - `EagerEndOfTurn` and `EndOfTurn` lines are suffixed with their confidence score
- **Color-coded speaker turns** - different colors for each turn_index, applied only to the transcript text (never to the statistics table)
- **Real-time feedback** - see transcriptions as they happen
- **Optional statistics table** - pass `--stats` to see live throughput/event counts for every connection instead of the transcript (useful for load testing)
- **Verbose mode** - optional full JSON response output for debugging, for every connection

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
- `--numerals` - Convert spoken numbers into digits (e.g. "nine hundred" -> "900")
- `--eager-eot-threshold <0.3-0.9>` (alias: `--eeot`) - Enable `EagerEndOfTurn`/`TurnResumed` events at this confidence threshold (default: disabled)
- `--connection <N>` - Which connection's transcript to print in the regular output mode (default: 0, the first connection)
- `--stats` - Show a live statistics table for all connections instead of the selected connection's transcript
- `--verbose` - Print full JSON responses for every connection instead of the selected connection's transcript

**Example with custom options:**

```bash
cargo run -- microphone --sample-rate 16000 --threads 2 --verbose
```

**Example with numerals enabled:**

```bash
cargo run -- microphone --numerals
```

**Example with eager end-of-turn detection enabled:**

```bash
cargo run -- microphone --eager-eot-threshold 0.4
# or, using the short alias
cargo run -- microphone --eeot 0.4
```

**Example printing the transcript from a specific connection (load-testing scenario):**

```bash
cargo run -- microphone --threads 4 --connection 2
```

**Example showing the live statistics table instead of a transcript:**

```bash
cargo run -- microphone --threads 4 --stats
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
- `--numerals` - Convert spoken numbers into digits (e.g. "nine hundred" -> "900")
- `--eager-eot-threshold <0.3-0.9>` (alias: `--eeot`) - Enable `EagerEndOfTurn`/`TurnResumed` events at this confidence threshold (default: disabled)
- `--connection <N>` - Which connection's transcript to print in the regular output mode (default: 0, the first connection)
- `--stats` - Show a live statistics table for all connections instead of the selected connection's transcript
- `--verbose` - Print full JSON responses for every connection instead of the selected connection's transcript

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

# Convert spoken numbers into digits
cargo run -- file --path recording.mp3 --numerals

# Enable eager end-of-turn detection (or use the short --eeot alias)
cargo run -- file --path recording.mp3 --eager-eot-threshold 0.4

# Print the transcript from connection 2 out of 4 concurrent connections
cargo run -- file --path recording.mp3 --threads 4 --connection 2

# Show the live statistics table instead of a transcript
cargo run -- file --path recording.mp3 --threads 4 --stats
```

### Help

To see all available commands and options:

```bash
cargo run -- --help
cargo run -- microphone --help
cargo run -- file --help
```

## Output Examples

### Default Mode (Regular Functional Mode)

In default mode, the app prints the transcript for the selected connection (connection 0 unless
`--connection` says otherwise). Every Flux message produces one line, prefixed with the event type
that produced it; `EagerEndOfTurn` and `EndOfTurn` lines are suffixed with their confidence score.
All lines belonging to the same turn (`turn_index`) share a color, cycling to the next color when
a new turn starts:

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
StartOfTurn: Here
Update: Here is
Update: Here is some text
EagerEndOfTurn: Here is some text that is being transcribed [eager_eot_confidence: 0.6200]
Update: Here is some text that is being transcribed by
EndOfTurn: Here is some text that is being transcribed by Deepgram's Flux model. [eot_confidence: 0.9100]
```

Only the selected connection's transcript is printed; other connections (when `--threads > 1`)
keep streaming and updating their counters in the background but produce no output of their own
unless `--stats` is passed (see below).

### Verbose Mode

With `--verbose`, see the full JSON responses from the Flux API:

```text
[Thread 0] 📨 Type: TurnInfo (event: StartOfTurn)
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
5. **Transcript Display**: Each `TurnInfo` message from the selected connection prints one line, prefixed with its event type
6. **Turn Detection**: All lines sharing a `turn_index` are printed in the same color, cycling to the next color on a new turn
7. **Completion**: When audio streaming completes, the connection closes gracefully

### Message Types

The Flux API sends these top-level message types (the `type` field):

- `Connected` - Initial connection confirmation
- `TurnInfo` - Transcription and turn-state updates (see below)
- `ConfigureSuccess` - A `Configure` control message was applied
- `ConfigureFailure` - A `Configure` control message was rejected
- `Error` - A fatal, unrecoverable error; the connection closes shortly after

Every `TurnInfo` message also carries an `event` field describing the turn-state
transition it represents:

- `StartOfTurn` - The user has begun speaking for the first time in the turn
- `Update` - Additional audio has been transcribed, but the turn state hasn't changed
- `EagerEndOfTurn` - Moderate confidence the user has finished speaking; an opportunity to start preparing an agent reply. Printed with its `end_of_turn_confidence` score as `[eager_eot_confidence: X.XXXX]`
- `TurnResumed` - Speech is continuing after an `EagerEndOfTurn` was sent for this turn
- `EndOfTurn` - The user has finished speaking for the turn. Printed with its `end_of_turn_confidence` score as `[eot_confidence: X.XXXX]`

Unlike Nova-3 streaming, Flux does not send separate `Results`, `SpeechStarted`,
`UtteranceEnd`, or `Metadata` message types - all transcription and turn-state
updates arrive as `TurnInfo` messages distinguished by their `event` field.

## Configuration

### Default Settings

- **Model**: `flux-general-en` (Deepgram's Flux model for English)
- **Sample Rate**: Auto-detected from file (file mode) or 44.1kHz (microphone mode)
- **Encoding**: linear16 (16-bit PCM)
- **Channels**: 1 (mono, converted if needed)
- **Chunk Duration**: 100ms for real-time simulation

### Color Scheme

Color is only ever applied to the selected connection's transcript text in the regular
functional output mode; the statistics table (below) is always printed uncolored, even if
a transcript line's color was still active moments before. Different speaker turns cycle
through these colors:

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

### No transcript appearing in output

- Check `flux-turn-taking.log` for parsing errors
- Try running with `--verbose` to see the raw API responses
- Ensure your audio file contains speech
- Verify the Flux API is returning TurnInfo events
- With `--threads > 1`, confirm `--connection` points at a connection that's actually receiving speech

## Performance and Load Testing

The application supports multiple concurrent connections for stress testing:

```bash
# Run with 10 concurrent connections
cargo run -- file --path audio.mp3 --threads 10
```

Add `--stats` (in either mode) to see a live statistics table showing throughput and Flux event
counts for every connection, refreshed twice a second, instead of a single connection's transcript:

```text
┌────────┬────────────┬────────────┬─────────────┬────────┬────────────────┬─────────────┬───────────┬────────┬───────┐
│ Thread │ Bytes Sent │ Bytes Recv │ StartOfTurn │ Update │ EagerEndOfTurn │ TurnResumed │ EndOfTurn │ Errors │ Other │
├────────┼────────────┼────────────┼─────────────┼────────┼────────────────┼─────────────┼───────────┼────────┼───────┤
│ 0      │ 1048576    │ 45231      │ 4           │ 142    │ 3              │ 1           │ 4         │ 0      │ 1     │
└────────┴────────────┴────────────┴─────────────┴────────┴────────────────┴─────────────┴───────────┴────────┴───────┘
```

## API Reference

This application connects to the Deepgram Flux API WebSocket endpoint:

```text
wss://api.deepgram.com/v2/listen?model=flux-general-en&sample_rate={rate}&encoding=linear16&numerals={true|false}&eager_eot_threshold={0.3-0.9}
```

Notes:

- `numerals` must be set when the connection is opened. Flux does not support toggling `numerals` mid-stream via a `Configure` message.
- `eager_eot_threshold` is only appended when `--eager-eot-threshold` is passed; omitting it disables `EagerEndOfTurn`/`TurnResumed` events (Flux's default). Unlike `numerals`, Flux does allow changing `eager_eot_threshold` mid-stream via a `Configure` message, though this CLI only sets it at connection time.

For more information about the Flux API, visit: [Deepgram Flux Documentation](https://developers.deepgram.com/docs/flux)
