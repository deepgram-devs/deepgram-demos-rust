# Voice Agent TUI manual test plan

1. Set `DEEPGRAM_API_KEY`, run `cargo run -- tui`, and confirm the terminal is restored after `q` and after a connection error.
1a. Induce a WebSocket connection error (for example, with an invalid endpoint) and confirm the header changes to `Connection error`, the request ID clears, and the failure appears in the event view.
1b. Run `voice-agent tui --verbose`, induce a connection or server error, and verify it is appended to `voice-agent.log` in the current working directory.
2. Press `Ctrl+P`, type `prompt`, open the editor, exercise Home/End, arrows, Backspace/Delete, insert text, and press `Ctrl+S`. Confirm the prompt update is sent only after saving.
2a. In the prompt editor, exercise `Ctrl+Left` and `Ctrl+Right` and confirm the cursor moves one word at a time.
2b. Save multiple prompts, restart the TUI, use **Select historical system prompt**, and confirm the selected prompt is restored and used for a connection.
2c. Set `DEEPGRAM_PROJECT_ID`, select **Select reusable agent configuration**, choose a listed configuration, connect, and verify the Settings JSON uses the selected UUID as `agent`.
3. Open `Select TTS voice`; select Aura-2 voices and Flux, reconnect if necessary, and verify the selected values are shown in the status bar.
3a. Type a partial model or voice name in the TTS chooser and confirm the complete catalog filters without losing keyboard navigation.
4. Open `Select STT provider`; exercise Flux English, Flux Multilingual, and Nova-3. Confirm the model/version/language payload is appropriate for V2/V1.
4a. Confirm TUI `UpdateListen` sends `language` only for Nova V1 and omits it for Flux V2; verify Flux Multilingual preserves configured `language_hints`.
5. Connect with headphones, speak, and verify the UI shows both the API transcription (`You`) and agent response (`Agent`) while audio plays.
6. Exercise Inject Agent, Inject User, Force End Turn, and Keep Alive from the command palette. Confirm outgoing actions appear in the event pane and server errors are visible.
7. For Inject Agent, enter custom text and cycle all three behaviors with Tab; verify the selected behavior and message are sent. For Inject User, enter custom text and verify it is sent as `InjectUserMessage`.
8. Open Update Think, edit the model, submit it, and verify the selected model is sent with the current prompt.
9. Verify `cargo run -- tui --help` shows the TUI-specific launch options and `cargo run -- config create --help` still shows configuration commands.
10. Receive several user and agent messages, toggle message timestamps on and off from the command palette, and confirm all existing messages gain or lose their retained timestamps together.
10a. Click user and agent messages in the conversation pane and confirm the selected message is highlighted. Use **Copy conversation** and verify the complete conversation is available in the clipboard.
10d. Click user, agent, client, warning, and event messages and verify each individual message is copied to the clipboard.
10c. Connect, use **Copy request ID**, and verify only the active `Welcome.request_id` is copied. Disconnect and confirm the action reports that no active request ID is available.
10b. Run the TUI with `RUST_LOG=debug`, connect, and confirm HTTP-to-WebSocket upgrade diagnostics do not render over the TUI.
11. Enable verbose JSON logging before connecting, confirm the `Welcome.request_id` names a new file in the system temporary directory, and verify it contains the `Welcome`, `Settings`, control, and server JSON messages. In a separate session, exchange messages and controls first, enable logging mid-session, and confirm the file begins with all retained earlier JSON messages before continuing with new messages. Toggle logging during a session and confirm logging starts/stops without affecting the connection.
11a. Click the warning-colored JSON log location in the message view and verify the fully qualified log-file path is copied to the clipboard.
11b. Enable verbose JSON logging before connecting and confirm the log location is the first message in the newly cleared message view.
12. Change TTS and listen settings, exit and restart the TUI, and verify the last voice/model and listen provider/model/version/language/tuning settings are restored from `voice-agent.yml`; add an unrelated YAML key and confirm saving preserves it.
