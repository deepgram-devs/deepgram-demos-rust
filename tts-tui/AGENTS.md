# AGENTS.md

- Ensure releases include Windows, Linux, and macOS (Intel and Apple ARM) binaries.
- The README always needs to reflect the current application capabilities.
- When updating `tts-tui`, update `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, `README.md`, and `TESTPLAN.md` as appropriate.
- Publish releases through `.github/workflows/tts-tui-release.yml`. Do not rely on local cross-compilation for release artifacts.
- After merging an application update, push an annotated tag named `tts-tui-v<version>` (matching `Cargo.toml`). The workflow builds native artifacts on GitHub Actions runners and creates the GitHub release from those artifacts.
- Verify the workflow completes all four targets before considering the release complete: `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`, and `x86_64-pc-windows-msvc`.
