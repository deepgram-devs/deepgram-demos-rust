# Velocity

Push-to-talk speech-to-text for Windows 11. Hold **Win+Ctrl+'** to record your voice, release to transcribe, and send the result to the active application using the output mode you choose.

## Requirements

- Windows 11
- A [Deepgram](https://deepgram.com) account and  API key

## Setup

1. Run `velocity.exe`.
2. On first launch a dialog will ask for your Deepgram API key. Enter it and click OK. The key is saved to `%USERPROFILE%\.config\deepgram\velocity.yml` and will not be asked for again.
3. The app runs silently in the system tray.
4. Open the tray menu and choose `Settings` to configure hotkeys, audio input, key terms, transcript history, and output mode.

## Usage

| Action | Result |
|---|---|
| Hold **Win+Ctrl+'** | Starts recording (you'll hear a chime) |
| Release **Win+Ctrl+'** | Stops recording, transcribes, and types the result |
| Press **Shift+Ctrl+Win+'** | Toggles keep-talking mode — recording stays active until you press the shortcut again |
| Press **Ctrl+Win+[** | Toggles streaming mode — text is typed continuously as you speak |
| Press **Ctrl+Win+]** | Re-send the currently selected recent transcript |

The settings window allows you to change these hotkeys. The values above are the defaults.

### Streaming Mode

Pressing **Ctrl+Win+[** activates streaming mode using the Deepgram WebSocket API. Speech is transcribed and typed in real time as you talk. Press the shortcut again to stop, or switch focus to a different window to stop automatically. Push-to-talk and keep-talking are disabled while streaming is active.

### Settings

The settings window is available from the tray menu and lets you configure:

- Deepgram API key
- transcription model
- Smart Formatting
- custom key terms, comma-separated in the settings UI
- global hotkeys
- microphone input device
- output mode:
  - type directly
  - copy to clipboard
  - paste clipboard into the active app
- append newline after transcript
- transcript history retention

When the settings window is open, the selected microphone shows a live activity indicator.

### Modern Settings UI

The repo now includes a WinUI 3 sidecar scaffold in `Velocity.Settings/README.md` for replacing the legacy Win32 configuration windows with a modern Windows 11 UI.

- If `Velocity.Settings.exe` is present next to `velocity.exe`, the Rust app will launch it for tray settings and API key onboarding.
- If the sidecar is not present, Velocity falls back to the built-in Win32 dialogs.
- Building the WinUI app requires a current .NET SDK and Windows App SDK tooling.
- Building the WinUI app from `Velocity.Settings/` also copies its output next to the workspace `velocity.exe` in `target\debug` or `target\release`.

### Transcript History

Velocity stores a recent transcript history locally in `%USERPROFILE%\.config\deepgram\velocity-history.yml`.

- The default history size is `20`.
- The tray menu shows recent transcript items.
- Choosing a recent transcript from the tray immediately re-sends it using the active output mode.
- The resend hotkey re-sends the most recently selected tray history item.

## Options

| Flag | Description |
|---|---|
| `--model <name>` | Deepgram model to use (default: `nova-3`) |
| `--smart-format` | Enable Deepgram Smart Formatting (punctuation, numbers, etc.) |
| `--verbose` | Log Deepgram responses and errors to the console |

Options can also be set in the configuration file:

```yaml
# %USERPROFILE%\.config\deepgram\velocity.yml
api_key: your-deepgram-api-key
model: nova-3
smart_format: true
keyterms:
  - Velocity
  - Deepgram
audio_input: Your microphone name
history_limit: 20
output_mode: direct-input
append_newline: false
hotkeys:
  push_to_talk: Win+Ctrl+'
  keep_talking: Win+Ctrl+Shift+'
  streaming: Win+Ctrl+[
  resend_selected: Win+Ctrl+]
```

The configuration file is watched for changes. Valid edits take effect without restarting. Invalid edits are rejected and the app keeps the last known good configuration. A backup copy is saved to `%USERPROFILE%\.config\deepgram\velocity.backup.yml` when the app runs.

## Models

Some commonly used Deepgram models:

| Model | Notes |
|---|---|
| `nova-3` | Best accuracy, default |
| `nova-2` | Previous generation |
| `base` | Faster, lower accuracy |

See the [Deepgram model docs](https://developers.deepgram.com/docs/models-overview) for the full list.
