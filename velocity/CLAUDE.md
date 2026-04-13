# Velocity

What is Velocity? It's a dictation app for Windows 11 that's powered by Deepgram's Voice AI APIs.

- This is a Windows 11 application written in Rust
- Use the `windows` crate for Rust to plug into the Windows 11 native APIs
- Make sure you use the latest Rust dependency versions
- Read the documentation for the latest versions of Rust crates, if you run into any coding issues

- It uses the Deepgram Speech-to-Text (STT) transcription API
- Use the Windows `RegisterHotKey` API
- Capture linear16 PCM audio at 48000 sample rate and send to Deepgram API
- Use an in-memory audio buffer and send to the HTTP API for now.
  - Do not use the Deepgram WebSocket streaming API.
  - Do not perform any post-processing of the text, just type it exactly as the transcript result appears.
- Use the Windows 11 API SDK to type the resulting transcription characters on the keyboard as input
- The application does not have any user interface
- Read the [documentation for Deepgram](https://developers.deepgram.com/reference/speech-to-text/listen-pre-recorded) HTTP API to understand the response payload format.
- Make a notification sound when audio capture starts and stops for user feedback.
- Use the latest version of the [windows](https://crates.io/crates/windows) Rust crate
- Any error messages from the Deepgram API or interpreting the response should be logged if the `--verbose` option is specified

## User Interaction

- Application only runs in the system tray using a generic icon for now.
  - Create a plain, white circle icon for now. I will update the icon later.
- "Keep Talking": If the user presses keyboard shortcut `SHIFT+CTRL+WIN+'`, then keep the recording active until they press the same shortcut again.
- "Push to Talk": While the user presses and is holding down the `WIN+CTRL+'` keys, capture audio from the microphone
  - When the user releases the `WIN+CTRL+'` keys, send the audio to Deepgram HTTP API for transcription
- "Text Streaming": When the user presses `CTRL+WIN+[` it should activate streaming mode.
  - Text is continuously written to the active application until the user presses the shortcut again.
  - This mode uses the [Deepgram WebSocket API](https://developers.deepgram.com/reference/speech-to-text/listen-streaming) for nova-3.
  - Do not user the interim results feature.
  - Make sure that WebSocket `KeepAlive` messages are sent every 5 seconds.
  - When the user activates this mode, open a WebSocket connection to Deepgram.
  - When the user deactivates this mode, close the activate WebSocket connection using the documented `CloseStream` message.
  - If the user changes focus to a different window, disable the streaming mode immediately.
  - If this mode is active, the user cannot use the other application modes, such as push to talk or keep talking.
- The hotkeys must work globally across any application that currently has focus.
- The user can specify the `--model` option to determine which `model` query string is passed to Deepgram API. The default value is `nova-3`.

## Deepgram Features and Configuration

- The user can specify `--smart-format` to enable the Smart Formatting feature in Deepgram
  - This can also be stored in the configuration file and read at startup
- Reload the configuration file if it changes while the application is running
- The user can specify their Deepgram API key and the application will store it in a configuration file
  - If configuation file or API key is missing or invalid, then pop up a box asking the user to enter the Deepgram API key
  - Write the API key to the configuration file after the user specifies it.
- Configuration file should be under the user home directory `%USERPROFILE%`, under `.config`
  - If no configuration file exists, then create it. Config file is named `velocity.yml`

## Miscellaneous Requirements

- Add a `CTRL+C` function handler that exits the application if it's running inside a terminal

## Documentation

- Make sure the README is always updated with any changes to application functionality
- The README should be focused on the user experience, like installing, using application features, and uninstalling the application
