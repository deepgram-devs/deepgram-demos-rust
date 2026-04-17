# Velocity.Settings

WinUI 3 sidecar application for modern Velocity configuration and onboarding.

## Purpose

This project is the replacement path for the legacy Win32 API key and settings windows in the Rust app.

- `velocity.exe` remains the background engine.
- `Velocity.Settings.exe` becomes the modern Windows 11 UI surface.
- The Rust process launches this executable when it is present next to `velocity.exe`.
- If the sidecar is missing, Velocity falls back to the existing Win32 dialogs.

## Current Scope

This scaffold includes:

- WinUI 3 desktop project metadata
- modern settings layout
- launch-mode handling for `--page settings` and `--page api-key`
- config file load/save against `%USERPROFILE%\.config\deepgram\velocity.yml`
- backup write to `%USERPROFILE%\.config\deepgram\velocity.backup.yml`

## Next Steps

Recommended follow-up work:

1. Install a current .NET SDK and Windows App SDK workload.
2. Build and run the WinUI project.
3. Replace placeholder audio device and microphone meter data with IPC from the Rust background process.
4. Add hotkey validation and conflict checks via IPC.
5. Close the sidecar automatically after API key save when launched in onboarding mode.

## Build Requirements

- .NET 10 SDK
- Windows App SDK / WinUI 3 tooling
- Windows 11 development environment
