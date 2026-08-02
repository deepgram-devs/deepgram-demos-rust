# Changelog

All notable changes to the Rust Flux WebSocket Client will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] - 2026-07-20

### Added

- `--numerals` flag to convert spoken numbers into digits (e.g. "nine hundred" -> "900"), set at connection time since Flux does not support toggling it mid-stream
- `--eager-eot-threshold` (alias `--eeot`, 0.3-0.9) to enable Flux's `EagerEndOfTurn`/`TurnResumed` events, validated client-side before connecting
- `--connection <N>` to select which connection's transcript is printed in the default output mode (default `0`, the first connection), validated against `--threads`
- `--stats` flag to show a live statistics table for all connections; the table is no longer shown by default
- Confidence-score suffixes (`[eager_eot_confidence: X.XXXX]` / `[eot_confidence: X.XXXX]`) on `EagerEndOfTurn`/`EndOfTurn` transcript lines
- `flux-turn-taking/AGENTS.md` documenting the project's product requirements and implementation notes for future changes

### Changed

- Message handling now conforms to Flux's actual schema: `type` is always `TurnInfo` for turn updates, with the real event (`StartOfTurn`, `Update`, `EagerEndOfTurn`, `TurnResumed`, `EndOfTurn`) in a nested `event` field, replacing the previous handling that assumed Nova-3 streaming's `Results`/`SpeechStarted`/`UtteranceEnd`/`Metadata` message types
- The statistics table's columns now report Flux's actual event types instead of Nova-3's
- Default output mode now prints a single connection's transcript instead of showing the statistics table; `--stats` opts back into the table
- Transcript lines are prefixed with the Flux event type that produced them and are redrawn in place (one line per turn) as new messages arrive, instead of printing a new line for every message or diffing individual words
- Color is applied only to the transcript text, never to the statistics table
- Pressing Ctrl+C in microphone mode now exits immediately instead of waiting up to 2 seconds for worker threads to finish

### Fixed

- Removed an extra blank line that appeared between consecutive turns in the transcript output

## [0.2.0] - 2026-02-16

### Added

- Multi-format audio file support (WAV, MP3, M4A, AAC) with automatic decoding using Symphonia
- Incremental word printing with color-coded speaker turn detection for real-time transcription display
- Real-time playback speed simulation for file streaming (100ms chunks based on detected sample rate)
- Deepgram Request ID display on connection for debugging and support
- Terminal color reset on application exit to prevent color artifacts in user's terminal

### Changed

- File mode parameter changed from `--file` to `--path` for clarity
- File streaming mode now shows incremental transcription by default instead of statistics table
- Added `--verbose` flag to show full JSON responses for debugging
- Sample rate is now auto-detected from audio files instead of using CLI parameter

## [0.1.0] - Initial Release

### Added

- Real-time microphone audio streaming to Deepgram Flux API
- Basic file streaming support with manual configuration
- Multi-threaded connection support for load testing
- Statistics table for monitoring throughput and message counts
- Comprehensive logging to file with configurable log levels
- WebSocket connection handling with proper cleanup and shutdown
