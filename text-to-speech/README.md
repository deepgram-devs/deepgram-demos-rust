# Deepgram Text-to-Speech (TTS) Demo

A Rust CLI application demonstrating Deepgram's Text-to-Speech API with multiple modes of operation.

## Features

- **Speak Mode**: Interactive text-to-speech with immediate playback
- **Save Mode**: Convert text to speech and save to an audio file
- **Stream Mode**: WebSocket-based streaming TTS with real-time audio playback

## Prerequisites

- Rust (latest stable version)
- A Deepgram API key

## Setup

1. Set your Deepgram API key as an environment variable:

```bash
export DEEPGRAM_API_KEY="your-api-key-here"
```

   Or create a `.env` file in the project directory:

```bash
DEEPGRAM_API_KEY=your-api-key-here
```

1. Build the project:

```bash
cargo build --release
```

## Usage

### Speak Mode (Interactive)

Speak text interactively with immediate audio playback:

```bash
cargo run --release -- speak
```

Or with a custom voice:

```bash
cargo run --release -- speak --voice aura-2-asteria-en
```

Type your text and press Enter to hear it spoken. Type `quit` to exit.

### Save Mode

Convert text to speech and save to a file:

```bash
cargo run --release -- save --text "Hello, world!" --output output.mp3
```

With custom voice:

```bash
cargo run --release -- save --text "Hello, world!" --output output.mp3 --voice aura-2-asteria-en
```

### Stream Mode (WebSocket)

Use WebSocket streaming for real-time text-to-speech:

```bash
cargo run --release -- stream
```

Or with a custom voice:

```bash
cargo run --release -- stream --voice aura-2-asteria-en
```

In stream mode:

- Type your text and press Enter to hear it spoken immediately
- Audio is streamed and played back in real-time
- Type `exit` to quit

## Available Voices

Deepgram offers various voice models. Some examples:

- `aura-2-thalia-en` (default)
- `aura-2-asteria-en`
- `aura-2-luna-en`
- `aura-2-stella-en`
- `aura-2-athena-en`
- `aura-2-hera-en`
- `aura-2-orion-en`
- `aura-2-arcas-en`
- `aura-2-perseus-en`
- `aura-2-angus-en`
- `aura-2-orpheus-en`
- `aura-2-helios-en`
- `aura-2-zeus-en`

For the complete list of available voices, visit the [Deepgram documentation](https://developers.deepgram.com/docs/tts-models).

## Optional Parameters

### Tags

Add custom tags to your requests for tracking and analytics:

```bash
cargo run --release -- speak --tags "demo,testing"
cargo run --release -- save --text "Hello" --output out.mp3 --tags "production"
cargo run --release -- stream --tags "websocket,demo"
```

### Endpoint Override

Override the default Deepgram API endpoint (useful for testing or using alternative deployments):

```bash
# For HTTP-based commands (speak and save)
cargo run --release -- speak --endpoint "https://api.deepgram.com"
cargo run --release -- save --text "Hello" --output out.mp3 --endpoint "https://api.deepgram.com"

# For WebSocket streaming
cargo run --release -- stream --endpoint "wss://api.deepgram.com"
```

The default endpoints are:

- HTTP commands: `https://api.deepgram.com`
- Stream command: `wss://api.deepgram.com`

## Architecture

- **main.rs**: CLI interface and command routing
- **stream.rs**: WebSocket streaming implementation with real-time audio playback

The streaming implementation uses:

- `tokio-tungstenite` for WebSocket connections
- `rodio` for audio playback
- Async/await for concurrent message handling
- Thread-based audio playback for optimal performance

## License

See LICENSE.md in the repository root.
