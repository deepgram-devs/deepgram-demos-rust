# TTS TUI (Text-to-Speech Terminal User Interface)

A terminal user interface (TUI) built with Rust and Ratatui for interacting with the Deepgram Text-to-Speech API. Supports voice selection, voice filtering, audio caching, playback speed control, timestamped logs, and a persistent TOML configuration file, all styled with the Deepgram brand color palette.

## Features

- Play saved text snippets with any Deepgram Aura or Aura-2 voice
- Browse and filter voices by name, language, or model via a dedicated popup (`/`)
- Add, delete, and persist text snippets to local storage
- Audio caching — repeated playback is served from disk instantly
- Adjustable TTS playback speed (`+`/`-`/`0` keys)
- Interactive API key entry — set or override the key at runtime without restarting
- Open the audio cache folder in Finder with a single keystroke (`o`)
- TOML configuration file at `~/.config/tts-tui.toml` with inline documentation
- Experimental feature flags via config file or environment variables
- Timestamped, color-coded log panel with scrollable history and mouse scroll support
- Mouse click to select specific items in lists
- Deepgram brand color palette throughout the UI

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
| `/` | Open voice filter popup |
| `Up` / `Down` | Navigate Saved Texts or Voices list |
| `Left` | Focus previous panel |
| `Right` / `Tab` | Focus next panel |
| `+` / `=` | Increase playback speed |
| `-` | Decrease playback speed |
| `0` | Reset playback speed to 1.0x |
| `Esc` | Stop audio playback / clear voice filter |

### Voice Filter Popup

Press `/` to open. Filter matches on voice name, language, or model.

| Key | Action |
|-----|--------|
| Type | Narrow the voice list in real time |
| `Enter` | Apply filter and close popup |
| `Esc` | Cancel without changing the current filter |
| `Ctrl+U` | Clear all filter text |
| `Backspace` | Delete last character |

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

### Mouse Controls

| Action | Effect |
|--------|--------|
| Click on item | Select that item and focus its panel |
| Scroll wheel over Saved Texts or Voices | Scroll the list |
| Scroll wheel over Logs | Scroll log history |
