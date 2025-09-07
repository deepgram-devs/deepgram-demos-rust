# Deepgram Voice Agent - Rust Implementation

This is a Rust application that demonstrates bidirectional conversation using Deepgram's Voice Agent WebSocket APIs. The application captures audio from your computer's microphone, streams it to Deepgram's Voice Agent, and handles the responses.

## Features

- **Real-time Audio Capture**: Captures audio from your computer's microphone using CPAL
- **WebSocket Communication**: Connects to Deepgram's Voice Agent API via WebSocket
- **Bidirectional Conversation**: Sends audio to the agent and receives responses
- **Configurable Agent**: Uses Nova-2 for speech recognition, GPT-4 for thinking, and Aura Asteria for speech synthesis
- **Audio Response Handling**: Receives and logs audio responses from the agent

## Prerequisites

1. **Deepgram API Key**: You need a Deepgram API key with access to Voice Agent features
2. **Rust**: Make sure you have [Rust toolchain installed](https://rustup.rs/)
3. **Audio Device**: A working microphone for audio input

## Setup

1. **Clone and navigate to the project**:

```bash
cd voice-agent
```

1. **Set up environment variables**:

```bash
cp .env.example .env
# Edit .env and add your Deepgram API key
```

1. **Install dependencies**:

```bash
cargo build
```

## Usage

1. **Run the application**:

```bash
cargo run
```

1. **Start speaking**: Once the application starts, it will begin capturing audio from your microphone and streaming it to Deepgram's Voice Agent.

1. **Monitor the conversation**: The application will log:

   - Your speech transcriptions
   - Agent thinking status
   - Agent responses
   - Audio data reception

1. **Stop the application**: Press `Ctrl+C` to stop the application.

## Configuration

The application is configured to use:

- **Listen Model**: Nova-2 for speech recognition
- **Think Provider**: OpenAI GPT-4 for conversation
- **Speak Model**: Aura Asteria for speech synthesis
- **Audio Input**: Linear16 PCM encoding
- **Audio Output**: Linear16 PCM at 24kHz

You can modify these settings in the `create_agent_config()` function in `src/main.rs`.

## Architecture

The application consists of several key components:

1. **AudioCapture**: Handles microphone input using CPAL
2. **WebSocket Client**: Manages connection to Deepgram Voice Agent API
3. **AudioPlayer**: Handles audio response playback (currently logs only)
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

## Future Enhancements

- **Audio Playback**: Complete implementation of audio response playback using rodio
- **Voice Activity Detection**: Add silence detection to optimize streaming
- **Configuration File**: Allow runtime configuration without code changes
- **Multiple Audio Formats**: Support for different audio input/output formats
- **Error Recovery**: Better handling of network disconnections and audio device issues

## License

This project is provided as an example implementation. Please refer to Deepgram's terms of service for API usage.
