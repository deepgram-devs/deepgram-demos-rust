# TTS TUI (Text-to-Speech Terminal User Interface)

A terminal user interface (TUI) built with Rust and Ratatui for interacting with the Deepgram Text-to-Speech API. Supports voice selection, text and voice filtering, color themes, multi-format audio output, sample rate control, audio caching, playback speed control, timestamped logs, and a persistent TOML configuration file.

## Features

- Play saved text snippets with any Deepgram Aura or Aura-2 voice
- Browse and filter voices by name, language, or model via a dedicated popup (`/` with Voices panel focused)
- Filter saved texts by content via the same `/` key (with Saved Texts panel focused)
- Add, delete, and persist text snippets to local storage
- Audio caching — repeated playback is served from disk instantly
- **Color themes** — choose from Deepgram (default), Nord, or Synthwave Outrun via the `t` key
- **Audio format selection** — choose MP3, Linear16 (WAV), μ-law, A-law, FLAC, or AAC via the `f` key
- **Sample rate selection** — choose the output sample rate for the active format via the `s` key
- Adjustable TTS playback speed (`+`/`-`/`0` keys)
- Interactive API key entry — set or override the key at runtime without restarting (`k`)
- Open the audio cache folder in Finder with a single keystroke (`o`)
- TOML configuration file at `~/.config/tts-tui.toml` with inline documentation
- Experimental feature flags via config file or environment variables
- Timestamped, color-coded log panel with scrollable history and mouse scroll support
- Mouse click to select specific items in lists

## Requirements

- Rust and Cargo (install via [rustup.rs](https://rustup.rs))
- A [Deepgram API key](https://console.deepgram.com/)

## Getting Started

```bash
cd tts-tui
cargo run
```

On first launch the app creates `~/.config/tts-tui.toml` with all options documented inline. If no API key is detected you will see a warning in the log panel — press `k` to enter one interactively.

## Configuration

Settings are resolved in this priority order (highest wins):

```
CLI arguments  >  environment variables  >  ~/.config/tts-tui.toml  >  built-in defaults
```

### API Key

Three ways to supply your Deepgram API key, in priority order:

```bash
# 1. Interactive — press 'k' inside the running app

# 2. Environment variable
export DEEPGRAM_API_KEY="your-api-key"

# 3. Config file (~/.config/tts-tui.toml)
# [api]
# key = "your-api-key"
```

### Custom Endpoint

For self-hosted deployments or non-production environments:

```bash
# CLI flag (highest priority)
cargo run -- --endpoint-override https://selfhosted.example.com/v1/speak

# Environment variable
export DEEPGRAM_TTS_ENDPOINT=https://selfhosted.example.com/v1/speak
cargo run

# Config file
# [api]
# endpoint = "https://selfhosted.example.com/v1/speak"
```

### Audio Format and Sample Rate

The output encoding and sample rate can be set via CLI, environment variable, or config file:

```bash
# CLI flags
cargo run -- --audio-format flac --sample-rate 48000

# Environment variables
export DEEPGRAM_AUDIO_FORMAT=linear16
export DEEPGRAM_SAMPLE_RATE=24000
cargo run

# Config file (~/.config/tts-tui.toml)
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

### Experimental Feature Flags

In-development features can be enabled in `~/.config/tts-tui.toml`:

```toml
[experimental]
# Stream audio playback as bytes arrive instead of waiting for the full download.
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

### Main Screen

| Key | Action |
|-----|--------|
| `?` | Show help screen |
| `q` | Quit (when Saved Texts panel is focused) |
| `Ctrl+Q` | Quit from any panel |
| `Enter` | Play selected text with selected voice |
| `n` | Add new text snippet |
| `d` | Delete selected text snippet |
| `k` | Set Deepgram API key interactively |
| `o` | Open audio cache folder in Finder |
| `/` | Open filter popup for focused panel |
| `t` | Select color theme |
| `f` | Select audio encoding format |
| `s` | Select output sample rate |
| `Up` / `Down` | Navigate Saved Texts or Voices list |
| `Left` | Focus previous panel |
| `Right` / `Tab` | Focus next panel |
| `+` / `=` | Increase playback speed |
| `-` | Decrease playback speed |
| `0` | Reset playback speed to 1.0x |
| `Esc` | Stop audio playback / clear active filter / close popup |
| `Backspace` | Remove last character from active filter |

### Filter Popups (`/`)

Press `/` to open a filter for the currently focused panel. The Saved Texts filter matches on text content; the Deepgram Voices filter matches on voice name, language, or model.

| Key | Action |
|-----|--------|
| Type | Narrow the list in real time (match count shown in title) |
| `Enter` | Apply filter and close popup |
| `Esc` | Cancel without changing the current filter |
| `Ctrl+U` | Clear all filter text |
| `Backspace` | Delete last character |

### Theme Select Popup (`t`)

| Key | Action |
|-----|--------|
| `Up` / `Down` | Navigate themes |
| `Enter` | Apply theme and close popup |
| `Esc` | Cancel |

### Text Entry

| Key | Action |
|-----|--------|
| `Enter` | Save text |
| `Esc` | Cancel |
| `Ctrl+V` / `Cmd+V` | Paste from clipboard |
| `Backspace` | Delete last character |

### API Key Entry

| Key | Action |
|-----|--------|
| `Enter` | Save key (overrides env var for this session) |
| `Esc` | Cancel |
| `Backspace` | Delete last character |

### Help Screen

| Key | Action |
|-----|--------|
| `Up` / `Down` | Scroll help text |
| `Esc` | Close help screen |

### Audio Format / Sample Rate Popups

Press `f` or `s` to open. Arrow keys navigate; `Enter` applies; `Esc` cancels.

| Key | Action |
|-----|--------|
| `Up` / `Down` | Navigate options |
| `Enter` | Apply selection and close popup |
| `Esc` | Cancel without changing the current setting |

### Mouse Controls

| Action | Effect |
|--------|--------|
| Click on item | Select that item and focus its panel |
| Scroll wheel over Saved Texts or Voices | Scroll the list |
| Scroll wheel over Logs | Scroll log history |
