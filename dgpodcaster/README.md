# Podcaster

A terminal user interface (TUI) application for generating dynamic podcasts using AI-generated scripts and Text-to-Speech voices. Create engaging multi-speaker podcasts on any topic with different Deepgram Aura-2 voices.

## Features

- Interactive TUI built with Ratatui
- AI-generated podcast scripts using Google Gemini API
- Multiple speaker support (1-4 speakers)
- High-quality text-to-speech using Deepgram Aura-2 voices
- Automatic audio concatenation to create final podcast file

## Prerequisites

Before running this application, you need:

1. **Deepgram API Key**: Get one at https://deepgram.com
2. **Google Gemini API Key**: Get one at https://ai.google.dev

## Setup

1. Set your API keys as environment variables:

```bash
export DEEPGRAM_API_KEY="your-deepgram-api-key"
export GEMINI_API_KEY="your-gemini-api-key"
```

2. Build the application:

```bash
cargo build --release
```

## Usage

Run the application:

```bash
cargo run --release
```

### Workflow

1. **Enter Topic**: Type a topic for your podcast and press Enter
   - You can paste text from clipboard using CMD+V (macOS) or Ctrl+V (Linux/Windows)
   - Example: "The history of nuclear power plants"
   - Example: "The invention of AI software and hardware"

2. **Select Speaker Count**: Press 1-4 to select the number of speakers, then press Enter

3. **Assign Voices**:
   - Use arrow keys to navigate between speakers (left pane) and voices (right pane)
   - Press Left/Right to switch between panes
   - Press Up/Down to select items within a pane
   - Press Enter to assign the selected voice to the selected speaker
   - Once all speakers have voices assigned, press **Ctrl+G** to generate the podcast

4. **Generation**: Wait while the application:
   - Generates the podcast script using Gemini
   - Creates audio clips for each utterance using Deepgram TTS
   - Concatenates all clips into a single file

5. **Complete**: The final podcast is saved as `podcast_output.wav`

## Keyboard Shortcuts

- **CMD+V / Ctrl+V**: Paste text from clipboard (topic input screen)
- **Esc**: Go back to previous screen or quit (from topic input)
- **Arrow Keys**: Navigate between UI elements
- **Enter**: Confirm selection or assign voice
- **Ctrl+G**: Generate podcast (from voice assignment screen)
- **q**: Quit (from completion or error screen)

## Available Voices

The application includes all 41 Deepgram Aura-2 English voices:
- aura-2-amalthea-en
- aura-2-andromeda-en
- aura-2-apollo-en
- aura-2-arcas-en
- aura-2-aries-en
- aura-2-asteria-en
- aura-2-athena-en
- aura-2-atlas-en
- aura-2-aurora-en
- aura-2-callista-en
- aura-2-cora-en
- aura-2-cordelia-en
- aura-2-delia-en
- aura-2-draco-en
- aura-2-electra-en
- aura-2-harmonia-en
- aura-2-helena-en
- aura-2-hera-en
- aura-2-hermes-en
- aura-2-hyperion-en
- aura-2-iris-en
- aura-2-janus-en
- aura-2-juno-en
- aura-2-jupiter-en
- aura-2-luna-en
- aura-2-mars-en
- aura-2-minerva-en
- aura-2-neptune-en
- aura-2-odysseus-en
- aura-2-ophelia-en
- aura-2-orion-en
- aura-2-orpheus-en
- aura-2-pandora-en
- aura-2-phoebe-en
- aura-2-pluto-en
- aura-2-saturn-en
- aura-2-selene-en
- aura-2-thalia-en
- aura-2-theia-en
- aura-2-vesta-en
- aura-2-zeus-en

## Output

The generated podcast is saved as `podcast_output.wav` in WAV format:
- Sample rate: 48kHz
- Channels: Mono
- Bit depth: 16-bit

## Project Structure

```
src/
├── main.rs       # Main application logic and state management
├── ui.rs         # TUI rendering for all screens
├── gemini.rs     # Google Gemini API integration for script generation
├── deepgram.rs   # Deepgram TTS API integration
└── audio.rs      # Audio file concatenation
```
