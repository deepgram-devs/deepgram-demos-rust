# TTS TUI (Text-to-Speech Terminal User Interface)

A terminal user interface (TUI) built with Rust and Ratatui for interacting with Deepgram Text-to-Speech. Supports hosted Deepgram, self-hosted Deepgram-compatible HTTP endpoints, and self-hosted Deepgram deployed on Amazon SageMaker, plus voice selection, text and voice filtering, color themes, multi-format audio output, sample rate control, optional volume normalization, audio caching, playback speed control, timestamped logs, playback queue, favorite voices, a command palette, and a persistent TOML configuration file.

## Features

- Play saved text snippets with any Deepgram Aura, Aura-2, or Flux voice
- **Flux TTS support** — all 36 Flux voices (`flux-*-en`) are selectable alongside Aura and Aura-2; requests are automatically routed to Deepgram's `/v2/speak` endpoint
- Choose the TTS provider: Deepgram-compatible HTTP endpoint or Amazon SageMaker `InvokeEndpoint`
- Browse and filter voices by name, language, or model via a dedicated popup (`/` with Voices panel focused)
- Filter saved texts by content via the same `/` key (with Saved Texts panel focused)
- Add, edit (`e`), delete, and reorder (`Ctrl+Up`/`Ctrl+Down`) text snippets with full persistence
- **Playback queue** — press `Space` to enqueue text+voice pairs; auto-advances between tracks; queue count shown in status bar
- **Favorite voices** — press `*` to star/unstar a voice (`★` indicator); favorites persisted to disk
- **Command Palette** (`Ctrl+P`) — search and invoke any action by name, with keyboard shortcuts shown
- Audio caching — repeated playback is served from disk instantly
- **Color themes** — choose from Deepgram (default), Nord, or Synthwave Outrun via the `t` key
- **Audio format selection** — choose MP3, Linear16 (WAV), μ-law, A-law, FLAC, or AAC via the `f` key
- **Sample rate selection** — choose the output sample rate for the active format via the `s` key
- **Volume normalization** — toggle normalized output at runtime with `v`
- **WebSocket TTS streaming** — stream hosted Aura, Aura-2, and Flux TTS audio over Deepgram WebSockets with `w`; Aura supports selectable 10-word, sentence, or punctuation chunking with `c`
- Adjustable TTS playback speed (`+`/`-`/`0` keys)
- Interactive API key entry — set or override the key at runtime without restarting (`k`)
- Open the audio cache folder with a single keystroke (`o`)
- TOML configuration file at `~/.config/deepgram/deepgram-tts-client.toml` with inline documentation
- Persistent rotating log at `~/.config/deepgram/tts-tui.log` (1 MiB per file, three files retained by default)
- Experimental feature flags via config file or environment variables
- Timestamped, color-coded log panel with scrollable history and mouse scroll support
- Mouse click to select specific items in lists
- Deepgram request tags: every request includes `tts-tui`, `appeng`, and `deepgram-demos-rust`; add more with `--tags`
- Identifies direct HTTP and WebSocket requests with a versioned `User-Agent` header (`tts-tui/<version>`)

## Requirements

- Rust and Cargo (install via [rustup.rs](https://rustup.rs))
- For `deepgram` provider mode: a hosted or self-hosted Deepgram-compatible HTTP endpoint and any required API key
- For `sagemaker` provider mode: AWS credentials with `sagemaker:InvokeEndpoint` permission and a self-hosted Deepgram TTS SageMaker endpoint in service

## Getting Started

```bash
cd tts-tui
cargo run
```

On first launch the app creates `~/.config/deepgram/deepgram-tts-client.toml` with all options documented inline. Existing configurations are moved automatically from `~/.config/deepgram-tts-client.toml`. In `deepgram` mode, if no API key is detected you will see a warning in the log panel, but requests can still be sent without an `Authorization` header for endpoints that do not require one. Press `k` to enter a key interactively if your hosted or self-hosted endpoint requires one. In `sagemaker` mode, the app uses the standard AWS credential chain instead of a Deepgram API key.

## Configuration

Settings are resolved in this priority order (highest wins):

```
CLI arguments  >  environment variables  >  ~/.config/deepgram/deepgram-tts-client.toml  >  built-in defaults
```

### Provider

Use Deepgram-compatible HTTP mode by default. Without `--endpoint`, this targets hosted Deepgram:

```bash
cargo run
```

### Logging

`tts-tui` writes application events to `~/.config/deepgram/tts-tui.log`. The active file rotates when it reaches 1 MiB by default; `tts-tui.log.1` and `tts-tui.log.2` are retained, for three log files total. Batch requests log the Deepgram `dg-request-id` response header; Aura WebSocket streaming logs the request ID from `Metadata`, Flux logs it from `Connected`, and both log every successfully sent `Speak` text chunk.

Configure the limits in the TOML file, or override them with `TTS_TUI_LOG_MAX_SIZE_BYTES`, `TTS_TUI_LOG_MAX_FILES`, `--log-max-size-bytes`, or `--log-max-files`:

```toml
[logging]
max_size_bytes = 1048576
max_files = 3
```

Use self-hosted Deepgram TTS on Amazon SageMaker through the AWS SageMaker Runtime `InvokeEndpoint` API:

```bash
cargo run -- \
  --provider sagemaker \
  --sagemaker-endpoint-name your-sagemaker-endpoint \
  --aws-region us-east-2
```

The same settings can be supplied with environment variables:

```bash
export TTS_TUI_PROVIDER=sagemaker
export SAGEMAKER_ENDPOINT_NAME=your-sagemaker-endpoint
export AWS_REGION=us-east-2
cargo run
```

Or in `~/.config/deepgram/deepgram-tts-client.toml`:

```toml
[api]
provider = "sagemaker"

[sagemaker]
endpoint_name = "your-sagemaker-endpoint"
region = "us-east-2"
```

#### SageMaker InvokeEndpoint transport

When `--provider sagemaker` is selected, `tts-tui` does not send an HTTP request to the Deepgram endpoint URL. Instead, it uses the AWS SDK SageMaker Runtime client and calls `InvokeEndpoint` on the configured SageMaker endpoint name.

The `InvokeEndpoint` request uses:

- `EndpointName`: the value from `--sagemaker-endpoint-name`, `SAGEMAKER_ENDPOINT_NAME`, or `[sagemaker].endpoint_name`
- `ContentType`: `application/json`
- `Accept`: an audio MIME type based on the selected encoding, such as `audio/mpeg`, `audio/wav`, or `audio/flac`
- Body: JSON shaped like `{"text":"..."}`
- `CustomAttributes`: Deepgram-compatible routing and TTS query parameters, such as `v1/speak?model=aura-2-thalia-en&encoding=linear16&sample_rate=24000`

AWS credentials and region are resolved through the standard AWS SDK configuration chain. Deepgram API keys and the `--endpoint` / `DEEPGRAM_TTS_ENDPOINT` HTTP URL setting are only used by the `deepgram` provider, not by the SageMaker transport.

### API Key

Three ways to supply your Deepgram API key, in priority order:

```bash
# 1. Interactive — press 'k' inside the running app

# 2. Environment variable
export DEEPGRAM_API_KEY="your-api-key"

# Windows PowerShell
$env:DEEPGRAM_API_KEY = "your-api-key"

# Windows Command Prompt
set DEEPGRAM_API_KEY=your-api-key

# 3. Config file (~/.config/deepgram/deepgram-tts-client.toml)
# [api]
# key = "your-api-key"
```

On Windows, `tts-tui` also checks `USERPROFILE` when resolving its configuration directory, so environment-variable overrides work even when `HOME` is not defined. Set the variable before launching `tts-tui` from the same terminal session.

API keys are only used in `deepgram` provider mode, which includes hosted and self-hosted Deepgram-compatible HTTP endpoints. If no key is configured, `tts-tui` sends the request without an `Authorization` header. SageMaker mode authenticates with AWS credentials from the standard AWS SDK chain, such as environment variables, shared AWS config files, SSO, or an IAM role.

### Custom Endpoint

For self-hosted Deepgram-compatible HTTP deployments, proxies, or non-production hosted environments:

```bash
# Hosted regional base URL; the app automatically uses /v1/speak
cargo run -- --provider deepgram --endpoint https://api.eu.deepgram.com

# CLI flag (highest priority)
cargo run -- --provider deepgram --endpoint https://selfhosted.example.com/v1/speak

# Environment variable
export DEEPGRAM_TTS_ENDPOINT=https://selfhosted.example.com/v1/speak
cargo run

# Config file
# [api]
# endpoint = "https://selfhosted.example.com/v1/speak"
```

This setting applies to the `deepgram` provider, which is the direct HTTP path for hosted or self-hosted Deepgram-compatible TTS. SageMaker provider configuration uses `[sagemaker].endpoint_name` and `[sagemaker].region` instead.

Both HTTP and HTTPS endpoint URLs are supported. If the endpoint is only a scheme and host, such as `https://api.eu.deepgram.com`, `tts-tui` sends requests to `/v1/speak` on that host. If the URL already includes a path, that path is preserved.

### Request Tags

Every Deepgram TTS request includes the default tags `tts-tui`, `appeng`, and `deepgram-demos-rust`. Add custom tags with `--tags`; repeat the option or provide comma-separated values:

```bash
cargo run -- --tags production --tags demo
cargo run -- --tags production,demo
```

Tags are sent as repeated `tag` query parameters for HTTP and WebSocket requests, and in SageMaker `CustomAttributes`.

### Flux TTS

[Flux](https://developers.deepgram.com/docs/flux-tts/overview) is Deepgram's TTS model built for voice agent pipelines. All 36 Flux voices are available in the Voices panel, filterable with `flux` (e.g. `/` then type `flux`):

| Voice ID | Name | Accent | Gender |
|----------|------|--------|--------|
| `flux-hannah-en` | Hannah | American | Female |
| `flux-kit-en` | Kit | British | Male |
| `flux-alexis-en` | Alexis | American | Female |
| `flux-cliff-en` | Cliff | American | Male |
| `flux-sienna-en` | Sienna | American | Female |
| `flux-cole-en` | Cole | American | Male |
| `flux-brooke-en` | Brooke | American | Female |
| `flux-colin-en` | Colin | British | Male |
| `flux-gemma-en` | Gemma | British | Female |
| `flux-haley-en` | Haley | American | Female |
| `flux-heather-en` | Heather | American | Female |
| `flux-miles-en` | Miles | American | Male |
| `flux-sean-en` | Sean | British | Male |
| `flux-bree-en` | Bree | American | Female |
| `flux-brittany-en` | Brittany | American | Female |
| `flux-bruce-en` | Bruce | American | Male |
| `flux-conor-en` | Conor | British | Male |
| `flux-donovan-en` | Donovan | American | Male |
| `flux-drew-en` | Drew | American | Male |
| `flux-elise-en` | Elise | American | Female |
| `flux-jack-en` | Jack | British | Male |
| `flux-kai-en` | Kai | Singaporean | Male |
| `flux-kelsey-en` | Kelsey | American | Female |
| `flux-maeve-en` | Maeve | Irish | Female |
| `flux-marcelo-en` | Marcelo | Filipino | Male |
| `flux-marcus-en` | Marcus | American | Male |
| `flux-meena-en` | Meena | Indian | Female |
| `flux-meghan-en` | Meghan | American | Female |
| `flux-naveen-en` | Naveen | Indian | Male |
| `flux-paige-en` | Paige | American | Female |
| `flux-priya-en` | Priya | Indian | Female |
| `flux-rufus-en` | Rufus | British | Male |
| `flux-sharon-en` | Sharon | Australian | Female |
| `flux-tanner-en` | Tanner | British | Male |
| `flux-wade-en` | Wade | American | Male |
| `flux-wes-en` | Wes | American | Male |

Selecting a Flux voice on the `deepgram` provider automatically sends the request to `/v2/speak` instead of `/v1/speak` — no configuration change is needed, including with a hosted regional base URL. Aura and Aura-2 voices continue to use `/v1/speak` unchanged.

Flux does not currently document support for the `speed` or `normalize_volume` query parameters, so `tts-tui` omits both when a Flux voice is selected rather than sending values the API may reject. If playback speed or volume normalization is active while a Flux voice plays, the log panel notes that the setting was ignored for that request. All standard audio formats (MP3, Linear16, μ-law, A-law, FLAC, AAC) are supported by Flux voices. Flux is not yet supported through the `sagemaker` provider.

### Audio Format and Sample Rate

The output encoding and sample rate can be set via CLI, environment variable, or config file:

```bash
# CLI flags
cargo run -- --audio-format flac --sample-rate 48000

# Environment variables
export DEEPGRAM_AUDIO_FORMAT=linear16
export DEEPGRAM_SAMPLE_RATE=24000
cargo run

# Config file (~/.config/deepgram/deepgram-tts-client.toml)
# [audio]
# format = "flac"
# sample_rate = 48000
```

Supported formats and their valid sample rates:

| Format | Encoding value | Valid sample rates |
|--------|---------------|-------------------|
| MP3 | `mp3` | 22050 Hz |
| Linear16 (WAV) | `linear16` | 8000, 16000, 24000, 32000, 48000 Hz |
| μ-law | `mulaw` | 8000, 16000 Hz |
| A-law | `alaw` | 8000, 16000 Hz |
| FLAC | `flac` | 8000, 16000, 22050, 32000, 48000 Hz |
| AAC | `aac` | 22050 Hz |

You can also change format and sample rate interactively with the `f` and `s` keys while the app is running. Switching formats automatically snaps the sample rate to a valid value if needed.

#### Volume normalization

Volume normalization is disabled by default. Press `v` from the main screen to toggle it for new requests; the status bar and log confirm the active state. The default state can still be set with `DEEPGRAM_NORMALIZE_VOLUME=true` or `[audio].normalize_volume = true` in `~/.config/deepgram/deepgram-tts-client.toml`.

When enabled, the app adds `normalize_volume=true` to the Deepgram TTS query parameters for both the HTTP and SageMaker providers. The setting is included in the audio cache key.

#### WebSocket TTS streaming

Press `w` on the main screen to switch the hosted Deepgram provider to streaming TTS. Aura and Aura-2 voices use `wss://api.deepgram.com/v1/speak`, authenticate through `Sec-WebSocket-Protocol`, and send the selected chunks as `Speak` messages. Flux voices use `wss://api.deepgram.com/v2/speak`, authenticate with `Authorization: Token ...`, and send the complete saved text as one `Speak` message so whitespace is preserved. Both modes send a final `Flush`; playback begins as binary Linear16 audio arrives and continues until the protocol's completion messages have been received and the audio sink is empty.

Press `c` to choose a chunking strategy for the next stream:

- **10 words** — sends groups of ten words.
- **Sentence boundary** — separates at `.`, `!`, and `?`.
- **Punctuation** — separates at sentence punctuation plus commas and semicolons.

Hosted streaming requires a Deepgram API key; self-hosted Deepgram-compatible endpoints may accept WebSocket connections without one. The configured HTTP(S) endpoint is converted to its WebSocket equivalent, and authentication headers are sent only when a key is configured. Streaming always uses Linear16 audio; the app uses the selected sample rate when it is compatible, otherwise 24000 Hz. For Flux, the app waits for `Flushed`, `SpeechMetadata`, and graceful `SessionMetadata` closure before completing. Press `Esc` to abort a stream; otherwise the app plays the complete received buffer. SageMaker remains unavailable through WebSocket streaming.

### Experimental Feature Flags

In-development features can be enabled in `~/.config/deepgram/deepgram-tts-client.toml`:

```toml
[experimental]
# Enable Deepgram WebSocket TTS streaming on startup (or press 'w' at runtime).
streaming_playback = false

# Allow SSML markup tags in text input for fine-grained speech control.
ssml_support = false
```

Each flag can also be toggled with an environment variable:

```bash
TTS_TUI_FEATURE_STREAMING_PLAYBACK=true cargo run
TTS_TUI_FEATURE_SSML_SUPPORT=true cargo run
```

## Keyboard Shortcuts

### Main Screen — Saved Texts panel

| Key | Action |
|-----|--------|
| `Enter` | Play selected text with selected voice |
| `Ctrl+Enter` | Play selected text, bypassing the audio cache (force regenerate) |
| `Space` | Enqueue selected text+voice for sequential playback |
| `n` | Add new text snippet |
| `e` | Edit selected text in place |
| `d` | Delete selected text snippet |
| `Ctrl+Up` | Move selected text up |
| `Ctrl+Down` | Move selected text down |
| `q` | Quit (when Saved Texts panel is focused) |

### Main Screen — Voices panel

| Key | Action |
|-----|--------|
| `*` | Toggle favorite (★) on selected voice |

### Main Screen — Any panel

| Key | Action |
|-----|--------|
| `Ctrl+P` | Open Command Palette |
| `?` | Show help screen |
| `Ctrl+Q` | Quit from any panel |
| `/` | Open filter popup for focused panel |
| `t` | Select color theme |
| `f` | Select audio encoding format |
| `s` | Select output sample rate |
| `v` | Toggle volume normalization |
| `w` | Toggle Deepgram WebSocket streaming |
| `c` | Cycle streaming chunking strategy |
| `k` | Set Deepgram API key interactively |
| `o` | Open audio cache folder |
| `Up` / `Down` | Navigate list |
| `Left` | Focus previous panel |
| `Right` / `Tab` | Focus next panel |
| `+` / `=` | Increase playback speed |
| `-` | Decrease playback speed |
| `0` | Reset playback speed to 1.0x |
| `Esc` | Stop audio / clear active filter / close popup |

### Filter Popups (`/`)

Press `/` to open a filter for the currently focused panel. The Saved Texts filter matches on text content; the Deepgram Voices filter matches on voice name, language, or model.

| Key | Action |
|-----|--------|
| Type | Narrow the list in real time (match count shown in title) |
| `Enter` | Apply filter and close popup |
| `Esc` | Cancel without changing the current filter |
| `Backspace` (empty field) | Cancel without changing the current filter |
| `Ctrl+U` | Clear all filter text |
| `Backspace` | Delete last character |

### Theme Select Popup (`t`)

| Key | Action |
|-----|--------|
| `Up` / `Down` | Navigate themes |
| `Enter` | Apply theme and close popup |
| `Esc` / `q` | Cancel |

### Text Entry

| Key | Action |
|-----|--------|
| `Enter` | Save text |
| `Esc` | Cancel |
| `Backspace` (empty field) | Cancel |
| `Ctrl+V` / `Cmd+V` | Paste from clipboard |
| `Backspace` | Delete last character |

### API Key Entry

| Key | Action |
|-----|--------|
| `Enter` | Save key (overrides env var for this session) |
| `Esc` | Cancel |
| `Backspace` (empty field) | Cancel |
| `Backspace` | Delete last character |

### Help Screen

| Key | Action |
|-----|--------|
| `Up` / `Down` | Scroll help text |
| `Esc` / `q` | Close help screen |

### Audio Format / Sample Rate Popups

Press `f` or `s` to open. Arrow keys navigate; `Enter` applies; `Esc` or `q` cancels.

| Key | Action |
|-----|--------|
| `Up` / `Down` | Navigate options |
| `Enter` | Apply selection and close popup |
| `Esc` / `q` | Cancel without changing the current setting |

### Mouse Controls

| Action | Effect |
|--------|--------|
| Click on item | Select that item and focus its panel |
| Scroll wheel over Saved Texts or Voices | Scroll the list |
| Scroll wheel over Logs | Scroll log history |
