# Deepgram Streaming Rust

A real-time speech-to-text application using Rust that connects to the Deepgram API via WebSocket and captures audio from your default microphone.

## Features

### Stream Mode (WebSocket API)
- Real-time audio capture from the default microphone using CPAL
- Stream audio files (MP3, WAV, FLAC) at real-time or fast rate
- Multichannel audio processing
- WebSocket connection to Deepgram API for live transcription
- Callback support for webhook integration
- Displays transcription results with confidence scores in real-time

### Transcribe Mode (HTTP API)
- Pre-recorded audio transcription for files
- Support for all audio formats (MP3, WAV, FLAC, and more)
- Multichannel audio processing
- Advanced AI features:
  - Speaker diarization (who spoke when)
  - Summarization (generate summaries)
  - Topic detection (identify key topics)
  - Intent detection (understand intent)
  - Sentiment analysis (analyze emotional tone)
  - Entity detection (extract key entities)
- Redaction of sensitive data based on Deepgram [supported entity types](https://developers.deepgram.com/docs/supported-entity-types)
- Multiple output formats (text, JSON, verbose JSON)

### General Features
- Cross-platform support (Windows, macOS, Linux)
- Multiple Deepgram models (nova-3, nova-2, enhanced, base)
- Language support for multiple languages
- Smart formatting and punctuation

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

The application supports two modes of operation:
1. **Stream**: Real-time streaming transcription (WebSocket API)
2. **Transcribe**: Pre-recorded audio transcription (HTTP API)

### Microphone Mode

Stream audio from your microphone for real-time transcription:

```bash
cargo run -- stream microphone
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
cargo run -- stream file --file path/to/audio.mp3
```

#### Real-time Streaming (Default)

By default, the file is streamed at real-time rate, simulating live audio:

```bash
cargo run -- stream file --file recording.wav
```

#### Fast Streaming

Use the `--fast` flag to stream the file as quickly as possible (may not be much faster than live)):

```bash
cargo run -- stream file --file podcast.mp3 --fast
```

### Transcribe Mode (Pre-recorded Audio)

Transcribe a pre-recorded audio file using the Deepgram HTTP API:

```bash
cargo run -- transcribe --file path/to/audio.mp3
```

This mode supports additional AI features:

```bash
# Basic transcription
cargo run -- transcribe --file recording.wav

# With punctuation and smart formatting
cargo run -- transcribe --file audio.mp3 --punctuate true --smart-format true

# With speaker diarization
cargo run -- transcribe --file meeting.wav --diarize true

# With multichannel processing
cargo run -- transcribe --file stereo-recording.wav --multichannel

# With AI features (summarization, topics, intents, sentiment)
cargo run -- transcribe --file podcast.mp3 --summarize v2 --topics true --intents true --sentiment true

# With entity detection
cargo run -- transcribe --file interview.wav --detect-entities true

# With specific model and language
cargo run -- transcribe --file audio.mp3 --model nova-3 --language en-US

# Output formats: text (default), json, or verbose-json
cargo run -- transcribe --file audio.mp3 --output json
```

### Callback Support (Stream Mode Only)

Both microphone and file modes support sending transcription results to a callback URL via HTTP POST:

```bash
# Microphone mode with callback
cargo run -- stream microphone --callback https://example.com/webhook

# File mode with callback
cargo run -- stream file --file audio.mp3 --callback https://example.com/webhook

# Use --silent flag to suppress console output when using callbacks
cargo run -- stream file --file audio.mp3 --callback https://example.com/webhook --silent
```

When a callback URL is provided:

- Deepgram will send transcription results to your specified URL via HTTP POST
- The callback method is automatically set to POST
- Console output continues by default unless `--silent` flag is used
- The `--silent` flag suppresses transcript output to the console

### Examples

#### Stream Mode Examples

```bash
# Transcribe from microphone
cargo run -- stream microphone

# Transcribe from microphone with redaction
cargo run -- stream microphone --redact pii,blood_type

# Transcribe from microphone with multichannel processing
cargo run -- stream microphone --multichannel

# Transcribe a WAV file at real-time rate
cargo run -- stream file --file recording.wav

# Transcribe an MP3 file as fast as possible
cargo run -- stream file --file podcast.mp3 --fast

# Transcribe a FLAC file with multichannel processing
cargo run -- stream file --file music.flac --multichannel

# Microphone with callback
cargo run -- stream microphone --callback https://example.com/webhook

# File with callback and silent mode
cargo run -- stream file --file audio.mp3 --callback https://example.com/webhook --silent

# Fast file streaming with callback
cargo run -- stream file --file podcast.mp3 --fast --callback https://example.com/webhook
```

#### Transcribe Mode Examples

```bash
# Simple transcription
cargo run -- transcribe --file audio.mp3

# Full-featured transcription with AI capabilities
cargo run -- transcribe --file meeting.wav \
  --model nova-3 \
  --punctuate true \
  --smart-format true \
  --diarize true \
  --multichannel \
  --summarize v2 \
  --topics true \
  --intents true \
  --sentiment true \
  --detect-entities true

# Transcription with redaction
cargo run -- transcribe --file sensitive.wav --redact pii,pci

# Get JSON output for further processing
cargo run -- transcribe --file audio.mp3 --output json > output.json
```

### Help

View all available options:

```bash
cargo run -- --help
cargo run -- stream --help
cargo run -- stream microphone --help
cargo run -- stream file --help
cargo run -- transcribe --help
```

## How It Works

### Stream Mode (Real-time)
1. **Audio Capture**: Uses the CPAL library to capture audio from the default input device or reads from a file
2. **Audio Processing**: Converts audio samples to 16-bit linear PCM format required by Deepgram
3. **WebSocket Connection**: Establishes a secure WebSocket connection to Deepgram's streaming API
4. **Real-time Streaming**: Continuously sends audio data to Deepgram and receives transcription results
5. **Display Results**: Shows transcribed text with confidence scores in the terminal

### Transcribe Mode (Pre-recorded)
1. **File Reading**: Reads the entire audio file into memory
2. **HTTP Request**: Sends the audio file to Deepgram's HTTP API in a single POST request
3. **AI Processing**: Deepgram processes the complete audio file with requested AI features
4. **Response Parsing**: Parses the JSON response containing transcript and analysis
5. **Display Results**: Shows transcript, confidence scores, and any requested AI insights (speakers, summaries, topics, etc.)

## Configuration

The application automatically detects your microphone's sample rate and channel configuration. It supports:

- Sample formats: F32, I16, U16
- Multiple channels (mono/stereo)
- Various sample rates

## Dependencies

- `tokio`: Async runtime
- `tokio-tungstenite`: WebSocket client with TLS support (for stream mode)
- `reqwest`: HTTP client (for transcribe mode)
- `cpal`: Cross-platform audio library (for microphone capture)
- `symphonia`: Audio decoding library (for file streaming)
- `serde`: JSON serialization/deserialization
- `dotenv`: Environment variable loading
- `futures-util`: Stream utilities
- `clap`: Command-line argument parsing

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
