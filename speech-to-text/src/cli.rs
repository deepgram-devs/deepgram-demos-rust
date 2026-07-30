use clap::{Parser, Subcommand};

use crate::stream::StreamSource;
use crate::transcribe::TranscribeArgs;

#[derive(Parser)]
#[command(name = "dg-stt")]
#[command(about = "Deepgram Speech-to-Text CLI", long_about = None)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Commands,
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    /// Stream audio for real-time transcription
    Stream {
        #[command(subcommand)]
        source: StreamSource,
    },
    /// Transcribe pre-recorded audio file using HTTP API
    Transcribe {
        #[command(flatten)]
        args: TranscribeArgs,
    },
}
