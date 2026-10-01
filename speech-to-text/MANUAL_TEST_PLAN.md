# Manual Test Plan: Usage Tags

These checks confirm that `--tag` is sent to Deepgram and appears in usage
reporting. They require a Deepgram API key and are not suitable for CI because
the usage reporting result must be checked in Deepgram Console.

## Pre-recorded transcription

Transcribe a short audio file with two tags, including one with a space:

```powershell
cargo run --package dg-stt -- transcribe --file .\sample.wav --tag "staging,speech team"
```

Confirm transcription succeeds. In Deepgram Console usage reporting, locate the
request and verify both tags are available for filtering/grouping.

## Streaming transcription

Run a short file through the streaming API with two tags:

```powershell
cargo run --package dg-stt -- stream file --file .\sample.wav --fast --tag "batch,speech team"
```

Confirm streaming completes and the request appears with both tags in usage
reporting. Repeat with `stream microphone` if microphone capture is available.

## Tag validation

- Confirm a 128-character tag is accepted.
- Confirm a 129-character tag is rejected before an API request is made.
- Confirm a tag containing spaces or URL-special characters arrives unchanged
  in usage reporting.

Deepgram documents a 128-character limit per tag and supports multiple `tag`
query parameters; see the [tagging guide](https://developers.deepgram.com/guides/fundamentals/tagging-your-usage-data).
