# Changelog

## 0.2.8 - 2026-08-10

- Added Flux TTS v2 support through `speak v2`, `save v2`, and `stream v2` subcommands.
- Added documented `/v2/speak` REST and WebSocket routing, Flux defaults, repeatable request tags, and v2 token authentication.
- Added default request tags (`tts-tui`, `appeng`, and `deepgram-demos-rust`) to all batch and streaming requests.
- Added a versioned `User-Agent` header to batch requests and streaming WebSocket handshakes.

## Unreleased

## 0.2.7 - 2026-08-02

- Initial cross-platform binary release.
- Includes interactive speech, file output, and WebSocket streaming Text-to-Speech modes.
