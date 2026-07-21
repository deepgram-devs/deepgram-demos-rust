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

Run the package build check:

```bash
cargo check -p tts-tui
```

## Manual Test Plan

### 1. Startup And Configuration

- Launch the app with no config file and verify `~/.config/deepgram-tts-client.toml` is created with documented `[api]`, `[sagemaker]`, `[audio]`, and `[experimental]` sections.
- Verify the log panel shows the config path at startup.
- Set `TTS_TUI_PROVIDER=deepgram` and verify the app logs the `deepgram` provider.
- Set `TTS_TUI_PROVIDER=sagemaker` without a SageMaker endpoint name and verify the app logs a warning that the endpoint name is missing.
- Verify CLI values override environment variables and TOML values for provider, endpoint, audio format, sample rate, volume normalization, SageMaker endpoint name, and AWS region.

### 2. Deepgram HTTP Provider

- Run with the default provider and a valid `DEEPGRAM_API_KEY`.
- Add a short text snippet and play it with the selected voice.
- Verify audio is generated, played, and cached.
- Press `Ctrl+Enter` and verify the cache is bypassed and a fresh request is made.
- Run with `--endpoint` pointing at a self-hosted or proxy Deepgram-compatible TTS endpoint and verify playback still works.
- Remove or invalidate the API key for an endpoint that requires one and verify the log panel shows a useful error.
- Enable volume normalization and verify the request query contains `normalize_volume=true`; disable it and verify the parameter is omitted.
- Press `v` and verify the status bar/log reports volume normalization enabled or disabled; verify the next request uses the new state.
- Play several different texts/voices back-to-back in quick succession (bypassing the cache with `Ctrl+Enter` for at least one) and verify no audible pop, click, or static plays between or during tracks.
- Start the app on a machine with no audio output device (or with audio hardware disabled) and verify the log panel shows "No audio output device available" at startup and that text/voice management still works without crashing.

### 2a. Flux TTS Voices

- Open the Voices filter (`/`) and type `flux`; verify all 12 `flux-*-en` voices are shown.
- Play a short phrase with a Flux voice and verify audio is generated and plays correctly.
- Increase or decrease playback speed, then play a Flux voice and verify the log notes that Flux ignores playback speed.
- Enable volume normalization, then play a Flux voice and verify the log notes that Flux ignores volume normalization.
- With a proxy tool (e.g. `mitmproxy`) or endpoint logs, confirm the outgoing request path is `/v2/speak` for a Flux voice and `/v1/speak` for an Aura or Aura-2 voice in the same session.
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
