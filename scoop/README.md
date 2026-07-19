# Deepgram Rust Samples Scoop Bucket

This repository can be used directly as a custom [Scoop](https://scoop.sh/) bucket on Windows.

## Install

```powershell
scoop bucket add deepgram-demos-rust https://github.com/deepgram-devs/deepgram-demos-rust
scoop install deepgram-demos-rust/dg-stt
```

Replace `dg-stt` with any available application:

| Application | Scoop command |
| --- | --- |
| Audio Recorder | `scoop install deepgram-demos-rust/audio-recorder` |
| Flux Turn-Taking | `scoop install deepgram-demos-rust/dg-flux` |
| Speech-to-Text | `scoop install deepgram-demos-rust/dg-stt` |
| Text-to-Speech | `scoop install deepgram-demos-rust/dg-tts` |
| Podcaster | `scoop install deepgram-demos-rust/dgpodcaster` |
| TTS TUI | `scoop install deepgram-demos-rust/tts-tui` |
| Voice Agent | `scoop install deepgram-demos-rust/voice-agent` |
| Velocity | `scoop install deepgram-demos-rust/velocity` |

The manifests are source packages: Scoop installs Rust via `rustup-msvc` and then builds the requested application from a SHA256-verified, immutable repository archive. This makes the branch usable before every application has a signed binary release. The first installation needs the Microsoft C++ Build Tools and Windows SDK and can take several minutes; later installs reuse Cargo's caches.

Each app still needs its own runtime configuration, such as a `DEEPGRAM_API_KEY`, after installation. Run `<app> --help` for its CLI options.

## Verify a manifest before publishing

From a Windows PowerShell prompt with Scoop installed:

```powershell
scoop install .\scoop\bucket\dg-stt.json
dg-stt --help
scoop uninstall dg-stt
```

Use [Test-ScoopManifests.ps1](Test-ScoopManifests.ps1) to validate all JSON files and verify that their pinned source archive has not changed.

## Updating the pinned archive

Run [Update-ScoopManifests.ps1](Update-ScoopManifests.ps1) after the target commit is on GitHub. It downloads the immutable archive, calculates its SHA256 hash, and updates every manifest together. Review the diff, validate at least one install, and commit the regenerated manifests.
