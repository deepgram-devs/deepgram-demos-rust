# Changelog

## [0.2.0] - 2026-02-20

### Added
- `--endpoint` CLI option to specify a custom Deepgram WebSocket endpoint (default: `wss://agent.deepgram.com`)
- `--speak-model` CLI option to select the TTS model (default: `aura-2-thalia-en`)
- `--think-type` CLI option to select the LLM provider type (default: `open_ai`)
- `--think-model` CLI option to select the LLM model (default: `gpt-4o-mini`)
- `--think-endpoint` CLI option to specify a custom LLM provider endpoint URL
- `--think-header` CLI option (repeatable) to pass custom headers to the LLM provider in `key=value` format
- `--verbose` CLI flag to print the full Settings JSON message at startup
- `--no-mic-mute` CLI flag to disable automatic microphone muting during agent audio playback
- Smart microphone muting: mic is automatically disabled when agent audio starts playing, and re-enabled 600ms after playback finishes (prevents audio feedback)
- Silent audio packet injection while mic is muted, to keep the WebSocket connection alive

## [0.1.0] - Initial Release

- Real-time microphone capture and streaming to Deepgram Voice Agent API via WebSocket
- Audio response playback using rodio
- Handles agent audio (binary and base64), transcripts, and status messages
- Configurable via environment variable `DEEPGRAM_API_KEY`
