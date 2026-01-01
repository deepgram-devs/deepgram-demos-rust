use clap::Args;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Args)]
pub struct TranscribeArgs {
    /// Path to the audio file
    #[arg(short, long)]
    pub file: PathBuf,

    /// Deepgram model to use (e.g., nova-3, nova-2, enhanced, base)
    #[arg(long)]
    pub model: Option<String>,

    /// Language code for transcription (e.g., en, es, fr, de)
    #[arg(long)]
    pub language: Option<String>,

    /// Enable punctuation
    #[arg(long)]
    pub punctuate: Option<bool>,

    /// Enable smart formatting
    #[arg(long)]
    pub smart_format: Option<bool>,

    /// Enable diarization (speaker detection)
    #[arg(long)]
    pub diarize: Option<bool>,

    /// Enable multichannel processing
    #[arg(long)]
    pub multichannel: bool,

    /// Enable sentiment analysis
    #[arg(long)]
    pub sentiment: Option<bool>,

    /// Enable summarization (v2 for advanced)
    #[arg(long)]
    pub summarize: Option<String>,

    /// Enable topic detection
    #[arg(long)]
    pub topics: Option<bool>,

    /// Enable intent detection
    #[arg(long)]
    pub intents: Option<bool>,

    /// Enable entity detection
    #[arg(long)]
    pub detect_entities: Option<bool>,

    /// Redact entities (comma-separated). Can include specific entities or categories: phi, pii, pci, numbers
    #[arg(long)]
    pub redact: Option<String>,

    /// Audio encoding format (e.g., linear16, mulaw, flac)
    #[arg(long)]
    pub encoding: Option<String>,

    /// Output format (json, verbose-json, or text)
    #[arg(long, default_value = "text")]
    pub output: String,

    /// Override the Deepgram API base URL
    #[arg(long)]
    pub endpoint: Option<String>,
}

// Response structures for pre-recorded API
#[derive(Debug, Deserialize)]
struct PreRecordedResponse {
    metadata: PreRecordedMetadata,
    results: PreRecordedResults,
}

#[derive(Debug, Deserialize)]
struct PreRecordedMetadata {
    request_id: String,
    #[serde(default)]
    duration: f64,
    #[serde(default)]
    channels: u32,
}

#[derive(Debug, Deserialize)]
struct PreRecordedResults {
    channels: Vec<PreRecordedChannel>,
    #[serde(default)]
    summary: Option<PreRecordedSummary>,
}

#[derive(Debug, Deserialize)]
struct PreRecordedChannel {
    alternatives: Vec<PreRecordedAlternative>,
}

#[derive(Debug, Deserialize)]
struct PreRecordedAlternative {
    transcript: String,
    #[serde(default)]
    confidence: f64,
    #[serde(default)]
    words: Vec<PreRecordedWord>,
    #[serde(default)]
    paragraphs: Option<PreRecordedParagraphs>,
    #[serde(default)]
    entities: Vec<PreRecordedEntity>,
    #[serde(default)]
    summaries: Vec<PreRecordedSummary>,
    #[serde(default)]
    topics: Vec<PreRecordedTopic>,
    #[serde(default)]
    intents: Vec<PreRecordedIntent>,
    #[serde(default)]
    sentiments: Vec<PreRecordedSentiment>,
}

#[derive(Debug, Deserialize)]
struct PreRecordedWord {
    word: String,
    start: f64,
    end: f64,
    confidence: f64,
    #[serde(default)]
    speaker: Option<u32>,
    #[serde(default)]
    punctuated_word: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PreRecordedParagraphs {
    paragraphs: Vec<PreRecordedParagraph>,
}

#[derive(Debug, Deserialize)]
struct PreRecordedParagraph {
    sentences: Vec<PreRecordedSentence>,
}

#[derive(Debug, Deserialize)]
struct PreRecordedSentence {
    text: String,
    start: f64,
    end: f64,
}

#[derive(Debug, Deserialize)]
struct PreRecordedEntity {
    label: String,
    value: String,
    confidence: f64,
    start_word: usize,
    end_word: usize,
}

#[derive(Debug, Deserialize)]
struct PreRecordedSummary {
    #[serde(default)]
    summary: String,
    #[serde(default)]
    start_word: usize,
    #[serde(default)]
    end_word: usize,
}

#[derive(Debug, Deserialize)]
struct PreRecordedTopic {
    topic: String,
    confidence: f64,
}

#[derive(Debug, Deserialize)]
struct PreRecordedIntent {
    intent: String,
    confidence: f64,
}

#[derive(Debug, Deserialize)]
struct PreRecordedSentiment {
    sentiment: String,
    confidence: f64,
    start_word: usize,
    end_word: usize,
}

fn parse_redact_entities(redact_value: &str) -> Vec<String> {
    let mut entities = Vec::new();

    // Split by comma and trim whitespace
    for item in redact_value.split(',') {
        let item = item.trim();

        if !item.is_empty() {
            // Keep categories and individual entities as-is
            // The API will handle category expansion on the server side
            entities.push(item.to_lowercase());
        }
    }

    // Remove duplicates while preserving order
    let mut seen = std::collections::HashSet::new();
    entities.retain(|e| seen.insert(e.clone()));

    entities
}

pub async fn run_transcribe_mode(
    api_key: String,
    args: TranscribeArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Transcribing audio file: {}", args.file.display());

    // Read the audio file
    let audio_data = std::fs::read(&args.file)?;
    println!("Read {} bytes from file", audio_data.len());

    // Build the API URL with query parameters
    let base_url = args.endpoint.unwrap_or_else(|| "https://api.deepgram.com".to_string());
    let mut url = format!("{}/v1/listen?", base_url);
    let mut params = Vec::new();

    // Add model parameter
    if let Some(model_name) = args.model {
        params.push(format!("model={}", model_name));
    }

    // Add language parameter
    if let Some(lang) = args.language {
        params.push(format!("language={}", lang));
    }

    // Add punctuate parameter
    if let Some(punct) = args.punctuate {
        params.push(format!("punctuate={}", punct));
    }

    // Add smart_format parameter
    if let Some(smart) = args.smart_format {
        params.push(format!("smart_format={}", smart));
    }

    // Add diarize parameter
    if let Some(diar) = args.diarize {
        params.push(format!("diarize={}", diar));
    }

    // Add multichannel parameter
    if args.multichannel {
        params.push("multichannel=true".to_string());
    }

    // Add sentiment parameter
    if let Some(sent) = args.sentiment {
        params.push(format!("sentiment={}", sent));
    }

    // Add summarize parameter
    if let Some(summ) = args.summarize {
        params.push(format!("summarize={}", summ));
    }

    // Add topics parameter
    if let Some(top) = args.topics {
        params.push(format!("topics={}", top));
    }

    // Add intents parameter
    if let Some(int) = args.intents {
        params.push(format!("intents={}", int));
    }

    // Add detect_entities parameter
    if let Some(ent) = args.detect_entities {
        params.push(format!("detect_entities={}", ent));
    }

    // Add redact parameter
    if let Some(redact_value) = args.redact {
        let redact_entities = parse_redact_entities(&redact_value);
        if !redact_entities.is_empty() {
            params.push(format!("redact={}", redact_entities.join("&redact=")));
        }
    }

    // Add encoding parameter
    if let Some(enc) = args.encoding {
        params.push(format!("encoding={}", enc));
    }

    // Join all parameters
    url.push_str(&params.join("&"));

    println!("Sending request to Deepgram API...");

    // Create HTTP client
    let client = reqwest::Client::new();

    // Send POST request
    let response = client
        .post(&url)
        .header("Authorization", format!("Token {}", api_key))
        .header("Content-Type", "application/octet-stream")
        .body(audio_data)
        .send()
        .await?;

    // Check response status
    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await?;
        return Err(format!("API request failed with status {}: {}", status, error_text).into());
    }

    // Parse response
    let response_text = response.text().await?;

    // Output based on format
    match args.output.as_str() {
        "json" => {
            // Parse and pretty-print JSON
            let parsed: serde_json::Value = serde_json::from_str(&response_text)?;
            println!("\n{}", serde_json::to_string_pretty(&parsed)?);
        }
        "verbose-json" => {
            // Output raw JSON response
            println!("\n{}", response_text);
        }
        "text" | _ => {
            // Parse and display transcript with additional info
            let response: PreRecordedResponse = serde_json::from_str(&response_text)?;

            println!("\n=== Transcription Results ===");
            println!("Request ID: {}", response.metadata.request_id);
            println!("Duration: {:.2}s", response.metadata.duration);
            println!("Channels: {}", response.metadata.channels);
            println!();

            for (i, channel) in response.results.channels.iter().enumerate() {
                if response.results.channels.len() > 1 {
                    println!("Channel {}:", i);
                }

                for alternative in &channel.alternatives {
                    println!("Transcript:");
                    println!("{}", alternative.transcript);
                    println!("\nConfidence: {:.1}%", alternative.confidence * 100.0);

                    // Display speaker diarization if available
                    if alternative.words.iter().any(|w| w.speaker.is_some()) {
                        println!("\n=== Speaker Diarization ===");
                        let mut current_speaker: Option<u32> = None;
                        let mut speaker_text = String::new();

                        for word in &alternative.words {
                            if let Some(speaker) = word.speaker {
                                if current_speaker != Some(speaker) {
                                    if !speaker_text.is_empty() {
                                        println!("Speaker {}: {}", current_speaker.unwrap(), speaker_text.trim());
                                        speaker_text.clear();
                                    }
                                    current_speaker = Some(speaker);
                                }
                                speaker_text.push_str(&format!("{} ", word.punctuated_word.as_ref().unwrap_or(&word.word)));
                            }
                        }
                        if !speaker_text.is_empty() {
                            println!("Speaker {}: {}", current_speaker.unwrap(), speaker_text.trim());
                        }
                    }

                    // Display entities if available
                    if !alternative.entities.is_empty() {
                        println!("\n=== Detected Entities ===");
                        for entity in &alternative.entities {
                            println!("{}: {} (confidence: {:.1}%)", entity.label, entity.value, entity.confidence * 100.0);
                        }
                    }

                    // Display topics if available
                    if !alternative.topics.is_empty() {
                        println!("\n=== Topics ===");
                        for topic in &alternative.topics {
                            println!("{} (confidence: {:.1}%)", topic.topic, topic.confidence * 100.0);
                        }
                    }

                    // Display intents if available
                    if !alternative.intents.is_empty() {
                        println!("\n=== Intents ===");
                        for intent in &alternative.intents {
                            println!("{} (confidence: {:.1}%)", intent.intent, intent.confidence * 100.0);
                        }
                    }

                    // Display summaries if available
                    if !alternative.summaries.is_empty() {
                        println!("\n=== Summaries ===");
                        for summary in &alternative.summaries {
                            if !summary.summary.is_empty() {
                                println!("{}", summary.summary);
                            }
                        }
                    }

                    // Display sentiment analysis if available
                    if !alternative.sentiments.is_empty() {
                        println!("\n=== Sentiment Analysis ===");
                        for sentiment in &alternative.sentiments {
                            println!("{} (confidence: {:.1}%)", sentiment.sentiment, sentiment.confidence * 100.0);
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
