# Changelog

## 0.3.0 - 2026-04-16

- Added a modern WinUI 3 settings sidecar with Rust fallback UI.
- Added configurable Deepgram keyterms, hotkeys, output modes, transcript history, and audio input selection.
- Added live microphone activity monitoring in the settings UI with RMS-to-decibel metering.
- Added live config reload with backup/rollback behavior and consolidated app data under `%USERPROFILE%\\.config\\deepgram`.
- Added richer tray controls and status for push-to-talk, keep-talking, and streaming modes.
