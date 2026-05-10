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
- Transcript history insertion, deduplication, and limit trimming.
- Output formatting for append-newline behavior.
- Windows startup naming and shortcut argument helpers.
- Deepgram pre-recorded URL construction with smart formatting and key terms.
- Deepgram streaming URL construction with smart formatting and key terms.
- Audio peak meter scaling helpers.

## Manual Test Plan

### 1. Settings UI

- Launch `velocity.exe`.
- Open the tray menu and choose `Settings`.
- Verify the settings window opens and can be focused again from the tray.
- Verify launching `velocity.exe` a second time does not start a second process.
- Verify the GPUI window shows fields for API key, model, language, smart formatting, key terms, hotkeys, audio device, history limit, output mode, append-newline, focused-app delivery, and Windows sign-in startup.
- Verify only one settings window can exist at a time.
- Edit a setting and verify the unsaved-changes banner appears, then save and verify it clears.
- Enter an invalid hotkey and an out-of-range history limit and verify each field shows immediate visual validation before saving.

### 2. Hotkey Configuration

- In settings, change each hotkey to a non-conflicting binding and save.
- Verify the new hotkeys work immediately without restarting.
- Enter an invalid hotkey string and verify save is rejected with a visible error.
- Restore defaults and verify:
  - `Win+Ctrl+'` starts push-to-talk.
  - `Win+Ctrl+Shift+'` toggles keep-talking.
  - `Win+Ctrl+[` toggles streaming.

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
- Disable `Send transcript to the app focused at delivery`, start a recording in one app, change focus before the transcript completes, and verify output returns to the original app.
- Enable `Send transcript to the app focused at delivery`, repeat the focus switch, and verify output goes to the currently focused app.

### 5. Transcript History

- Dictate multiple transcripts and verify they are saved in `%USERPROFILE%\.config\deepgram\velocity-history.yml`.
- Lower the history limit in settings, save, and verify older saved entries are trimmed.

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

- Add multiple key terms in settings as a comma-separated list and save.
- Dictate phrases containing those terms in both push-to-talk and streaming mode.
- Verify the terms are better recognized and remain persisted after restarting the app.

### 9. Windows Sign-In Startup

- Enable `Launch Velocity when I sign in to Windows`.
- Verify `%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup\Deepgram Velocity.lnk` is created with `velocity.exe` as the target and `--start-minimized` as the argument.
- Verify the shortcut uses the Deepgram icon in Windows Startup Apps.
- Disable the setting and verify the startup shortcut is removed.
- Launch `velocity.exe --start-minimized` with no API key configured and verify the app remains tray-first while surfacing the missing-key status.
