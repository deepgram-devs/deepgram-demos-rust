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
    /// List available speech-to-text models
    ListModels {
        /// Include non-latest model versions
        #[arg(long)]
        include_outdated: bool,

        /// Override the Deepgram API base URL
        #[arg(long)]
        endpoint: Option<String>,
    },
}
