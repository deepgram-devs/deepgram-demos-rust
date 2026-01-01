# Changelog for Deepgram Rust Speech-to-Text (STT) CLI

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
