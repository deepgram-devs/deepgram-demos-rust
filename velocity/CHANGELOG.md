# Changelog

## 0.5.0 - 2026-05-10

- Added a Windows sign-in startup toggle to the built-in GPUI settings window, backed by a branded Startup-folder shortcut with Deepgram icon metadata.
- Added cleanup for legacy `Deepgram Velocity` and `Velocity` Run registry startup values.
- Added `--start-minimized` startup launch handling so automatic Windows sign-in launches stay in the tray instead of prompting on screen.
- Added startup-entry repair so existing Windows startup configuration follows the current `velocity.exe` path after upgrades.
- Added a focused-app delivery setting so transcripts can either go to the app focused at delivery time or return to the app that was focused when recording started.
- Added embedded Windows executable resources, version metadata, and the Deepgram `.ico` asset for the application executable and Scoop shortcut.
- Added Scoop deployment support with a manifest, local install instructions, and community bucket publishing notes.
- Removed the recent-transcript tray menu and resend-selected hotkey from configuration, hotkey registration, state, and tests.
- Updated README and manual test coverage to match the retained local transcript history behavior.

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
