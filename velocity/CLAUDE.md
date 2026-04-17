# Velocity

Velocity is a Windows 11 dictation application powered by Deepgram speech-to-text APIs. It runs primarily as a tray app written in Rust, with a WinUI 3 settings sidecar for a more modern configuration experience.

## Core Direction

- Keep the background engine in Rust.
- Use the `windows` crate for native Windows integration.
- Prefer current stable Rust dependency versions.
- Maintain a lightweight tray-first workflow, but support a comprehensive settings experience for configuration and onboarding.
- Keep README and test coverage aligned with shipped functionality.

## Speech And Transcription

- Support both Deepgram pre-recorded HTTP transcription and streaming WebSocket transcription.
- Capture mono `linear16` PCM audio at `48000` Hz.
- Do not use interim streaming transcript results for output.
- Respect the configured Deepgram model, defaulting to `nova-3`.
- Support Deepgram Smart Formatting.
- Support custom recognition terms for both pre-recorded and streaming requests.
- Persist recognition terms in config as a YAML array under `keyterms`.
- In settings UIs, display and edit recognition terms as a comma-separated text value.
- When sending Deepgram requests:
  - Use `keyterm` for `nova-3`.
  - Use `keyword` for `nova-2`.
  - Send one query parameter per term, repeating the parameter for multiple terms.
- When `--verbose` is specified, log Deepgram request query string parameters and API/parse errors.

## User Interface

- The application runs from the Windows system tray.
- The tray menu must expose meaningful state and actions, not just quit behavior.
- Provide a settings experience reachable from the tray.
- Prefer launching the WinUI 3 settings sidecar when available.
- Keep the built-in Rust settings window as a fallback path if the sidecar is unavailable.
- The settings experience must let the user configure major application behavior without manual YAML editing.
- Surface validation failures clearly and do not silently discard invalid values.
- Continue using audible start/stop cues for capture feedback.

## Settings Requirements

- The settings experience must include:
  - Deepgram API key
  - transcription model
  - smart formatting
  - recognition key terms
  - hotkeys
  - selected microphone
  - transcript delivery mode
  - append-newline behavior
  - history retention
- Recognition key terms must be shown as a comma-separated textbox value in the UI.
- Saving from the UI must parse the textbox as a comma-separated list and persist a YAML array to config.
- The settings UI should reflect live microphone activity for the selected input when practical.

## Hotkeys

- Use `RegisterHotKey` for global hotkeys.
- Hotkeys must be configurable by the user.
- Preserve these defaults:
  - Push to Talk: `Win+Ctrl+'`
  - Keep Talking: `Win+Ctrl+Shift+'`
  - Streaming: `Win+Ctrl+[`
  - Resend Selected Transcript: `Win+Ctrl+]`
- Validate hotkey text before saving.
- If a new hotkey binding cannot be registered, keep the previous known-good bindings active and report the error.

## Recording Modes

- Push to Talk:
  - Record only while the shortcut is held.
  - On release, send the buffered audio to the pre-recorded Deepgram API.
- Keep Talking:
  - Toggle recording on/off with its hotkey.
- Streaming:
  - Toggle streaming mode with its hotkey.
  - Use the Deepgram WebSocket API.
  - Send `KeepAlive` messages every 5 seconds.
  - Close the stream with the documented `CloseStream` message when stopping.
  - Stop immediately if the user changes focus to another window.
- While streaming is active, push-to-talk and keep-talking must not run.

## Audio Input

- Let the user choose the sound input device used for transcription.
- Persist the selected input device in configuration.
- Expose available input devices in settings through a combo box.
- If the selected device is unavailable, fail safely and surface the issue.
- Show microphone activity so the user can verify the selected mic is receiving audio.

## Transcript Output

- Support these delivery modes:
  - direct keyboard input
  - copy to clipboard
  - paste from clipboard into the active application
- Support appending a newline after the transcript.
- Persist output behavior in configuration.
- Re-sent transcripts must respect the currently selected output mode.

## Transcript History

- Save transcript history locally.
- Default retention to `20` items.
- Let the user configure the retention count.
- Show recent transcripts in the tray menu.
- Let the user select a recent transcript from the tray and resend it.
- Provide a hotkey that resends the currently selected recent transcript.

## Configuration

- Store config under `%USERPROFILE%\.config\deepgram`.
- Use `velocity.yml` as the main config file.
- Use `velocity.backup.yml` as the backup copy.
- Use `velocity-history.yml` for transcript history.
- If the config file is missing or the API key is unavailable, prompt for API key entry through the settings/onboarding flow.
- Persist the Deepgram API key after entry.
- Reload config changes while the app is running.
- If a config change is valid, apply it without restart whenever practical.
- If a config change is invalid, reject it, keep the last known good configuration active, and surface the error.
- Always maintain a backup config copy when the app runs.
- The canonical YAML field for recognition terms is `keyterms`.
- The app may continue reading legacy `key_terms` for compatibility, but should write `keyterms`.

## System Tray

- Show meaningful current state in the tray menu.
- Reflect whether recording is active.
- Reflect whether keep-talking is active.
- Reflect whether streaming is active.
- Include tray actions for:
  - opening settings
  - toggling keep-talking
  - toggling streaming
  - resending recent transcripts
  - quitting the app
- Keep tray state synchronized with actual runtime state.

## WinUI Sidecar

- Keep the WinUI 3 settings app in `Velocity.Settings/`.
- Use it for API key onboarding and settings when available.
- Make sure it reads and writes the same `velocity.yml` file as the Rust process.
- Keep its behavior aligned with the Rust fallback settings UI.

## Miscellaneous Requirements

- Use native Windows APIs for text input, clipboard behavior, tray integration, and audio device access.
- Support a `CTRL+C` handler to exit cleanly when running in a terminal.

## Documentation And Testing

- Keep `README.md` user-focused and current with shipped behavior.
- Maintain automated tests for config parsing/serialization, key term behavior, output formatting, history logic, and hotkey parsing.
- Add or update manual test steps when behavior depends on Windows UI, tray integration, microphone devices, or OS-specific interactions.
