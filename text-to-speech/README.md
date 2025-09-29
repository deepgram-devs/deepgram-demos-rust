# Deepgram Text-to-Speech CLI

This Rust application provides a command-line interface for generating speech using the Deepgram Text-to-Speech API.

## Prerequisites

- Rust (latest stable version)
- Deepgram API Key

## Setup

1. Clone the repository
2. Set your Deepgram API key as an environment variable:
   ```
   export DEEPGRAM_API_KEY=your_api_key_here
   ```
   Or create a `.env` file in the project root with:
   ```
   DEEPGRAM_API_KEY=your_api_key_here
   ```

## Usage

Run the application with the `speak` subcommand:

```bash
cargo run -- speak
```

Optional arguments:
- `--voice`: Specify the voice model (default: "aura-2")
- `--language`: Specify the language (default: "en-US")
- `--tags`: Add optional request tags
- `--callback-url`: Provide an optional callback URL

Example:
```bash
cargo run -- speak --voice aura-2 --language en-US
```

### Interactive Usage

- Enter text to generate speech
- Type 'quit' to exit the application

## Features

- Asynchronous text-to-speech generation
- Real-time audio playback
- Configurable voice and language settings

## Dependencies

- Tokio for async runtime
- Reqwest for HTTP requests
- Clap for CLI parsing
- Rodio for audio playback

## Error Handling

The application provides detailed error messages for:
- Missing API key
- Network request failures
- Audio playback issues

## License

[Include your project's license information]
