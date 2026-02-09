# TTS TUI (Text-to-Speech Terminal User Interface)

This is a terminal user interface (TUI) application built with Rust and Ratatui that allows you to interact with the Deepgram Text-to-Speech (TTS) API.

## Features

- Select and play saved text snippets using various Deepgram Aura voices
- Browse and filter available Deepgram TTS voices in real-time
- Add new text snippets to the saved list
- Delete selected text snippets from the list
- Audio caching for faster repeated playback
- Built-in help screen with all keyboard shortcuts
- Mouse support for scrolling through lists
- Clipboard paste support for adding text
- Real-time logging of application actions and API responses
- ⚠️ **Not available**: Adjust TTS playback speed dynamically (+/- keys)

## How to Run

1. **Prerequisites:** Ensure you have Rust and Cargo installed.
2. **Deepgram API Key:** Obtain a Deepgram API Key and set it as an environment variable or in a `.env` file in the `tts-tui` directory:

```bash
export DEEPGRAM_API_KEY="YOUR_DEEPGRAM_API_KEY"
# or in tts-tui/.env file:
# DEEPGRAM_API_KEY="YOUR_DEEPGRAM_API_KEY"
```

3. **Navigate to the directory:**

```bash
cd tts-tui
```

4. **Run the application:**

```bash
cargo run
```

## Specify Custom Endpoint

If you'd like to specify a different Deepgram endpoint, such as Deepgram self-hosted or a non-production environment, you can use the `DEEPGRAM_TTS_ENDPOINT` environment variable or the `--endpoint` option.

```bash
# Use the --endpoint option
cargo run -- --endpoint https://selfhosted.example.com/v1/speak

# Use an environment variable
export DEEPGRAM_TTS_ENDPOINT=https://selfhosted.example.com/v1/speak
cargo run
```

## Keyboard Shortcuts

### General
- `?`: Show help screen with all keyboard shortcuts
- `q`: Quit the application (when focused on Saved Texts panel)
- `Enter`: Play the selected text snippet with the selected voice
- `Up`/`Down` arrows: Navigate through lists
- `Left`/`Right`/`Tab`: Switch focus between panels (Saved Texts, Voices)

### Playback Speed Control
- `+` or `=`: Increase playback speed
- `-`: Decrease playback speed
- `0`: Reset playback speed to default (1.0x)

### Text Management
- `n`: Enter input mode to add a new text snippet
  - While in input mode:
    - `Enter`: Save the new text
    - `Esc`: Cancel input
    - `Backspace`: Delete the last character
    - `Ctrl+V` or `Cmd+V`: Paste from clipboard
    - Any other character: Type into the input buffer
- `d`: Delete the currently selected text snippet

### Voice Filtering
- Type any character while focused on the Voices panel to filter voices by name
- `Backspace`: Remove last character from voice filter
- `Esc`: Clear voice filter (when voice filter is active)

### Mouse Controls
- Scroll wheel: Navigate through the Saved Texts list
