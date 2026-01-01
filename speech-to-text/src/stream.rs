use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum StreamSource {
    /// Stream audio from microphone for real-time transcription
    Microphone {
        /// Callback URL for receiving transcription results
        #[arg(long)]
        callback: Option<String>,

        /// Suppress console output of transcripts
        #[arg(long)]
        silent: bool,

        /// Override the Deepgram API base URL
        #[arg(long)]
        endpoint: Option<String>,

        /// Audio encoding format (e.g., linear16, mulaw, flac)
        #[arg(long)]
        encoding: Option<String>,

        /// Audio sample rate in Hz
        #[arg(long)]
        sample_rate: Option<u32>,

        /// Number of audio channels
        #[arg(long)]
        channels: Option<u16>,

        /// Enable multichannel processing
        #[arg(long)]
        multichannel: bool,

        /// Enable interim results
        #[arg(long)]
        interim_results: Option<bool>,

        /// Enable punctuation
        #[arg(long)]
        punctuate: Option<bool>,

        /// Enable smart formatting
        #[arg(long)]
        smart_format: Option<bool>,

        /// Deepgram model to use (e.g., nova-2, enhanced, base)
        #[arg(long)]
        model: Option<String>,

        /// Redact entities (comma-separated). Can include specific entities or categories: phi, pii, pci, other
        #[arg(long)]
        redact: Option<String>,

        /// Language code for transcription (e.g., en, es, fr, de)
        #[arg(long)]
        language: Option<String>,
    },
    /// Stream audio from a file for transcription
    File {
        /// Path to the audio file (supports MP3, WAV, FLAC)
        #[arg(short, long)]
        file: PathBuf,

        /// Stream audio as fast as possible instead of real-time rate
        #[arg(long)]
        fast: bool,

        /// Callback URL for receiving transcription results
        #[arg(long)]
        callback: Option<String>,

        /// Suppress console output of transcripts
        #[arg(long)]
        silent: bool,

        /// Override the Deepgram API base URL
        #[arg(long)]
        endpoint: Option<String>,

        /// Audio encoding format (e.g., linear16, mulaw, flac)
        #[arg(long)]
        encoding: Option<String>,

        /// Audio sample rate in Hz
        #[arg(long)]
        sample_rate: Option<u32>,

        /// Number of audio channels
        #[arg(long)]
        channels: Option<u16>,

        /// Enable multichannel processing
        #[arg(long)]
        multichannel: bool,

        /// Enable interim results
        #[arg(long)]
        interim_results: Option<bool>,

        /// Enable punctuation
        #[arg(long)]
        punctuate: Option<bool>,

        /// Enable smart formatting
        #[arg(long)]
        smart_format: Option<bool>,

        /// Deepgram model to use (e.g., nova-2, enhanced, base)
        #[arg(long)]
        model: Option<String>,

        /// Redact entities (comma-separated). Can include specific entities or categories: phi, pii, pci, other
        #[arg(long)]
        redact: Option<String>,

        /// Language code for transcription (e.g., en, es, fr, de)
        #[arg(long)]
        language: Option<String>,
    },
}
