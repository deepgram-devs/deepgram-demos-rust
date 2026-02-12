# Changelog

## 0.2.5 - 2026-02-11

### Features

- **Enhanced log styling** - Logs now display with color-coded icons (✓ Success in green, ✗ Error in red, ℹ Info in blue)
- **Accurate playback progress bar** - Added MP3 duration parsing for precise audio playback progress tracking
- **Audio abort on ESC** - Press ESC during audio playback to immediately stop playback
- **Voice panel organization** - Voices now grouped by language with visual separators
- **Text list metadata** - Display character count for each text
- **Scrollable help screen** - Use Up/Down arrow keys to scroll through help content on small screens
- **Mouse click support** - Click on the Saved Texts or Deepgram Voices blocks to set focus
- **Gender indicators** - Voices now display gender symbols (♂ male, ♀ female) for quick identification
- **Keyboard shortcut** - CTRL+Q to quit application from any focused panel

### Bug Fixes

- Fixed voice selection index issue when language separators were present
- Fixed audio playback state not resetting when starting new audio clips
- Ensured status bar resets and displays correctly when generating audio while playback is active

### Dependencies

- Added `mp3-duration` for accurate MP3 audio duration parsing

## 0.2.4 - 2026-02-10

- Add loading indicator during audio synthesis with responsive UI
- Fix text wrapping in "enter new text" popup box
- Remove speed query string parameter if set to 1.00x

## 0.2.3 - 2026-02-07

- Add saved text persistence to local filesystem

## 0.2.2 - 2026-02-04

- Add the TTS speed setting in the UI

## 0.2.1 - 2026-02-02

- Added support for specifying a custom endpoint with `--endpoint` or `DEEPGRAM_TTS_ENDPOINT` variable.
