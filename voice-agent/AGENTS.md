# Voice Agent application requirements

Keep this file up-to-date whenever voice-agent functionality or user-facing behavior changes. Add, revise, or remove the relevant requirement in the same change as the implementation.

## Functional requirements

- The voice-agent application must expose its interactive terminal UI as the `tui` subcommand (`voice-agent tui`); do not reintroduce a `--tui` flag.
- The voice-agent TUI must provide a searchable command palette that lists every available TUI function and displays the keyboard shortcut beside any command that has one. Shortcuts must be right-justified without colliding with command text.
- The voice-agent TUI must support Space as a connect/disconnect toggle, `?` help, and terminal restoration before exit.
- The bottom status bar must show the active listen model and TTS voice with concise human-readable labels.
- The voice-agent TUI must display both user transcription and agent response text from the Voice Agent connection, retain conversation history, and support toggling timestamps for all historical and future user/agent messages.
- Persist the timestamp-display setting in the Voice Agent YAML configuration and restore it when the TUI starts.
- Persist the verbose JSON logging setting in the Voice Agent YAML configuration and restore it when the TUI starts.
- Each newly established voice-agent connection must clear the conversation/message view and begin with a fresh history.
- The header connection status must always reflect the WebSocket state, including `Connection error` when a connection terminates because of an error.
- The voice-agent TUI must use distinct colors for user messages, agent messages, and client-side injected/control messages sent to the API.
- The voice-agent TUI must allow mouse selection of user and agent messages and provide a command-palette action to copy the complete conversation to the system clipboard.
- The voice-agent TUI must provide a separate command-palette action to copy only the current Deepgram `request_id` to the system clipboard, and must clear that ID when the connection ends.
- HTTP-to-WebSocket upgrade diagnostics must not be rendered in the TUI; they may be retained in diagnostic logging when enabled.
- The TUI must suppress rodio's `Dropping OutputStream` shutdown diagnostic so closing a connection does not print over the interface.
- The voice-agent TUI must provide a full system-prompt editor with text insertion, deletion, cursor navigation, and forward/backward navigation controls, and must send the edited prompt through the supported Voice Agent update message.
- The system-prompt editor must support word-wise backward and forward movement using the terminal's control-arrow navigation (for example, `Ctrl+Left` and `Ctrl+Right`).
- The TUI must provide a separate historical-system-prompt selector and persist selected/edited prompts for reuse.
- Persist TUI preferences as YAML at the platform-resolved Deepgram configuration path, using `$HOME/.config/deepgram/voice-agent.yml` on Unix-like systems and the equivalent per-user configuration directory on other platforms. Preserve unrelated YAML keys because other tools may use the same file.
- Persist historical prompts, the last TTS voice/model, and the complete listen transcription configuration in that YAML file, and apply those preferences to subsequent voice-agent connections.
- The voice-agent TUI must allow selecting every currently available Aura-2 and Flux TTS voice, selecting Flux or Aura-2 with the correct Voice Agent TTS API version, and must prompt for required fields before sending updates.
- TUI Settings must configure Deepgram speech with `agent.speak.provider.type: "deepgram"`, `version: "v2"` for Flux models, and `version: "v1"` for Aura models, following the Deepgram Voice Agent TTS specification.
- Never infer a Deepgram TTS family from an unrecognized model identifier. Reject unsupported providers and model families before sending Settings or `UpdateSpeak`.
- The voice-agent TUI must allow selecting Flux English STT, Flux Multilingual STT, or Nova-3 STT, choosing the listen language, and prompting for required fields before sending updates.
- `UpdateListen` messages must match the current Deepgram schema: use `listen.provider`, include `language` only for Nova V1, and include only Flux-supported EOT/keyterm/language-hint fields for Flux V2.
- Voice Agent client messages exposed by the TUI must use interactive fields rather than hard-coded values. Injection must prompt for message text; agent injection must also prompt for `default`, `queue`, or `interrupt` behavior. Update actions must prompt for or select their applicable fields. Fieldless controls may execute directly.
- The voice-agent TUI must provide verbose JSON logging as a command-palette toggle. When enabled, it must read `request_id` from Deepgram's `Welcome` message and write all client/server JSON messages to a new text file named `<request_id>.txt` in the system temporary directory, showing the path in the UI. If enabled mid-session, it must first write all previously retained JSON history before appending new messages.
- The JSON log location must appear as a warning-colored, clickable message in the message view. Clicking it must copy the fully qualified log-file path to the clipboard.
- When verbose JSON logging is enabled before a new Voice Agent connection, the log location must be the first message displayed in the newly cleared message view.
- The list of TTS voices can be filtered by the user

## Catalog maintenance

- Any voice-agent TTS selection list must include every currently available Aura-2 voice and every currently available Flux TTS voice. Maintain the catalog against Deepgram's [Aura voices documentation](https://developers.deepgram.com/docs/tts-models) and [Flux TTS voices documentation](https://developers.deepgram.com/docs/flux-tts/voices); do not ship abbreviated or featured-only lists.
- Do not include `aura-2-perseo-it`: it is not a valid Aura-2 voice despite its appearance in the published catalog.
