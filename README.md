# Deepgram Rust Samples

This repository contains sample applications using [Rust](https://rustup.rs/) and the [Deepgram APIs](https://developers.deepgram.com/reference/deepgram-api-overview) for [Text-to-Speech (TTS)](https://developers.deepgram.com/docs/tts-rest), [Speech-to-Text (STT)](https://developers.deepgram.com/docs/pre-recorded-audio), and [Voice Agent (VA)](https://developers.deepgram.com/docs/voice-agent).

Feel free to use these applications to learn from, and adapt them to your unique use cases.

## Running Rust Applications

In general, you can run a Rust application by changing into the project directory and running `cargo run`.
The project directory is typically the one containing a `Cargo.toml` file and a `./src/` child directory.

If you'd rather "install" the sample applications, so you can execute them using their binary name, you can run:

```bash
cargo install --path text-to-speech
```

If you try running `cargo run` from the repository root, you will need to add the `--bin` option to the `cargo` command.
Specify the project name from the `Cargo.toml` of the desired project.
The project name may be different than the directory containing the project files.

```bash
cargo run --bin rust-flux
```

If you want to view CLI help text for a particular command, when running with `cargo`, use the `--` delimiter and then pass the `--help` option.
The `--` delimiter causes any following options to be passed to the Rust application, instead of `cargo` itself.

```bash
cargo run --bin dg-tts -- --help
```

## Application Ideas

Use these ideas to inspire business and application ideas you could build with Deepgram!

- Schedule vehicle maintenance appointments automatically, with Voice Agent
- Language training application, using Text-to-Speech
- Customer service quality improvement, using Speech-to-Text transcription
- Sales tracking and marketing optimization, using Speech-to-Text transcriptions

## Sample Applications

| Name | Description | Deepgram APIs |
|------|-------------|---------------|
| [Speech-to-Text](speech-to-text/) | Real-time speech-to-text application capturing audio from default microphone | STT |
| [Text-to-Speech](text-to-speech/) | Command-line interface for generating and playing back speech | TTS |
| [Voice Agent](voice-agent/) | Bidirectional conversation using Voice Agent WebSocket APIs | VA |
| [Flux Turn-Taking](flux-turn-taking/) | Real-time audio streaming to Deepgram Flux API for speech recognition | STT |

## License

See `LICENSE.md`.
