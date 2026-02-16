# Changelog

All notable changes to the Rust Flux WebSocket Client will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
