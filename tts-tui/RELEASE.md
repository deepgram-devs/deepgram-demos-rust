# TTS TUI Release Checklist

The current release is `0.9.0` (2026-07-21).

## Required Release Files

Before cutting a release, verify these files are current:

- `README.md` documents all user-facing functionality and example commands.
- `CHANGELOG.md` includes all changes since the previous release.
- `TESTPLAN.md` includes automated and manual test coverage.
- `Cargo.toml` has the correct package version and description.
- The workspace `Cargo.lock` includes dependency changes needed by `tts-tui`.

## Pre-Release Verification

Run from the repository root:

```bash
cargo check -p tts-tui
cargo test -p tts-tui
```

Perform the manual checks in `TESTPLAN.md` that are relevant to the release scope. For provider changes, test at least:

- Hosted Deepgram through the `deepgram` provider.
- A self-hosted or proxy Deepgram-compatible HTTP endpoint through `--endpoint`.
- Self-hosted Deepgram on SageMaker through the `sagemaker` provider.

Do not commit API keys, AWS credentials, generated audio cache files, or local user configuration files.

## Binary Matrix

The TTS TUI release should include binaries for:

| Platform | Target triple | Artifact name |
|----------|---------------|---------------|
| macOS Apple silicon | `aarch64-apple-darwin` | `tts-tui-aarch64-apple-darwin.tar.gz` |
| macOS Intel | `x86_64-apple-darwin` | `tts-tui-x86_64-apple-darwin.tar.gz` |
| Linux x86_64 | `x86_64-unknown-linux-gnu` | `tts-tui-x86_64-unknown-linux-gnu.tar.gz` |
| Windows x86_64 | `x86_64-pc-windows-msvc` | `tts-tui-x86_64-pc-windows-msvc.zip` |

## GitHub Actions Release Workflow

Use `.github/workflows/tts-tui-release.yml` for release builds. It runs on native
GitHub-hosted runners for all four targets, packages each binary, uploads the
artifacts between jobs, generates `SHA256SUMS.txt`, and creates the GitHub release.

After updating the version in `Cargo.toml`, push an annotated tag matching it:

```bash
git tag -a tts-tui-v<version> -m "tts-tui v<version>"
git push origin tts-tui-v<version>
```

The workflow is triggered by tags matching `tts-tui-v*`. Confirm all four build
jobs and the publish job succeed before announcing the release.

## Packaging

Package each binary with:

- The `tts-tui` executable.
- `README.md`.
- `CHANGELOG.md`.
- `TESTPLAN.md`.
- `LICENSE.md` from the repository root.

Use `tts-tui.exe` for the Windows archive. Preserve executable permissions for Unix archives.

## Smoke Tests

For each platform artifact:

- Run `tts-tui --help` and verify the provider, endpoint, audio format, sample rate, SageMaker endpoint name, and AWS region options are listed.
- Launch the TUI and verify the terminal enters and exits alternate screen mode cleanly.
- Verify `~/.config/deepgram-tts-client.toml` is created or read without parse errors.

For at least one platform:

- Generate audio with the `deepgram` provider.
- Generate audio with the `deepgram` provider and a custom self-hosted `--endpoint`.
- Generate audio with the `sagemaker` provider against a self-hosted Deepgram SageMaker endpoint.

## Release Notes

Release notes should include:

- Version number and date.
- A short summary of user-facing changes.
- Any provider-specific migration notes.
- Known limitations or warnings, including any manual tests that were skipped.
- Do not copy binary checksums into the release description; GitHub exposes checksums for release assets separately.
