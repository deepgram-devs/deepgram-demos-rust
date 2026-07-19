# Velocity Implementation Notes

## Releases

- Publish Windows releases through `.github/workflows/velocity-release.yml`.
- Do not rely on local Windows cross-compilation for release artifacts; the workflow builds the MSVC binary on a native `windows-2025` GitHub-hosted runner for Windows 11 users.
- When updating Velocity, keep `Cargo.toml`, `Cargo.lock`, `README.md`, `CHANGELOG.md`, and `TESTPLAN.md` current as appropriate.
- After merging an update, push an annotated tag named `velocity-v<version>` matching `Cargo.toml`. The workflow builds `velocity.exe`, packages it with the required documentation, generates checksums, and creates or updates the GitHub release.
- Verify both the Windows build job and release job succeed before considering a release complete.

## Product Direction

Velocity is a Windows 11 speech-to-text tray application written entirely in Rust.

There is no C# or .NET settings sidecar. The API key onboarding flow and the full settings window are implemented in Rust with GPUI and ship as part of `velocity.exe`.

## Current UI Architecture

- The settings UI lives in [src/settings.rs](C:/git/deepgram-demos-rust/velocity/src/settings.rs).
- The settings and onboarding windows share the same GPUI `SettingsView` state model.
- The tray window and Win32 message loop live in [src/tray.rs](C:/git/deepgram-demos-rust/velocity/src/tray.rs).
- Global application state and cross-thread coordination live in [src/state.rs](C:/git/deepgram-demos-rust/velocity/src/state.rs).
- Windows sign-in startup integration lives in [src/startup.rs](C:/git/deepgram-demos-rust/velocity/src/startup.rs).

## Windowing Constraint

The GPUI settings window currently runs on a spawned thread, not on the main tray thread.

The settings entry points live in [src/settings.rs](C:/git/deepgram-demos-rust/velocity/src/settings.rs). Keep the GPUI application/window lifecycle contained there unless the tray architecture is changed deliberately.

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

Do not move hotkey registration back into the GPUI settings thread. That causes `RegisterHotKey` failures such as "Invalid window; it belongs to other thread."

## Functional Requirements

The Rust settings UI must retain all current configuration functionality:

- API key entry and save
- model selection
- language selection constrained by the selected model
- smart formatting toggle
- key terms editing
- hotkey fields for push-to-talk, keep talking, and streaming
- microphone input selection
- live microphone activity meter
- transcript history limit
- output mode selection
- append-newline toggle
- focused-app delivery toggle
- Windows sign-in launch toggle
- reload from disk
- config-changed-on-disk warning
- validation and status/error text

Failures must be visible in the settings window, not only in logs. Plain text fields should validate as close to real time as GPUI allows; current examples are the hotkey fields and transcript History limit field.

When the focused-app delivery toggle is enabled, Velocity must deliver completed transcripts to the application focused at the end of the transcription connection. When it is disabled, Velocity must deliver completed transcripts to the application that was focused when recording started.

## Windows Sign-In Startup

The `Launch Velocity when I sign in to Windows` setting is not part of `velocity.yml`.

It is an immediate Windows setting backed by a per-user Startup folder shortcut:

- shortcut path: `%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup\Deepgram Velocity.lnk`
- shortcut target: current `velocity.exe`
- shortcut arguments: `--start-minimized`
- shortcut icon: exported Deepgram icon at `%APPDATA%\Deepgram\Velocity\deepgram-velocity-<version>.ico`, icon index `0`
- shortcut AppUserModelID: `Deepgram.Velocity`

The older Run registry values named `Deepgram Velocity` and `Velocity` are legacy. Enabling or disabling startup should remove them, along with stale `StartupApproved\Run` bookkeeping values for those names and stale `StartupApproved\StartupFolder` bookkeeping for `Deepgram Velocity.lnk`, so Windows Startup Apps uses the branded shortcut entry with the correct icon.

Changing this switch must update the Startup shortcut immediately and must not set the settings UI unsaved-changes banner. The shortcut plus Windows `StartupApproved` enabled/disabled state is the source of truth. Enabling startup must delete and recreate the shortcut rather than overwriting it in place so Windows does not retain stale shortcut icon metadata.

If enabling or disabling startup fails, the settings UI must roll the switch back to its previous value and show visible failure text in the Status section.

On normal startup, if the shortcut or legacy Run value exists and is not disabled in `StartupApproved`, Velocity may repair the shortcut to the current executable path. This supports ZIP extraction and personally compiled binaries that move between folders.

`--start-minimized` means the app should start tray-first. If the API key is missing during a startup launch, keep the app in the tray and surface the missing-key error through runtime status instead of opening onboarding automatically.

Velocity should call `SetCurrentProcessExplicitAppUserModelID` with `Deepgram.Velocity` early during process startup so Windows 11 associates windows, notifications, shell entries, and startup registrations with the same formal app identity.

## Keyboard UX

- `Ctrl+S` in the settings UI should trigger the same save path as the Save button.
- The Save button and `Ctrl+S` must use identical validation and persistence behavior.

## Settings UI Color Rules

- The `Velocity Settings` heading uses a per-character text gradient:
  - left: RGB `#12B8D8`
  - middle: RGB `#20D6B7`
  - right: HSV `155 92 85`, RGB `#11D986`
- Reuse the same HSV `155 92 85` / RGB `#11D986` green for non-gradient success accents, including Status success text and the unsaved-changes banner background.
- Immediate validation borders use a 2px left-to-right linear gradient overlay clipped to the text box radius:
  - left: HSV `324 99 92`, RGB `#EB028E`
  - right: HSV `278 84 99`, RGB `#AF28FC`
- When the validation gradient is active on a text field, set the text field's own border to the window background and disable its focus border so only the gradient border is visible.
- Use the validation gradient frame for invalid History limit and hotkey inputs. Do not show inline unsupported-hotkey helper text under hotkey fields.

## Configuration Rules

- Continue using `%USERPROFILE%\.config\deepgram\velocity.yml`.
- Continue saving `%USERPROFILE%\.config\deepgram\velocity.backup.yml`.
- Continue using `%USERPROFILE%\.config\deepgram\velocity-history.yml`.
- Do not store Windows sign-in startup state in the YAML config; read and write the Startup shortcut instead.
- Preserve compatibility with the current config schema.
- Reject invalid values without corrupting the last known good runtime state.
- The transcript History limit setting must validate as a number from `1` through `100`.

## Dependency License Requirements

Velocity must only use dependencies with licenses that are compatible with open source distribution.

- Do not add dependencies with proprietary, source-available-only, non-commercial, no-redistribution, or otherwise open-source-incompatible licenses.
- Before adding a new dependency, verify its license metadata and prefer widely used OSI-approved licenses such as MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, MPL-2.0, or Unicode-compatible license terms.
- If a dependency has multiple licenses, ensure at least one usable license path is compatible with Velocity's open source distribution.
- If license compatibility is unclear, do not add the dependency until the ambiguity is resolved and documented.
- Keep transitive dependency license risk in mind for release work; avoid dependency changes that introduce unresolved license obligations.

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
