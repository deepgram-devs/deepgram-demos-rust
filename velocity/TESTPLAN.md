# Velocity Test Plan

## Automated Test Plan

Run the package test suite:

```powershell
$env:CARGO_TARGET_DIR='C:\Users\TrevorSullivan\.codex\memories\velocity-target'
cargo test -p velocity
```

Automated coverage currently verifies:

- Configuration defaults for new settings fields.
- Configuration normalization for key terms, audio device names, and invalid history limits.
- Hotkey parsing, formatting, and invalid hotkey rejection.
- Transcript history insertion, deduplication, selection, and limit trimming.
- Output formatting for append-newline behavior.
- Deepgram pre-recorded URL construction with smart formatting and key terms.
- Deepgram streaming URL construction with smart formatting and key terms.
- Audio peak meter scaling helpers.

## Manual Test Plan

### 1. Settings UI

- Launch `velocity.exe`.
- Open the tray menu and choose `Settings`.
- Verify the settings window opens and can be focused again from the tray.
- Verify the window shows fields for API key, model, smart formatting, key terms, hotkeys, audio device, history limit, output mode, and append-newline.

### 2. Hotkey Configuration

- In settings, change each hotkey to a non-conflicting binding and save.
- Verify the new hotkeys work immediately without restarting.
- Enter an invalid hotkey string and verify save is rejected with a visible error.
- Restore defaults and verify:
  - `Win+Ctrl+'` starts push-to-talk.
  - `Win+Ctrl+Shift+'` toggles keep-talking.
  - `Win+Ctrl+[` toggles streaming.
  - `Win+Ctrl+]` resends the selected recent transcript.

### 3. Microphone Selection And Activity

- Open settings and choose a non-default microphone from the combo box.
- Speak into the selected microphone and verify the mic activity indicator updates.
- Save the selection and perform a transcription.
- Verify audio is captured from the selected device.
- Disconnect or disable the selected device, then trigger transcription again.
- Verify the app falls back safely and surfaces the issue in the tray/settings status.

### 4. Output Modes

- Set output mode to `Type directly` and verify transcripts are typed into the focused app.
- Set output mode to `Copy to clipboard` and verify transcripts are copied without typing.
- Set output mode to `Paste clipboard` and verify the clipboard content is pasted into the focused app.
- Enable `Append newline after transcript` and verify a trailing newline is included for all output modes.

### 5. Transcript History And Resend

- Dictate multiple transcripts and verify they appear in the tray `Recent transcripts` menu.
- Select a recent transcript from the tray and verify it is resent immediately.
- Use the resend hotkey and verify the last selected recent transcript is resent again.
- Lower the history limit in settings, save, and verify older items are trimmed.

### 6. Keep-Talking And Streaming Tray Status

- Start keep-talking mode and verify the tray tooltip/menu shows recording is active.
- Stop keep-talking mode and verify the tray returns to idle status.
- Start streaming mode and verify the tray tooltip/menu shows streaming is active.
- Stop streaming mode from the tray and verify status returns to idle.

### 7. Live Config Reload And Backup

- Start the app and verify `%USERPROFILE%\.config\deepgram\velocity.backup.yml` is created or updated.
- Edit `%USERPROFILE%\.config\deepgram\velocity.yml` while the app is running and change:
  - output mode
  - append-newline
  - hotkeys
  - history limit
- Verify valid changes apply without restarting.
- Introduce an invalid configuration value such as `history_limit: 0`.
- Verify the running app keeps the previous working configuration and surfaces an error in the tray/settings status.

### 8. Key Terms

- Add multiple key terms in settings, one per line, and save.
- Dictate phrases containing those terms in both push-to-talk and streaming mode.
- Verify the terms are better recognized and remain persisted after restarting the app.
