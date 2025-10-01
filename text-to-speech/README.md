# Deepgram Text-to-Speech CLI

This Rust application provides a command-line interface for generating and playing back speech using the Deepgram [Text-to-Speech API](https://developers.deepgram.com/docs/text-to-speech).

## Prerequisites

- [Rust Toolchain](https://rustup.rs) (latest stable version)
- Deepgram API Key

## Setup

1. Clone the repository
2. Set your Deepgram API key as an environment variable:

In bash or zsh:

```bash
export DEEPGRAM_API_KEY=your_api_key_here
```

In PowerShell:

```pwsh
$env:DEEPGRAM_API_KEY = 'your_api_key_here'
```

Or create a `.env` file in the project root with:

```toml
DEEPGRAM_API_KEY=your_api_key_here
```

## Usage

### Speak Command (Interactive Playback)

Run the application with the `speak` subcommand for interactive text-to-speech playback:

```bash
cargo run -- speak
```

Optional arguments:

- `--voice`: Specify the voice model (default: `aura-2-thalia-en`)
- `--tags`: Add optional request tags

Example:

```bash
cargo run -- speak --voice aura-2-helena-en
```

**Interactive Usage:**

- Enter text to generate and playback speech
- Type 'quit' to exit the application

### Save Command (Save to File)

Use the `save` subcommand to generate audio and save it to a file:

```bash
cargo run -- save --text "Hello, world!" --output output.mp3
```

Required arguments:

- `--text`: The text to convert to speech
- `--output`: The output file path (e.g., `output.mp3`, `audio.wav`)

Optional arguments:

- `--voice`: Specify the voice model (default: `aura-2-thalia-en`)
- `--tags`: Add optional request tags

Example with custom voice:

```bash
cargo run -- save --text "Welcome to Deepgram" --output welcome.mp3 --voice aura-2-thalia-en
```

## Features

- Asynchronous text-to-speech generation
- Real-time audio playback with interactive input
- Save generated audio to files
- Configurable voice models
- Support for custom request tags

## Dependencies

- Tokio for async runtime
- Reqwest for HTTP requests
- Clap for CLI parsing
- Rodio for audio playback
