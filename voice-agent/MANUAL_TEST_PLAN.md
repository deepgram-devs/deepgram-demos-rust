# Voice Agent Manual Test Plan

These checks require a working Deepgram Voice Agent API key, microphone, and speakers or headphones.

## Think Live

1. Start the standard CLI with Think Live enabled and a JSON log:

   ```powershell
   $logPath = Join-Path $PWD 'think-live-messages.log'
   cargo run -p voice-agent -- --think-live --think-live-backchannel-sensitivity 0.2 --think-live-presence-check-max-count 2 --think-live-presence-check-interval-ms 5000 --think-live-frustration-max-count 2 --think-live-restricted-term human --think-live-restricted-term transfer --json-log $logPath --verbose
   ```

2. Confirm the Settings JSON contains `agent.think_live.provider` with `type: deepgram` and `version: v1`, plus the selected parameters. Confirm the existing `agent.think.provider` remains the reasoning LLM.
3. Speak for an extended turn and pause. Confirm Think Live backchannels/presence checks play and appear in `ConversationText` with their `conversational_behavior` labels. If frustration is detected, confirm the CLI reports its count and the `escalated` state.
4. From the TUI, connect with `cargo run -p voice-agent -- tui --think-live --think-live-backchannel-sensitivity 0.2`. Use **Adjust Think Live backchannel sensitivity**, enter a different value, and confirm `UpdateThink_Live` contains the changed sensitivity plus the other configured fields in the JSON log. Confirm the UI reports the `ThinkLiveUpdated` acknowledgement.
5. Use **Request dynamic agent filler** and confirm the client sends `InjectAgentMessage` with `is_dynamic: true` and no `message` field. Confirm the resulting filler is shown with its behavior label.
6. Enable sample functions with `--enable-sample-functions --function-initial-agent-filler --function-agent-filler-max-count 2 --function-agent-filler-interval-ms 7000`. Confirm the function definitions contain the configured filler fields and waits are filled while the function call is pending.
7. Confirm Think Live settings without `--think-live` fail with a clear validation error, and sample-function filler settings require both `--think-live` and `--enable-sample-functions`.

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

## AEC3 echo cancellation

These checks need a working Deepgram API key, microphone, and speakers or headphones.

1. Run the standard CLI with AEC3 enabled:

   ```powershell
   cargo run -p voice-agent -- --aec-delay-ms 116
   ```

2. Speak while the agent is speaking. Confirm near-end speech is still captured, agent playback is not fed back as user speech, and the session remains responsive.
3. Interrupt agent speech by speaking. Confirm Deepgram's `UserStartedSpeaking` event clears already queued agent audio immediately in both CLI and TUI sessions.
4. Repeat without `--aec-delay-ms` to check adaptive delay, then set `$env:AEC_DELAY_MS = '116'` and confirm the environment default works. Pass `--aec-delay-ms 0` and confirm the CLI value overrides the environment value.
5. Confirm values above 500 are rejected. Confirm `--audio-encoding mulaw` is rejected by an AEC build.
6. Repeat with the TUI using `cargo run -p voice-agent -- tui`. Speak during agent playback and confirm the microphone remains live while echo is suppressed. Confirm `AEC speaker reference active` is not printed over the TUI.
7. Run with `--no-mic-mute=false`. Confirm the microphone is muted during playback and resumes after 600 ms of silence.
8. Use `--save-audio <PATH>` in a standard AEC session and confirm the raw file contains the processed 24 kHz linear16 stream, not the unprocessed microphone capture.
9. Test with the normal device rates and, when supported, an explicit `--audio-sample-rate`. Confirm playback underruns and a new agent-speaking turn do not leak stale render audio into the microphone stream.
10. During a live response, confirm a burst of incoming TTS chunks does not produce `AEC speaker queue is full; audio chunk dropped` or cut out audio. If playback errors persist because the output device is unavailable, confirm the `Failed to play audio` diagnostic appears no more than once per second.
11. End an active standard CLI conversation with `Ctrl+C` while agent audio or microphone packets are in flight. Confirm shutdown completes without an `AEC worker failed closed` error or `JoinHandle polled after completion` panic. Repeat with a remote connection close.
