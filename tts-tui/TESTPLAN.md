# TTS TUI Test Plan

## Automated Test Plan

Run the package test suite:

```bash
cargo test -p tts-tui
```

Automated coverage currently verifies:

- Default experimental flags are disabled.
- Boolean environment variable parsing handles common true/false values.
- The generated default TOML configuration template parses successfully.
- SageMaker `CustomAttributes` are built with the Deepgram `v1/speak` path and encoded TTS query parameters.
- SageMaker fixed-rate encodings omit `sample_rate` when appropriate.
- Deepgram HTTP and SageMaker requests add `normalize_volume=true` only when enabled.
- Deepgram HTTP, WebSocket, and SageMaker requests include the default request tags and preserve additional tags.
- Deepgram HTTP requests and WebSocket handshakes include a `User-Agent` header formatted as `tts-tui/<version>`.

Run the package build check:

```bash
cargo check -p tts-tui
```

## Manual Test Plan

### 1. Startup And Configuration

- Launch the app with no config file and verify `~/.config/deepgram/deepgram-tts-client.toml` is created with documented `[api]`, `[sagemaker]`, `[audio]`, and `[experimental]` sections.
- Place a config at the legacy `~/.config/deepgram-tts-client.toml` path, launch the app, and verify it is moved to the new `~/.config/deepgram/` directory without losing settings.
- Verify the log panel shows the config path at startup.
- Verify `~/.config/deepgram/tts-tui.log` is created and contains startup and UI log entries. Set `[logging].max_size_bytes` to a small value, generate enough log output to rotate it, and verify the active file plus at most `max_files - 1` numbered backups are retained.
- Set `TTS_TUI_PROVIDER=deepgram` and verify the app logs the `deepgram` provider.
- Set `TTS_TUI_PROVIDER=sagemaker` without a SageMaker endpoint name and verify the app logs a warning that the endpoint name is missing.
- Verify CLI values override environment variables and TOML values for provider, endpoint, audio format, sample rate, SageMaker endpoint name, and AWS region.

### 2. Deepgram HTTP Provider

- Run with the default provider and a valid `DEEPGRAM_API_KEY`.
- Add a short text snippet and play it with the selected voice.
- Verify audio is generated, played, and cached.
- Press `Ctrl+Enter` and verify the cache is bypassed and a fresh request is made.
- Run with `--endpoint` pointing at a self-hosted or proxy Deepgram-compatible TTS endpoint and verify playback still works.
- Run with `--tags production --tags demo` (or `--tags production,demo`) and use endpoint logging to verify the default tags plus both custom tags are sent.
- Use endpoint logging or a test proxy to verify the outgoing `User-Agent` is `tts-tui/0.9.7` for the current release.
- Remove or invalidate the API key for an endpoint that requires one and verify the log panel shows a useful error.
- Verify `tts-tui --help` does not list a `--normalize-volume` option.
- Press `v` to enable volume normalization and verify the request query contains `normalize_volume=true`; press it again to disable normalization and verify the parameter is omitted.
- Press `v` and verify the status bar/log reports volume normalization enabled or disabled; verify the next request uses the new state.
- Press `w` and verify the status bar/log reports WebSocket streaming enabled or disabled. With a hosted Aura voice, a valid `DEEPGRAM_API_KEY`, and streaming enabled, play a saved text and verify that audio begins before the full utterance has been generated.
- Press `c` and verify the status bar cycles through 10 words, sentence boundary, and punctuation chunking. With endpoint logging enabled, verify the app sends one `Speak` WebSocket message per chunk and a final `Flush` message.
- Verify `~/.config/deepgram/tts-tui.log` records the individual WebSocket `Speak` chunks after they are sent.
- Verify a hosted Deepgram batch request logs its `dg-request-id` response header, and a WebSocket stream logs the request ID received in its `Metadata` (Aura) or `Connected` (Flux) message.
- Verify the app waits for Deepgram's `Flushed` response before closing the WebSocket and plays all queued audio. Press `Esc` during another stream and verify playback stops and the connection closes without waiting for the final audio.
- With streaming enabled, verify a Flux voice uses WebSocket streaming and plays the complete utterance; verify the SageMaker provider still shows a clear unsupported-mode error and its normal request path works when streaming is disabled.
- With streaming enabled and a self-hosted Deepgram-compatible HTTP(S) endpoint configured, omit the API key and verify the app connects without an authentication header.
- Play several different texts/voices back-to-back in quick succession (bypassing the cache with `Ctrl+Enter` for at least one) and verify no audible pop, click, or static plays between or during tracks.
- Start the app on a machine with no audio output device (or with audio hardware disabled) and verify the log panel shows "No audio output device available" at startup and that text/voice management still works without crashing.

### 2a. Flux TTS Voices

- Open the Voices filter (`/`) and type `flux`; verify all 36 `flux-*-en` voices are shown, including the featured voices and the additional accent/character voices listed in the README.
- Play a short phrase with a Flux voice and verify audio is generated and plays correctly.
- Increase or decrease playback speed, then play a Flux voice and verify the log notes that Flux ignores playback speed.
- Enable volume normalization, then play a Flux voice and verify the log notes that Flux ignores volume normalization.
- With a proxy tool (e.g. `mitmproxy`) or endpoint logs, confirm the outgoing request path is `/v2/speak` for a Flux voice and `/v1/speak` for an Aura or Aura-2 voice in the same session.
- For Flux streaming, confirm the WebSocket uses `Authorization: Token ...`, sends `Speak` followed by `Flush`, receives `Flushed` and `SpeechMetadata`, then sends `Close` and receives `SessionMetadata` before the connection closes.
- Play an Aura or Aura-2 voice immediately after a Flux voice and verify both work without needing to restart the app or change configuration.

### 3. SageMaker Provider

- Configure AWS credentials with `sagemaker:InvokeEndpoint` permission.
- Run with `--provider sagemaker --sagemaker-endpoint-name <endpoint> --aws-region <region>`.
- Add a short text snippet and play it with an Aura-2 voice.
- Verify the request reaches the SageMaker endpoint and audio plays locally.
- Verify the generated cache entry is distinct from the same text and voice generated through the `deepgram` provider.
- Verify normalized and unnormalized requests produce distinct cache entries.
- Run with an invalid endpoint name and verify the log panel surfaces a SageMaker invocation error with endpoint and region context.

### 4. Audio Formats And Sample Rates

- Open the audio format popup with `f`.
- Select MP3, Linear16, FLAC, AAC, μ-law, and A-law one at a time.
- For each format, open the sample-rate popup with `s` and verify only valid sample rates are shown.
- Generate and play a short phrase for each supported format.
- Verify the status bar shows the active format and sample rate.
- Verify μ-law and A-law audio play correctly instead of failing through the generic decoder.

### 5. Saved Texts And Voice Selection

- Add, edit, delete, and reorder saved text snippets.
- Restart the app and verify saved texts persist.
- Filter saved texts with `/` while the Saved Texts panel is focused.
- Filter voices with `/` while the Voices panel is focused.
- Toggle a favorite voice with `*`, restart the app, and verify the favorite marker persists.
- Click items in both lists with the mouse and verify selection and focus update correctly.

### 6. Playback Queue And Controls

- Press `Space` on several text and voice combinations and verify the queue count appears in the status bar.
- Start playback and verify queued items advance automatically.
- Stop playback with `Esc` and verify the app returns to an idle state.
- Use the command palette to clear the playback queue.
- Verify the progress bar appears for generated audio with a known duration.

### 7. Help, Logs, And Command Palette

- Press `?` and verify the help popup lists current keyboard and mouse controls.
- Scroll the help popup with arrow keys on a small terminal.
- Press `Ctrl+P`, search for several commands, and invoke them.
- Scroll the log panel with the mouse wheel and verify older entries are reachable.
- Verify errors are color-coded and include enough detail to diagnose API, AWS, or playback failures.

### 8. Terminal Recovery

- Quit with `q` from the Saved Texts panel and verify the terminal returns to normal mode.
- Quit with `Ctrl+Q` from the Voices panel and verify the terminal returns to normal mode.
- Start playback, quit during or after playback, and verify the terminal still exits alternate screen mode and shows the cursor.
