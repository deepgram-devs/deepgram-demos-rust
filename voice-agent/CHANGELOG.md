# Changelog

## Unreleased

## [0.1.3] - 2026-07-14

### Changed
- Added a default system prompt that instructs the Voice Agent to keep responses concise; `--prompt` continues to override it.
- Added Amazon Bedrock think-provider CLI options for temperature, endpoint, IAM/STS credential type, region, access key, secret key, and session token.
- Added validation for required Bedrock endpoint and credentials; reusable configurations reject embedded AWS credentials.
- Added `config create`, `config use`, and `config delete` subcommands for managing reusable Deepgram agent configurations.
- Agent configuration creation and deletion now discover accessible projects automatically when `--project-id` is omitted. A sole project is selected automatically; multiple projects require an explicit project ID.
- Added `config variable` subcommands to create, list, get, update, and delete reusable agent template variables.
- Updated variable creation requests to send `is_sensitive: false` as a JSON boolean.
- Removed deprecated top-level `agent.language` from the Voice Agent Settings message; `--listen-language` now only sets `agent.listen.provider.language`.
- Omitted `agent.listen.provider.language` automatically when the listen model starts with `flux-`.
- Omitted `agent.listen.provider.smart_format` from Settings JSON unless `--listen-smart-format` is specified.
- Printed the Voice Agent request ID when `--verbose` is enabled.
- Added comma-separated `--language-hint` CLI support that serializes to `agent.listen.provider.language_hints`.

## [0.1.2] - 2026-05-22

### Added
- Added CLI options for Deepgram listen provider type, model, version, language, keyterms, `eot_threshold`, and `eager_eot_threshold`.

## [0.2.2] - 2026-04-02

### Fixed
- Corrected agent system prompt JSON field from `agent.think.instructions` to `agent.think.prompt` to match the Deepgram Voice Agent API

### Added
- `--prompt` CLI option documented in README

## [0.2.1] - 2026-02-20

### Fixed
- Corrected server message type names to match the Deepgram Voice Agent API (`ConversationText` with `role`/`content` fields, `AgentThinking`, `AgentStartedSpeaking`, etc.) — transcripts were silently dropped due to mismatched names
- Fixed default log level so conversational output appears without setting `RUST_LOG` (was broken due to `filter_level` being overridden by `from_default_env`)
- Demoted all operational/diagnostic log entries to `debug` level; default output now shows only conversation transcripts and essential status
- Added proper `error!`/`warn!` handling for `Error` and `Warning` messages from the API
- Removed unused `base64` dependency

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
