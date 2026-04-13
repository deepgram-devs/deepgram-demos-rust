# Velocity

Push-to-talk speech-to-text for Windows 11. Hold **Win+Ctrl+'** to record your voice, release to transcribe — the result is typed instantly into whatever application has focus.

## Requirements

- Windows 11
- A [Deepgram](https://deepgram.com) account and  API key

## Setup

1. Run `velocity.exe`.
2. On first launch a dialog will ask for your Deepgram API key. Enter it and click OK. The key is saved to `%USERPROFILE%\.config\velocity.yml` and will not be asked for again.
3. The app runs silently in the system tray.

## Usage

| Action | Result |
|---|---|
| Hold **Win+Ctrl+'** | Starts recording (you'll hear a chime) |
| Release **Win+Ctrl+'** | Stops recording, transcribes, and types the result |
| Press **Shift+Ctrl+Win+'** | Toggles keep-talking mode — recording stays active until you press the shortcut again |
| Press **Ctrl+Win+[** | Toggles streaming mode — text is typed continuously as you speak |

The transcribed text is typed into whichever window currently has focus, exactly as Deepgram returns it.

### Streaming Mode

Pressing **Ctrl+Win+[** activates streaming mode using the Deepgram WebSocket API. Speech is transcribed and typed in real time as you talk. Press the shortcut again to stop, or switch focus to a different window to stop automatically. Push-to-talk and keep-talking are disabled while streaming is active.

## Options

| Flag | Description |
|---|---|
| `--model <name>` | Deepgram model to use (default: `nova-3`) |
| `--smart-format` | Enable Deepgram Smart Formatting (punctuation, numbers, etc.) |
| `--verbose` | Log Deepgram responses and errors to the console |

Options can also be set in the configuration file:

```yaml
# %USERPROFILE%\.config\velocity.yml
api_key: your-deepgram-api-key
model: nova-3
smart_format: true
```

The configuration file is watched for changes — edits take effect without restarting.

## Models

Some commonly used Deepgram models:

| Model | Notes |
|---|---|
| `nova-3` | Best accuracy, default |
| `nova-2` | Previous generation |
| `base` | Faster, lower accuracy |

See the [Deepgram model docs](https://developers.deepgram.com/docs/models-overview) for the full list.
