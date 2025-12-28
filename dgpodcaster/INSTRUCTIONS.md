
- In this directory, create a Rust CLI application as a terminal user interface (TUI)
- The user can navigate the TUI using keyboard commands and shortcuts.
- The purpose of this application is to generate dynamic podcasts, using speaker scripts generated from an LLM service, and Text-to-Speech (TTS) voice synthesis. Each podcast speaker will have a different voice so that the listener can differentiate between them.


- Use the latest release version of the ratatui crate for Rust
- Use the Deepgram API to perform Text-to-Speech (TTS) API operations for the podcast.
- Use "rig" Rust crate to call the Google Gemini API with the Gemini 3 Flash Preview model to generate the podcast text. https://github.com/0xPlaygrounds/rig
- Modularize the code into Rust modules logically, so that the code base is more easily maintainable.

## User Flow

- When the user launches the application, prompt them to enter some text for a topic that they want to generate a podcast for. Example topics could potentially include:
  - Example topic: The history of nuclear power plants.
  - Example topic: The history of the invention of AI software and hardware, and the mathematics behind it.
  - Example topic: The difference between 120-volt and 240-volt infrastructure across different countries.
- After prompting the user for the topic, prompt the user to select the number of podcast speakers that they want, from a list of 1 to 4.
- After the user selects the number of speakers, expose an interface with the left pane showing each podcast speaker in a list. The user can use the up and down arrow keys to select the podcast speaker number from the list. The user can use the left and right arrow keys to switch between the left and right pane. On the right pane, list all of the Deepgram aura-2 Text-to-Speech (TTS) voices that are available from this documentation: https://developers.deepgram.com/docs/tts-models 
  - When the user selects an aura-2 voice from the right pane, associate that to the selected podcast speaker on the left.
- Specify a keyboard shortcut to continue to generating the podcast audio, such as ctrl+g.

## Generate Podcast

- After the user has completed providing inputs to the application, we need to generate the final podcast audio file.
- Use the "rig" crate to generate a podcast audio script, using the following prompt template: "Generate a podcast script that has [SPEAKER_COUNT] distinct speakers. Only output the script itself with the speaker ID prefix before each utterance. Do not output anything else except the podcast script. The podcast should be approximately five minutes long. Each speaker should have approximately equal time spent speaking. The script should be conversational in nature, so that it sounds like the podcast speakers are being personal and sharing ideas with each other. The topic of the podcast is: [PODCAST_TOPIC]."
  - Substitute "[SPEAKER_COUNT]" with the number of speakers that the user selected before.
  - Substitute "[PODCAST_TOPIC]" with the topic that the user previously specified in the text input field.
- Divide the podcast script utterances in order, and specify which speaker will utter the phrase.
- Use the Deepgram TTS HTTP API to generate the audio for each utterance, using the associated speaker's aura-2 voice.
- Save each of the generated audio clips separately, and ensure you keep track of the order that they need to be in.
- After generating all of the speaker audio utterances, join them together into a single audio file by concatenating them in the correct order.
