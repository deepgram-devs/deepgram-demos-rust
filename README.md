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

Run the application:

```bash
cargo run
```

The application will:

1. Initialize your default microphone
2. Connect to the Deepgram WebSocket API
3. Start capturing and streaming audio
4. Display transcription results in real-time

Press `Ctrl+C` to stop the application.

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

This project is provided as-is for educational and demonstration purposes.
