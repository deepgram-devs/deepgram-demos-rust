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
