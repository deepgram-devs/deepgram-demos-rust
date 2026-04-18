# Velocity

Velocity is a Windows 11 dictation application powered by Deepgram speech-to-text APIs. The shipped app is a Rust tray process with a WinUI 3 settings sidecar when `Velocity.Settings.exe` is available, plus built-in Win32 fallback dialogs for onboarding and settings.

## Current Application State

- Current app version: `0.3.5`.
- Primary runtime: Rust background/tray app in `src/`.
- Primary settings UX: WinUI 3 sidecar in `Velocity.Settings/`.
- Fallback settings UX: native Win32 dialogs implemented in Rust.
- Configuration and transcript history live under `%USERPROFILE%\.config\deepgram`.
- The app reloads config changes while running and keeps an in-memory last-known-good config if a reload fails.

## Speech And Transcription

- Velocity supports both Deepgram pre-recorded HTTP transcription and streaming WebSocket transcription.
- Audio capture is mono `linear16` PCM at `48000` Hz.
- Streaming output uses final transcript results only; interim results are ignored.
- Supported Deepgram models are currently limited to `nova-2` and `nova-3`.
- The default model is `nova-3`.
- The config may include an optional `language` value.
- When no language is selected, Velocity omits the `language` query parameter entirely.
- Language choices are restricted by the selected model:
  - `nova-2` exposes its supported language list.
  - `nova-3` exposes its supported language list, which is broader than `nova-2`.
- Smart Formatting is supported for both pre-recorded and streaming requests.
- Custom recognition terms are supported for both request types.
- Recognition terms are stored canonically in YAML as `keyterms`.
- Velocity still accepts legacy `key_terms` on read for compatibility.
- Recognition terms are sent as repeated query parameters:
  - `keyterm` for `nova-3`
  - `keyword` for `nova-2`
- With `--verbose`, the app logs request query-string details plus API and parsing failures.

## Recording Modes

- Push to Talk:
  - Default hotkey: `Win+Ctrl+'`
  - Records while held, then submits buffered audio on release.
- Keep Talking:
  - Default hotkey: `Win+Ctrl+Shift+'`
  - Toggles recording on and off.
- Streaming:
  - Default hotkey: `Win+Ctrl+[`
  - Uses the Deepgram WebSocket API.
  - Sends `KeepAlive` messages every 5 seconds.
  - Sends Deepgram's `CloseStream` message when stopping.
  - Stops if focus changes to another window.
- Re-send selected transcript:
  - Default hotkey: `Win+Ctrl+]`
  - Re-sends the currently selected recent transcript using the active output mode.
- Streaming remains mutually exclusive with push-to-talk and keep-talking.
- Audible start and stop tones are still used for capture feedback.

## Settings Surface

Both settings UIs currently expose:

- Deepgram API key
- model selection
- language selection with a `Do not specify` option
- smart formatting
- recognition key terms
- hotkeys
- audio input selection
- transcript delivery mode
- append-newline behavior
- history retention

Current UX details:

- The WinUI sidecar is the preferred settings and onboarding path when present.
- The Win32 Rust settings window is the fallback path when the sidecar is missing.
- The WinUI sidecar supports `--page settings` and `--page api-key`.
- Both settings UIs present recognition terms as a comma-separated text value and persist them as a YAML array.
- Both settings UIs restrict language choices to the currently selected model.
- Both settings UIs show live microphone activity for the selected device.
- The WinUI sidecar warns when the config file changed on disk before the user saves.
- The Rust fallback UI validates hotkey text before saving and restores prior hotkeys if registration fails.
- The WinUI sidecar currently normalizes and saves config data, but hotkey registration validation still happens in the Rust tray process during reload.

## Audio Input

- Users can select a preferred microphone input device.
- The selected device name is persisted in config as `audio_input`.
- If the requested device cannot be opened for transcription, the Rust app surfaces an error.
- During active capture, the Rust app logs when it falls back from an unavailable requested device to the actual default device.
- The Rust fallback settings UI and the WinUI sidecar both provide live microphone metering for the selected input.

## Transcript Output And History

- Supported output modes:
  - `direct-input`
  - `clipboard`
  - `paste`
- Output mode is persisted as `output_mode`.
- Optional newline appending is persisted as `append_newline`.
- Delivered transcripts and re-sent history items both respect the active output mode.
- Transcript history is stored in `%USERPROFILE%\.config\deepgram\velocity-history.yml`.
- Default retention is `20`.
- History entries are de-duplicated by transcript text, newest-first.
- The most recent pushed transcript becomes the selected history item.
- The tray menu shows recent transcript items and marks the selected one.

## Configuration Files

- Main config file: `%USERPROFILE%\.config\deepgram\velocity.yml`
- Backup config file: `%USERPROFILE%\.config\deepgram\velocity.backup.yml`
- Transcript history file: `%USERPROFILE%\.config\deepgram\velocity-history.yml`

Current config behavior:

- Missing config loads as defaults.
- Missing or empty API key triggers onboarding.
- The Rust process writes a backup config on startup.
- The WinUI sidecar writes both the main config and backup config on save.
- Config normalization trims key terms and audio device values.
- Config normalization rejects unsupported models, unsupported model/language combinations, zero history retention, and malformed hotkey text during Rust-side load/save.
- A background watcher checks for config timestamp changes and posts a reload message to the tray process.
- If a reload is invalid, the tray process keeps the previous active config and surfaces an error in tray/settings state.

## System Tray

- Velocity runs as a tray-first application.
- Tray tooltip and status text reflect:
  - idle
  - recording active
  - streaming active
  - last error
- Tray menu actions currently include:
  - opening settings
  - toggling keep-talking
  - toggling streaming
  - selecting and re-sending recent transcripts
  - quitting the app
- Double-clicking the tray icon opens settings.

## WinUI Sidecar

- The sidecar project lives in `Velocity.Settings/`.
- It targets `net10.0-windows10.0.19041.0` and builds as `x64`.
- Rust locates the sidecar in this order:
  - `VELOCITY_SETTINGS_EXE` if set
  - `Velocity.Settings.exe` next to `velocity.exe`
  - common development build output paths under `Velocity.Settings\bin`
- The WinUI project copies its build output into the Rust workspace `target\<configuration>\` directory after build so the tray app can launch it directly from local builds.
- API key onboarding waits for the sidecar to write a non-empty API key to config, then returns that value to the Rust process.

## Testing And Documentation Status

- The Rust codebase currently includes automated tests for config normalization, key term serialization and legacy compatibility, language validation, history behavior, output formatting, hotkey parsing, and Deepgram URL construction.
- `README.md`, `CHANGELOG.md`, and `CLAUDE.md` should stay aligned with the shipped `0.3.5` behavior, especially around model/language support, sidecar behavior, and current settings capabilities.
