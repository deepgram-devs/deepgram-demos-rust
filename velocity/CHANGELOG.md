# Changelog

## 0.4.0 - 2026-04-25

- Replaced the Rust settings prototype with the current GPUI settings and onboarding window.
- Added a compact dark settings layout with Deepgram heading gradients, section hover feedback, app icon integration, and a visible unsaved-changes banner.
- Added immediate visual validation for invalid hotkeys and transcript history limits.
- Added richer runtime status feedback in the settings window, including mode tiles, visible failure states, and active microphone display.
- Kept settings persistence on the tray thread so hotkey updates roll back safely when registration fails.
- Removed the legacy WinUI/.NET settings sidecar from the release path; Velocity now ships as a single Windows executable.

## 0.3.5 - 2026-04-21

- Replaced the legacy settings implementations with a single Rust `iced` settings and onboarding UI.
- Enforced single-instance behavior for both the Velocity process and the settings window.
- Added model and language drop-down selectors to the Rust settings UI with support for Deepgram Nova-2 and Nova-3.
- Restricted language selection to the languages supported by the currently selected model, with an explicit "Do not specify" option.
- Added propagation of the selected language to Deepgram HTTP and WebSocket requests, omitting the `language` query parameter when unspecified.
- Added configurable Deepgram keyterms, hotkeys, output modes, transcript history, audio input selection, live microphone activity monitoring, and config reload behavior in the Rust settings UI.
