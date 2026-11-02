# Voice Agent Manual Test Plan

These checks require a working Deepgram Voice Agent API key, microphone, and speakers or headphones.

## JSON file logging

1. In PowerShell, choose a fresh log path and start the standard CLI:

   ```powershell
   $logPath = Join-Path $PWD 'voice-agent-messages.log'
   Remove-Item $logPath -ErrorAction SilentlyContinue
   cargo run -p voice-agent -- --json-log $logPath
   ```

2. Speak a short prompt, wait for the agent response, then stop the program with `Ctrl+C`.
3. Inspect the log with `Get-Content $logPath`. Confirm each entry starts with a UTC date and 24-hour timestamp, followed by `CLIENT` or `SERVER`. Confirm entries include `Welcome` and `ConversationText` server messages plus the client `Settings` message.
4. Restart the CLI with the same path and confirm new messages are appended instead of replacing the earlier session.
5. Confirm the log contains JSON text messages only; microphone and agent audio continue working and do not appear as binary payloads in the file.

## TUI JSON file logging

1. Choose a fresh path and launch the TUI with file logging enabled:

   ```powershell
   $logPath = Join-Path $PWD 'voice-agent-tui-messages.log'
   Remove-Item $logPath -ErrorAction SilentlyContinue
   cargo run -p voice-agent -- tui --json-log $logPath
   ```

2. Connect from the command palette. Confirm the absolute log path appears as a warning-colored message and clicking it copies that path.
3. Speak to the agent and send a client control message from the command palette. Confirm both server and client JSON messages with UTC 24-hour timestamp prefixes are appended while audio still flows.
4. Disconnect and reconnect. Confirm the same selected log file continues to append messages for the new session.

## Standard CLI ForceEndTurn shortcut

1. Launch a standard, non-TUI session with Flux V2:

   ```powershell
   cargo run -p voice-agent -- --listen-provider deepgram --listen-model flux-general-en --listen-version v2
   ```

2. During an agent turn, press `f`. Confirm the CLI reports `Sent ForceEndTurn`, the agent turn ends, and the received end-of-turn transcript/trigger is shown at info level.
3. Repeat with the default Nova-3 configuration. Press `f` and confirm the CLI warns that Flux V2 is required and does not send the message.
4. Press `Ctrl+C` to stop. Confirm the terminal returns to normal input and echo behavior.
5. During the session, confirm successive runtime log messages and wrapped messages stay aligned at the left margin.

## Standard CLI Settings validation diagnostics

1. In a test environment, launch the standard CLI with a deliberately invalid listen or speak setting that reaches the Voice Agent server.
2. Confirm the server error is printed and the submitted Settings JSON follows it, with credential values redacted, syntax-colored keys and values, and every pretty-printed line starting at column zero.
3. In PowerShell, set `$env:NO_COLOR = '1'`, repeat the check, then remove it with `Remove-Item Env:NO_COLOR`. Confirm the JSON remains pretty-printed without ANSI color codes. Redirected stderr should also remain uncolored.

## Configuration YAML compatibility

1. Back up the Voice Agent configuration file at the platform-resolved Deepgram configuration path.
2. Add an unrelated nested setting with a string, boolean, number, list, and map value.
3. Launch `voice-agent tui`, change a persisted preference, then exit normally.
4. Confirm the preference was saved and the unrelated setting and its value types were preserved.
5. Restart the TUI and confirm the saved preference is restored.
