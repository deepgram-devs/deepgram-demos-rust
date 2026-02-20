# Changelog

## 0.3.0 - 2026-02-20

### Features

- **TOML configuration file** - Application settings are now loaded from `~/.config/tts-tui.toml`, created automatically on first run with inline documentation for every option
- **API key in config** - `api.key` can be set in the config file as an alternative to the environment variable
- **Endpoint in config** - `api.endpoint` can be set in the config file; priority order is CLI flag > env var > config file > default
- **Experimental feature flags** - New `[experimental]` config section for gating in-development features; each flag can also be overridden with a `TTS_TUI_FEATURE_<NAME>=true|false` environment variable
- **Interactive API key entry** - Press `k` to open a masked popup for entering or overriding the Deepgram API key at runtime without restarting the application
- **Open cache in Finder** - Press `o` to reveal the audio cache folder in macOS Finder
- **Startup API key warning** - A yellow warning log entry is shown at startup when no API key is configured, with guidance on how to set one
- **Startup config path log** - The resolved config file path is logged at startup for easy discoverability
- **Experimental flags logged at startup** - Any enabled experimental flags are announced in the log panel on launch
- **Warning log level** - New `LogLevel::Warning` displayed in yellow with a `⚠` icon
- **Log timestamps** - Every log entry now displays an `HH:MM:SS` timestamp prefix in the log panel
- **Log scroll** - Mouse scroll wheel over the log panel scrolls through log history; title shows current scroll offset
- **Voice filter popup** - Press `/` from anywhere on the main screen to open a dedicated voice filter popup; `Enter` applies, `Esc` cancels without changing the existing filter, `Ctrl+U` clears the input
- **Deepgram brand colors** - All UI elements now use the official Deepgram color palette: Spring Green (`#13ef93`) for primary accents, Azure (`#149afb`) for focused panels and info logs, and semantic colors for Success (`#12b76a`), Warning (`#fec84b`), and Error (`#f04438`)
- **Polished popups** - Text entry, voice filter, API key, and help popups now have rounded borders, colored accents, placeholder text, cursor indicators, and keyboard shortcut hint rows
- **Mouse click to select** - Clicking a specific row in the Saved Texts or Deepgram Voices panel now selects that item directly, accounting for scroll offset and skipping non-selectable separator rows

### Changes

- Cache file references in log messages are now trimmed to the last 12 characters (e.g. `…a3f9c1b2.mp3`) instead of the full filesystem path
- API key is now resolved at startup (env var → config file) and stored in `App`; the `k` command overrides it for the current session
- Inline voice filtering by typing while the Voice panel is focused has been replaced by the explicit `/` popup for a cleaner interaction model

### Dependencies

- Added `toml 0.8` for config file parsing
- Added `chrono 0.4` for log entry timestamps

## 0.2.5 - 2026-02-11

### Features

- **Enhanced log styling** - Logs now display with color-coded icons (✓ Success in green, ✗ Error in red, ℹ Info in blue)
- **Accurate playback progress bar** - Added MP3 duration parsing for precise audio playback progress tracking
- **Audio abort on ESC** - Press ESC during audio playback to immediately stop playback
- **Voice panel organization** - Voices now grouped by language with visual separators
- **Text list metadata** - Display character count for each text
- **Scrollable help screen** - Use Up/Down arrow keys to scroll through help content on small screens
- **Mouse click support** - Click on the Saved Texts or Deepgram Voices blocks to set focus
- **Gender indicators** - Voices now display gender symbols (♂ male, ♀ female) for quick identification
- **Keyboard shortcut** - CTRL+Q to quit application from any focused panel

### Bug Fixes

- Fixed voice selection index issue when language separators were present
- Fixed audio playback state not resetting when starting new audio clips
- Ensured status bar resets and displays correctly when generating audio while playback is active

### Dependencies

- Added `mp3-duration` for accurate MP3 audio duration parsing

## 0.2.4 - 2026-02-10

- Add loading indicator during audio synthesis with responsive UI
- Fix text wrapping in "enter new text" popup box
- Remove speed query string parameter if set to 1.00x

## 0.2.3 - 2026-02-07

- Add saved text persistence to local filesystem

## 0.2.2 - 2026-02-04

- Add the TTS speed setting in the UI

## 0.2.1 - 2026-02-02

- Added support for specifying a custom endpoint with `--endpoint` or `DEEPGRAM_TTS_ENDPOINT` variable.
