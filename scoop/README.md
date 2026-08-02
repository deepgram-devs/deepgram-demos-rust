# Deepgram Rust Samples Scoop Bucket

This repository can be used directly as a custom [Scoop](https://scoop.sh/) bucket on Windows.

## Install

```powershell
scoop bucket add dg https://github.com/deepgram-devs/deepgram-demos-rust
scoop install dg/dg-stt
```

After an application's first release has completed, replace `dg-stt` with any available application:

| Application | Scoop command |
| --- | --- |
| Audio Recorder | `scoop install dg/audio-recorder` |
| Flux Turn-Taking | `scoop install dg/dg-flux` |
| Speech-to-Text | `scoop install dg/dg-stt` |
| Text-to-Speech | `scoop install dg/dg-tts` |
| Podcaster | `scoop install dg/dgpodcaster` |
| TTS TUI | `scoop install dg/tts-tui` |
| Voice Agent | `scoop install dg/voice-agent` |
| Velocity | `scoop install dg/velocity` |

Each Scoop manifest installs a SHA256-verified Windows ZIP from the matching GitHub release. Rust, Cargo, and the Microsoft C++ Build Tools are not required on users' machines. Both x64 and ARM64 Windows assets are published for every release.

Each app still needs its own runtime configuration, such as a `DEEPGRAM_API_KEY`, after installation. Run `<app> --help` for its CLI options.

## Verify a manifest before publishing

From a Windows PowerShell prompt with Scoop installed:

```powershell
scoop install .\scoop\bucket\dg-stt.json
dg-stt --help
scoop uninstall dg-stt
```

Use [Test-ScoopManifests.ps1](Test-ScoopManifests.ps1) to validate all JSON files and verify that their release assets match their pinned hashes.

## Release and manifest publishing

Every application has a tag-triggered release workflow. A tag named `<app>-v<version>` builds six native artifacts: macOS Intel and ARM, Linux Intel and ARM, and Windows x64 and ARM64. After publishing the GitHub release, the workflow writes the matching Scoop manifest to `scoop/bucket/` with the hashes of the two Windows ZIPs.

For example, publishing `dg-stt-v0.3.0` creates `dg-stt-0.3.0-x86_64-pc-windows-msvc.zip` and `dg-stt-0.3.0-aarch64-pc-windows-msvc.zip`, then updates `scoop/bucket/dg-stt.json`. The package becomes available through `scoop install dg/dg-stt` once that manifest update reaches the bucket branch.

The workflow-dispatch inputs can backfill an existing annotated tag, so the first precompiled release for each application does not require a source change.
