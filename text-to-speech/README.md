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

Run the application with the `speak` subcommand:

```bash
cargo run -- speak
```

Optional arguments:

- `--voice`: Specify the voice model (eg. `aura-2-helena-en`)
- `--tags`: Add optional request tags
- `--callback-url`: Provide an optional callback URL

Example:

```bash
cargo run -- speak --voice aura-2 --language en-US
```

### Interactive Usage

- Enter text to generate and playback speech
- Type 'quit' to exit the application

## Features

- Asynchronous text-to-speech generation
- Real-time audio playback
- Configurable voice

## Dependencies

- Tokio for async runtime
- Reqwest for HTTP requests
- Clap for CLI parsing
- Rodio for audio playback
