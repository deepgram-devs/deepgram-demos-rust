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

        /// Enable speaker diarization (identify individual speakers)
        #[arg(long)]
        diarize: bool,

        /// Detect named entities (people, places, organizations, etc.)
        #[arg(long)]
        detect_entities: bool,

        /// Enable interim results
        #[arg(long)]
        interim_results: bool,

        /// Enable voice activity detection events
        #[arg(long)]
        vad_events: bool,

        /// Enable punctuation
        #[arg(long)]
        punctuate: bool,

        /// Enable smart formatting
        #[arg(long)]
        smart_format: bool,

        /// Enable sentiment analysis
        #[arg(long)]
        sentiment: bool,

        /// Enable intent recognition
        #[arg(long)]
        intents: bool,

        /// Enable topic detection
        #[arg(long)]
        topics: bool,

        /// Deepgram model to use (e.g., nova-2, enhanced, base)
        #[arg(long)]
        model: Option<String>,

        /// Redact entities (comma-separated). Can include specific entities or categories: phi, pii, pci, other
        #[arg(long)]
        redact: Option<String>,

        /// Language code for transcription (e.g., en, es, fr, de)
        #[arg(long)]
        language: Option<String>,

        /// Comma-separated keyterms to boost recognition for (nova-3+ only, e.g. --keyterm "Deepgram,nova-3,speech AI")
        #[arg(long, conflicts_with = "keywords")]
        keyterm: Option<String>,

        /// Comma-separated keywords to boost recognition for (nova-2 and older, optional intensifier per word, e.g. --keywords "Deepgram:2,API,speech:-1")
        #[arg(long, conflicts_with = "keyterm")]
        keywords: Option<String>,
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

        /// Enable speaker diarization (identify individual speakers)
        #[arg(long)]
        diarize: bool,

        /// Detect named entities (people, places, organizations, etc.)
        #[arg(long)]
        detect_entities: bool,

        /// Enable interim results
        #[arg(long)]
        interim_results: bool,

        /// Enable voice activity detection events
        #[arg(long)]
        vad_events: bool,

        /// Enable punctuation
        #[arg(long)]
        punctuate: bool,

        /// Enable smart formatting
        #[arg(long)]
        smart_format: bool,

        /// Enable sentiment analysis
        #[arg(long)]
        sentiment: bool,

        /// Enable intent recognition
        #[arg(long)]
        intents: bool,

        /// Enable topic detection
        #[arg(long)]
        topics: bool,

        /// Deepgram model to use (e.g., nova-2, enhanced, base)
        #[arg(long)]
        model: Option<String>,

        /// Redact entities (comma-separated). Can include specific entities or categories: phi, pii, pci, other
        #[arg(long)]
        redact: Option<String>,

        /// Language code for transcription (e.g., en, es, fr, de)
        #[arg(long)]
        language: Option<String>,

        /// Endpointing sensitivity in milliseconds (e.g., 10, 300, 500). Controls how long
        /// Deepgram waits after speech stops before finalizing a transcript segment.
        /// Lower values produce faster but potentially incomplete results.
        #[arg(long)]
        endpointing: Option<u32>,

        /// Utterance end timeout in milliseconds (e.g., 1000). Deepgram sends an UtteranceEnd
        /// message after this many ms of silence, signaling the end of an utterance.
        /// Requires --interim-results and --vad-events to also be specified.
        #[arg(long)]
        utterance_end: Option<u32>,

        /// Comma-separated keyterms to boost recognition for (nova-3+ only, e.g. --keyterm "Deepgram,nova-3,speech AI")
        #[arg(long, conflicts_with = "keywords")]
        keyterm: Option<String>,

        /// Comma-separated keywords to boost recognition for (nova-2 and older, optional intensifier per word, e.g. --keywords "Deepgram:2,API,speech:-1")
        #[arg(long, conflicts_with = "keyterm")]
        keywords: Option<String>,
    },
}
