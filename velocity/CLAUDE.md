# Velocity Implementation Notes

## Product Direction

Velocity is a Windows 11 speech-to-text tray application written entirely in Rust.

There is no C# or .NET settings sidecar. The API key onboarding flow and the full settings window are implemented in Rust with `iced` and ship as part of `velocity.exe`.

## Current UI Architecture

- The settings UI lives in [src/settings.rs](C:/git/deepgram-demos-rust/velocity/src/settings.rs).
- The settings and onboarding windows share the same `SettingsWindow` state model.
- The tray window and Win32 message loop live in [src/tray.rs](C:/git/deepgram-demos-rust/velocity/src/tray.rs).
- Global application state and cross-thread coordination live in [src/state.rs](C:/git/deepgram-demos-rust/velocity/src/state.rs).

## Windowing Constraint

The `iced` window currently runs on a spawned thread, not on the main tray thread.

Because `winit 0.30` rejects off-main-thread event loops by default on Windows, the repo vendors `iced_winit` under `vendor/iced_winit/` and patches it via the workspace root `Cargo.toml` to enable `EventLoopBuilderExtWindows::with_any_thread(true)`.

If the vendored patch is removed or bypassed, opening the settings window will panic.

## Single-Instance Requirements

- Only one `velocity.exe` process may run at a time.
- Only one settings or onboarding window may exist at a time.
- Repeated `Settings` actions must focus the existing window instead of opening a new one.

The process-level mutex lives in [src/single_instance.rs](C:/git/deepgram-demos-rust/velocity/src/single_instance.rs). The settings-window singleton is enforced in [src/settings.rs](C:/git/deepgram-demos-rust/velocity/src/settings.rs).

## Save And Hotkey Rules

Hotkey registration is thread-affine because the hotkey manager owns the tray window handle.

That means:

- the settings UI must not call `HotkeyManager::apply_config` directly
- the settings UI must request saves through `AppState::request_config_save`
- the tray thread must process the save through `WM_APP_SAVE_CONFIG`
- hotkey rollback on failure must happen on the tray thread

The relevant flow is:

- [src/settings.rs](C:/git/deepgram-demos-rust/velocity/src/settings.rs) builds and validates the candidate config
- [src/state.rs](C:/git/deepgram-demos-rust/velocity/src/state.rs) queues the pending save and blocks for the result
- [src/tray.rs](C:/git/deepgram-demos-rust/velocity/src/tray.rs) handles `WM_APP_SAVE_CONFIG`
- [src/state.rs](C:/git/deepgram-demos-rust/velocity/src/state.rs) applies hotkeys, saves the config file, updates runtime state, and reports the result back

Do not move hotkey registration back into the `iced` settings thread. That causes `RegisterHotKey` failures such as "Invalid window; it belongs to other thread."

## Functional Requirements

The Rust settings UI must retain all current configuration functionality:

- API key entry and save
- model selection
- language selection constrained by the selected model
- smart formatting toggle
- key terms editing
- all hotkey fields
- microphone input selection
- live microphone activity meter
- transcript history limit
- output mode selection
- append-newline toggle
- reload from disk
- config-changed-on-disk warning
- validation and status/error text

## Keyboard UX

- `Ctrl+S` in the settings UI should trigger the same save path as the Save button.
- The Save button and `Ctrl+S` must use identical validation and persistence behavior.

## Configuration Rules

- Continue using `%USERPROFILE%\.config\deepgram\velocity.yml`.
- Continue saving `%USERPROFILE%\.config\deepgram\velocity.backup.yml`.
- Continue using `%USERPROFILE%\.config\deepgram\velocity-history.yml`.
- Preserve compatibility with the current config schema.
- Reject invalid values without corrupting the last known good runtime state.

## Error Handling Expectations

- Invalid hotkeys must not partially replace the active hotkey set.
- Failed config saves must leave the previous runtime config intact.
- Save failures must surface in the settings window and tray status.
- The settings UI may block waiting for the tray thread to finish a save request.

## Non-Goals

- Do not reintroduce `Velocity.Settings/`.
- Do not add .NET, WinUI, Windows App SDK, or sidecar-specific build/runtime requirements.
- Do not add a second executable for settings or onboarding.

## Known Presentation Differences

Functional parity is the goal. Exact WinUI visuals are not.

Expected differences include:

- widget styling
- layout metrics
- typography
- window chrome behavior
