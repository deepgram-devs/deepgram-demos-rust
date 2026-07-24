# Deepgram Voice Agent - Rust Implementation

This is a Rust application that demonstrates bidirectional conversation using Deepgram's Voice Agent WebSocket APIs. The application captures audio from your computer's microphone, streams it to Deepgram's Voice Agent, and handles the responses.

⚠️ **IMPORTANT**: Please use headphones to avoid spoken audio, from the agent, back-feeding through your microphone. Alternatively, use `--no-mic-mute` if your setup handles echo cancellation externally.

## Features

- **Real-time Audio Capture**: Captures audio from your computer's microphone using CPAL
- **WebSocket Communication**: Connects to Deepgram's Voice Agent API via WebSocket
- **Bidirectional Conversation**: Sends audio to the agent and receives responses
- **Flexible CLI Configuration**: Configure endpoint, TTS model, LLM provider, and custom headers via command-line flags
- **Reusable Agent Configurations**: Save agent settings to a Deepgram project and reuse them by UUID through dedicated subcommands
- **Audio Response Handling**: Receives and plays back audio responses from the agent using rodio
- **Smart Microphone Control**: Automatically disables microphone during agent speech and re-enables 600ms after silence (prevents feedback)
- **Silent Packet Injection**: Sends silent audio frames while muted to keep the WebSocket connection alive
- **Sample Function Calling**: Optionally register sample client-side tools (`--enable-sample-functions`) to exercise the agent's function-calling flow

## Function Calling

Pass `--enable-sample-functions` to register three client-side tools in `agent.think.functions`:

- `get_current_time` — returns the current UTC time
- `roll_dice` — rolls a die with an optional `sides` argument (default 6)
- `get_weather` — returns mock weather data for a given `location`

When the agent emits a `FunctionCallRequest` for one of these, the CLI computes a canned/mock
result locally and replies with a `FunctionCallResponse` automatically — no server endpoint is
involved. This is meant for exercising and debugging the function-calling round trip (e.g. with
different `--think-type` providers), not as a real weather/time/dice integration.

```bash
cargo run -- --enable-sample-functions --verbose
```

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
| `--listen-provider <TYPE>` | `deepgram` | STT provider type for `agent.listen.provider.type` |
| `--listen-model <MODEL>` | `nova-3` | STT model for `agent.listen.provider.model` |
| `--listen-version <VERSION>` | _(none)_ | STT model version for `agent.listen.provider.version` |
| `--listen-language <LANG>` | `en` | STT language for `agent.listen.provider.language`; omitted automatically when `--listen-model` starts with `flux-` |
| `--language-hint <CSV>` | _(none)_ | Comma-separated language hints for `agent.listen.provider.language_hints` |
| `--listen-keyterms <CSV>` | _(none)_ | Comma-separated keyterms for `agent.listen.provider.keyterms` |
| `--listen-eot-threshold <VALUE>` | _(none)_ | End-of-turn threshold for `agent.listen.provider.eot_threshold` |
| `--listen-eager-eot-threshold <VALUE>` | _(none)_ | Eager end-of-turn threshold for `agent.listen.provider.eager_eot_threshold` |
| `--listen-smart-format [true\|false]` | _(none)_ | Optional smart formatting for `agent.listen.provider.smart_format`; omitted from Settings JSON when unspecified |
| `--speak-model <MODEL>` | `aura-2-thalia-en` | TTS model for agent voice |
| `--think-type <TYPE>` | `open_ai` | LLM provider type |
| `--think-model <MODEL>` | `gpt-4o-mini` | LLM model |
| `--think-temperature <VALUE>` | _(none)_ | LLM temperature |
| `--think-endpoint <URL>` | _(none)_ | Custom URL for LLM provider |
| `--think-header <KEY=VALUE>` | _(none)_ | Extra header for LLM provider (repeatable) |
| `--think-credentials-type <iam\|sts>` | _(none)_ | AWS Bedrock credential type |
| `--think-aws-region <REGION>` | _(none)_ | AWS Bedrock region; also reads `AWS_REGION` |
| `--think-aws-access-key-id <KEY>` | _(none)_ | AWS Bedrock access key ID; also reads `AWS_ACCESS_KEY_ID` |
| `--think-aws-secret-access-key <KEY>` | _(none)_ | AWS Bedrock secret access key; also reads `AWS_SECRET_ACCESS_KEY` |
| `--think-aws-session-token <TOKEN>` | _(none)_ | Required for STS credentials; also reads `AWS_SESSION_TOKEN` |
| `--prompt <TEXT>` | Concise responses | System prompt / instructions for the agent; overrides the default concise-response prompt |
| `--verbose` | _(off)_ | Print full Settings JSON at startup and the Voice Agent request ID after connecting |
| `--no-mic-mute` | _(off)_ | Disable mic muting during playback |
| `--enable-sample-functions` | _(off)_ | Register sample client-side functions (`get_current_time`, `roll_dice`, `get_weather`) so the agent can call tools mid-conversation |

## Example Commands

`config create`, `config delete`, and all `config variable` commands accept an optional `--project-id` (or
`DEEPGRAM_PROJECT_ID`). When it is omitted, the CLI lists projects accessible
to `DEEPGRAM_API_KEY`; it automatically uses the only project, and asks for
`--project-id` when multiple projects are available.

Configuration-management commands also support `--verbose`. This prints the
exact request URL, headers, and serialized payload sent to Deepgram. The API
token and WebSocket key are redacted, which is useful for troubleshooting
request validation without exposing credentials.

```bash
# Basic usage
cargo run

# Use a different TTS voice
cargo run -- --speak-model aura-2-apollo-en

# Tune the listen provider
cargo run -- --listen-model nova-3 \
             --listen-language en \
             --language-hint "en,es" \
             --listen-keyterms "Deepgram,Voice Agent,Rust" \
             --listen-eot-threshold 0.8 \
             --listen-eager-eot-threshold 0.4 \
             --listen-smart-format \
             --verbose

# Use Claude via a custom endpoint
cargo run -- --think-type anthropic \
             --think-model claude-3-5-haiku-20241022 \
             --think-endpoint https://api.anthropic.com/v1 \
             --think-header "x-api-key=YOUR_ANTHROPIC_KEY"

# Use Amazon Bedrock with long-lived IAM credentials
cargo run -- \
             --think-type aws_bedrock \
             --think-model us.anthropic.claude-3-5-sonnet-20241022-v2:0 \
             --think-temperature 0.7 \
             --think-endpoint https://bedrock-runtime.us-east-2.amazonaws.com/ \
             --think-credentials-type iam \
             --think-aws-region us-east-2 \
             --think-aws-access-key-id "$AWS_ACCESS_KEY_ID" \
             --think-aws-secret-access-key "$AWS_SECRET_ACCESS_KEY"

# For temporary STS credentials, use --think-credentials-type sts and also
# provide --think-aws-session-token "$AWS_SESSION_TOKEN".

# AWS credentials are intentionally not allowed in `config create` reusable
# configurations because those values are visible to project members. Use an
# inline launch for Bedrock credentials.

# Connect to a self-hosted endpoint
cargo run -- --endpoint wss://my-internal-agent.example.com

# Set a custom system prompt for the agent
cargo run -- --prompt "You are a helpful assistant that speaks only in rhymes."

# Save the current agent settings as a reusable configuration
cargo run -- config create \
             --name customer-service

# Launch using a previously saved reusable configuration
cargo run -- config use YOUR_AGENT_CONFIG_UUID

# Delete a reusable configuration after verifying no active sessions reference it
cargo run -- config delete \
             --yes \
             YOUR_AGENT_CONFIG_UUID

# Reusable configurations must not contain provider headers or API keys.
# Do not combine `config create` with custom provider headers.

# If the API key has multiple projects, specify the project explicitly:
cargo run -- config create \
             --project-id YOUR_PROJECT_ID \
             --name customer-service

# Manage template variables used by reusable configurations. Values may be plain text or JSON.
# Agent variables currently require the API property `is_sensitive: false`.
cargo run -- config variable create \
             --key DG_SYSTEM_PROMPT \
             --value "You are a helpful customer service agent." \
             --verbose
cargo run -- config variable list
cargo run -- config variable get VARIABLE_ID
cargo run -- config variable update VARIABLE_ID --value '"You are a concise agent."'
cargo run -- config variable delete VARIABLE_ID --yes

# Use `config variables` as an alias for `config variable`.

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
3. **Audio format issues**: If voice recognition is not working on macOS, open **Audio MIDI Setup**, select your microphone, and switch its input format to a single audio channel (mono). Then restart the voice-agent CLI.

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
