# Changelog for Deepgram Rust Speech-to-Text (STT) CLI

## 2026-04-06

Version: `0.3.0`

### Stream Mode

* Added `--diarize` flag to identify individual speakers; output groups consecutive words by speaker
* Added `--detect-entities` flag to enable named entity detection
* Added `--vad-events` flag to enable voice activity detection events
* Added `--sentiment`, `--intents`, `--topics` flags for audio intelligence in stream mode
* Added `--endpointing` (file mode) to control endpointing sensitivity in milliseconds
* Added `--utterance-end` (file mode) for utterance end timeout; requires `--interim-results` and `--vad-events`
* Added `--keyterm` for keyword boosting on nova-3+ models (comma-separated; conflicts with `--keywords`)
* Added `--keywords` for legacy keyword boosting on nova-2 and older (comma-separated with optional intensifier, e.g. `word:2.0`)
* Simplified boolean flags: `--interim-results`, `--punctuate`, `--smart-format` are now plain flags (no `true`/`false` value needed)
* Progress bar displayed during file streaming showing elapsed and total time
* Rich file metadata printed before streaming: codec, format, bitrate, sample rate, channels, bit depth, duration
* Extended audio format support: AAC, M4A/MP4, OGG, Vorbis, MKV, ALAC (in addition to MP3, WAV, FLAC)
* Deepgram request ID is now printed on successful connect and on connection error
* WebSocket connection errors now display the HTTP status code and error body
* Parse errors are written to `dg-stt-debug.log` for debugging
* Sends a `CloseStream` message when file streaming completes; keepalive/sender tasks are properly aborted
* ANSI line-clear (`\r\x1b[2K`) used to keep transcript output clean alongside the progress bar

### Transcribe Mode

* Added `--keyterm` and `--keywords` flags (same semantics as stream mode)

## 2025-12-31

* Moved the `file` and `microphone` subcommands underneath a new `stream` subcommand
  * Added the `--multichannel` parameter to both of these subcommands
* Added a "transcribe" root-level subcommand for HTTP pre-recorded transcription
  * This supports the `--multichannel` parameter, to get separate transcripts per-channel
  * **NOTE**: Originally, this git repository was intended to hold Deepgram streaming-only examples, but expanded to include other Rust examples, including single request-response APIs.

## 2025-11-28

* Exit immediately after the final transcription message is received + timeout (wait) period

## 2025-11-16

* Added CLI parameter for `--language`

## 2025-10-20

Version: `0.1.3`

* Added CLI parameters for Deepgram transcription:
  * Encoding
  * Sample Rate
  * Channels
  * Interim results
  * Punctuation
  * Smart formatting
  * Model
