# Changelog

## 0.3.5 - 2026-04-18

- Added model and language drop-down selectors to both settings UIs with support for Deepgram Nova-2 and Nova-3.
- Restricted language selection to the languages supported by the currently selected model, with an explicit "Do not specify" option.
- Added propagation of the selected language to Deepgram HTTP and WebSocket requests, omitting the `language` query parameter when unspecified.
- Added automatic deployment of the WinUI settings sidecar into the Rust app output directory so the updated settings UI is launched reliably.

## 0.3.0 - 2026-04-16

- Added a modern WinUI 3 settings sidecar with Rust fallback UI.
- Added configurable Deepgram keyterms, hotkeys, output modes, transcript history, and audio input selection.
- Added live microphone activity monitoring in the settings UI with RMS-to-decibel metering.
- Added live config reload with backup/rollback behavior and consolidated app data under `%USERPROFILE%\\.config\\deepgram`.
- Added richer tray controls and status for push-to-talk, keep-talking, and streaming modes.
