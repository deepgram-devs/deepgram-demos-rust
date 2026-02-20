# Deepgram Voice Agent - Rust Implementation

This is a Rust application that demonstrates bidirectional conversation using Deepgram's Voice Agent WebSocket APIs. The application captures audio from your computer's microphone, streams it to Deepgram's Voice Agent, and handles the responses.

⚠️ **IMPORTANT**: Please use headphones to avoid spoken audio, from the agent, back-feeding through your microphone. Alternatively, use `--no-mic-mute` if your setup handles echo cancellation externally.

## Features

- **Real-time Audio Capture**: Captures audio from your computer's microphone using CPAL
- **WebSocket Communication**: Connects to Deepgram's Voice Agent API via WebSocket
- **Bidirectional Conversation**: Sends audio to the agent and receives responses
- **Flexible CLI Configuration**: Configure endpoint, TTS model, LLM provider, and custom headers via command-line flags
- **Audio Response Handling**: Receives and plays back audio responses from the agent using rodio
- **Smart Microphone Control**: Automatically disables microphone during agent speech and re-enables 600ms after silence (prevents feedback)
- **Silent Packet Injection**: Sends silent audio frames while muted to keep the WebSocket connection alive

## Prerequisites

1. **Deepgram API Key**: You need a Deepgram API key with access to Voice Agent features
2. **Rust**: Make sure you have [Rust toolchain installed](https://rustup.rs/)
3. **Audio Device**: A working microphone for audio input

## Setup

1. **Clone and navigate to the project**:

```bash
git clone git@github.com:deepgram-devs/deepgram-demos-rust.git
cd voice-agent
```

2. **Set up environment variables**:

```bash
cp .env.example .env
# Edit .env and add your Deepgram API key
```

3. **Install dependencies**:

```bash
cargo build
```

## Usage

1. **Run the application**:

```bash
cargo run
```

2. **Start speaking**: Once the application starts, it will begin capturing audio from your microphone and streaming it to Deepgram's Voice Agent.

3. **Monitor the conversation**: The application will log:

   - Your speech transcriptions
   - Agent thinking status
   - Agent responses
   - Audio data reception

4. **Stop the application**: Press `Ctrl+C` to stop the application.

## CLI Options

| Option | Default | Description |
|--------|---------|-------------|
| `--endpoint <URL>` | `wss://agent.deepgram.com` | Deepgram WebSocket endpoint |
| `--speak-model <MODEL>` | `aura-2-thalia-en` | TTS model for agent voice |
| `--think-type <TYPE>` | `open_ai` | LLM provider type |
| `--think-model <MODEL>` | `gpt-4o-mini` | LLM model |
| `--think-endpoint <URL>` | _(none)_ | Custom URL for LLM provider |
| `--think-header <KEY=VALUE>` | _(none)_ | Extra header for LLM provider (repeatable) |
| `--verbose` | _(off)_ | Print full Settings JSON at startup |
| `--no-mic-mute` | _(off)_ | Disable mic muting during playback |

## Example Commands

```bash
# Basic usage
cargo run

# Use a different TTS voice
cargo run -- --speak-model aura-2-apollo-en

# Use Claude via a custom endpoint
cargo run -- --think-type anthropic \
             --think-model claude-3-5-haiku-20241022 \
             --think-endpoint https://api.anthropic.com/v1 \
             --think-header "x-api-key=YOUR_ANTHROPIC_KEY"

# Connect to a self-hosted endpoint
cargo run -- --endpoint wss://my-internal-agent.example.com

# Print the full Settings JSON for debugging
cargo run -- --verbose

# Keep mic live during playback (e.g., using headphones)
cargo run -- --no-mic-mute
```

## Architecture

The application consists of several key components:

1. **AudioCapture**: Handles microphone input using CPAL
2. **WebSocket Client**: Manages connection to Deepgram Voice Agent API
3. **AudioPlayer**: Handles audio response playback using rodio with automatic microphone control
4. **Message Handlers**: Process different types of responses from the agent

## Troubleshooting

### Common Issues

1. **No microphone detected**: Make sure your microphone is connected and working
2. **WebSocket connection fails**: Check your API key and internet connection
3. **Audio format issues**: The application expects standard audio input devices

### Logging

The application uses `env_logger`. You can control log levels with:

```bash
RUST_LOG=debug cargo run  # For detailed logging
RUST_LOG=info cargo run   # For normal logging (default)
```

## API Reference

This application uses Deepgram's Voice Agent API. For more information:

- [Deepgram Voice Agent Documentation](https://developers.deepgram.com/docs/voice-agent)
- [Deepgram API Console](https://console.deepgram.com/)

## Current Features

- **Full Audio Playback**: Complete implementation of audio response playback using rodio
- **Smart Microphone Management**: Automatic microphone disable/enable based on agent speech, with `--no-mic-mute` opt-out
- **Silent Packet Injection**: Keeps the WebSocket connection alive while mic is muted
- **Real-time Audio Streaming**: Continuous audio capture and streaming to Deepgram
- **Multiple Message Types**: Handles user transcripts, agent transcripts, thinking status, and audio data
- **Binary and Text Audio**: Supports both base64-encoded and binary audio responses

## Future Enhancements

- **Voice Activity Detection**: Add silence detection to optimize streaming
- **Configuration File**: Allow runtime configuration without code changes
- **Multiple Audio Formats**: Support for different audio input/output formats
- **Error Recovery**: Better handling of network disconnections and audio device issues

## License

This project is provided as an example implementation. Please refer to Deepgram's terms of service for API usage.
