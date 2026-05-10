# Velocity

Push-to-talk speech-to-text for Windows 11. Hold **Win+Ctrl+'** to record your voice, release to transcribe, and send the result to the active application using the output mode you choose.

## Requirements

- Windows 11
- A [Deepgram](https://deepgram.com) account and  API key

## Setup

### Scoop

A Scoop manifest is available at `deploy/scoop/velocity.json`.

From a local checkout:

```powershell
scoop install .\deploy\scoop\velocity.json
```

After this file is published on `main`, it can also be installed directly from the repository manifest:

```powershell
scoop install https://raw.githubusercontent.com/deepgram-devs/deepgram-demos-rust/main/velocity/deploy/scoop/velocity.json
```

Once accepted into a community Scoop bucket, the install command can move to the bucket-native package name.

### Manual

1. Run `velocity.exe`.
2. On first launch the built-in settings window will ask for your Deepgram API key. The key is saved to `%USERPROFILE%\.config\deepgram\velocity.yml` and will not be asked for again.
3. The app runs silently in the system tray.
4. Open the tray menu and choose `Settings` to configure hotkeys, audio input, key terms, transcript history, and output mode.

## Usage

| Action | Result |
|---|---|
| Hold **Win+Ctrl+'** | Starts recording (you'll hear a chime) |
| Release **Win+Ctrl+'** | Stops recording, transcribes, and types the result |
| Press **Shift+Ctrl+Win+'** | Toggles keep-talking mode — recording stays active until you press the shortcut again |
| Press **Ctrl+Win+[** | Toggles streaming mode — text is typed continuously as you speak |

The settings window allows you to change these hotkeys. The values above are the defaults.

### Streaming Mode

Pressing **Ctrl+Win+[** activates streaming mode using the Deepgram WebSocket API. Speech is transcribed and typed in real time as you talk. Press the shortcut again to stop, or switch focus to a different window to stop automatically. Push-to-talk and keep-talking are disabled while streaming is active.

### Settings

The settings window is available from the tray menu and is implemented directly in Rust with GPUI. It ships inside `velocity.exe`, so there is no separate settings sidecar to install or launch. Only one settings window can exist at a time; choosing `Settings` again focuses the existing window.

It lets you configure:

- Deepgram API key
- transcription model
- transcription language
- Smart Formatting
- custom key terms, comma-separated in the settings UI
- global hotkeys
- microphone input device
- output mode: type directly, copy to clipboard, or paste clipboard into the active app
- append newline after transcript
- focused-app delivery behavior
- transcript history retention
- launch at Windows sign-in

The current settings UI uses a compact dark layout with a Deepgram gradient heading, hover feedback on each settings section, and an unsaved-changes banner when edited values differ from the last saved configuration. Plain text fields validate as you type: invalid hotkeys and out-of-range history limits are framed with a bright validation border before save. Save failures, runtime errors, and config reload warnings are shown in the settings status area and reflected through the tray status.

When the settings window is open, the selected microphone shows a live activity indicator and the Status section shows recording, keep-talking, and streaming state.

By default, completed transcripts are sent to the app focused at delivery time. If `Send transcript to the app focused at delivery` is disabled, Velocity returns focus to the app that was active when recording started before typing or pasting the transcript.

The Windows sign-in setting creates a per-user Startup folder shortcut named `Deepgram Velocity.lnk`, starts Velocity with `--start-minimized`, and uses the Deepgram icon in Windows Startup Apps.

### Transcript History

Velocity stores a recent transcript history locally in `%USERPROFILE%\.config\deepgram\velocity-history.yml`.

- The default history size is `20`.
- The history limit controls how many transcripts are retained locally.

## Options

| Flag | Description |
|---|---|
| `--model <name>` | Deepgram model to use (default: `nova-3`) |
| `--smart-format` | Enable Deepgram Smart Formatting (punctuation, numbers, etc.) |
| `--start-minimized` | Start tray-first without opening onboarding when launched by Windows sign-in |
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
deliver_to_focused_app: true
hotkeys:
  push_to_talk: Win+Ctrl+'
  keep_talking: Win+Ctrl+Shift+'
  streaming: Win+Ctrl+[
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
