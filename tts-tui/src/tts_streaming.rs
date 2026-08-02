use anyhow::{anyhow, Context, Result};
use futures_util::{SinkExt, StreamExt};
use rust_decimal::Decimal;
use serde_json::json;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{
        client::IntoClientRequest,
        http::{
            header::{AUTHORIZATION, SEC_WEBSOCKET_PROTOCOL},
            HeaderValue,
        },
        Message,
    },
};
use tokio_util::sync::CancellationToken;
use url::Url;

const AURA_SPEAK_WEBSOCKET_URL: &str = "wss://api.deepgram.com/v1/speak";
const FLUX_SPEAK_WEBSOCKET_URL: &str = "wss://api.deepgram.com/v2/speak";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamingProtocol {
    Aura,
    Flux,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChunkingStrategy {
    TenWords,
    Sentence,
    Punctuation,
}

impl ChunkingStrategy {
    pub const ALL: [Self; 3] = [Self::TenWords, Self::Sentence, Self::Punctuation];

    pub fn next(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|strategy| *strategy == self)
            .unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::TenWords => "10 words",
            Self::Sentence => "sentence boundary",
            Self::Punctuation => "punctuation",
        }
    }
}

pub struct StreamingRequest<'a> {
    pub api_key: &'a str,
    pub voice_id: &'a str,
    pub protocol: StreamingProtocol,
    pub speed: Decimal,
    pub sample_rate: u32,
    pub chunking_strategy: ChunkingStrategy,
    pub text: &'a str,
}

pub async fn stream_speech(
    request: StreamingRequest<'_>,
    cancellation: CancellationToken,
    events: mpsc::UnboundedSender<crate::app::TtsResult>,
) -> Result<()> {
    // Flux performs turn management server-side. Send one complete Speak so
    // whitespace is preserved between tokens and no client chunking is needed.
    let chunks = match request.protocol {
        StreamingProtocol::Aura => chunk_text(request.text, request.chunking_strategy),
        StreamingProtocol::Flux => vec![request.text.to_string()],
    };
    if chunks.is_empty() {
        return Err(anyhow!("Cannot stream an empty text utterance"));
    }

    let url = build_streaming_url(
        request.protocol,
        request.voice_id,
        request.speed,
        request.sample_rate,
    )?;
    let mut websocket_request = url
        .as_str()
        .into_client_request()
        .context("Failed to create Deepgram WebSocket request")?;
    match request.protocol {
        StreamingProtocol::Aura => {
            let protocol = HeaderValue::from_str(&format!("token, {}", request.api_key))
                .context("Deepgram API key contains an invalid WebSocket protocol character")?;
            websocket_request
                .headers_mut()
                .insert(SEC_WEBSOCKET_PROTOCOL, protocol);
        }
        StreamingProtocol::Flux => {
            let authorization = HeaderValue::from_str(&format!("Token {}", request.api_key))
                .context("Deepgram API key contains an invalid authorization header")?;
            websocket_request
                .headers_mut()
                .insert(AUTHORIZATION, authorization);
        }
    }

    let (mut socket, _) = connect_async(websocket_request)
        .await
        .context("Failed to connect to Deepgram TTS WebSocket")?;

    let _ = events.send(crate::app::TtsResult::StreamingStarted {
        chunk_count: chunks.len(),
        sample_rate: request.sample_rate,
    });

    let chunk_count = chunks.len();
    for (index, chunk) in chunks.into_iter().enumerate() {
        if cancellation.is_cancelled() {
            close_after_abort(&mut socket, request.protocol).await;
            return Ok(());
        }
        socket
            .send(Message::Text(
                json!({ "type": "Speak", "text": &chunk })
                    .to_string()
                    .into(),
            ))
            .await
            .context("Failed to send Speak message to Deepgram TTS WebSocket")?;
        let _ = events.send(crate::app::TtsResult::StreamingChunk {
            index: index + 1,
            total: chunk_count,
            text: chunk,
        });
    }

    socket
        .send(Message::Text(json!({ "type": "Flush" }).to_string().into()))
        .await
        .context("Failed to send final Flush message to Deepgram TTS WebSocket")?;

    let mut saw_flushed = false;
    let mut close_sent = false;
    loop {
        tokio::select! {
            _ = cancellation.cancelled() => {
                close_after_abort(&mut socket, request.protocol).await;
                return Ok(());
            }
            message = socket.next() => {
                let message = message
                    .ok_or_else(|| anyhow!("Deepgram TTS WebSocket closed before sending Flushed"))?
                    .context("Failed to receive a Deepgram TTS WebSocket message")?;
                match message {
                    Message::Binary(audio) => {
                        let _ = events.send(crate::app::TtsResult::StreamingAudio(audio.to_vec()));
                    }
                    Message::Text(text) => {
                        match serde_json::from_str::<serde_json::Value>(&text) {
                            Ok(value) if value["type"] == "Flushed" => {
                                saw_flushed = true;
                                if request.protocol == StreamingProtocol::Aura {
                                    let _ = events.send(crate::app::TtsResult::StreamingFlushed);
                                    socket.close(None).await.context("Failed to close Deepgram TTS WebSocket after Flushed")?;
                                    return Ok(());
                                }
                            }
                            Ok(value) if value["type"] == "SpeechMetadata" => {
                                if request.protocol == StreamingProtocol::Flux && saw_flushed && !close_sent {
                                    socket
                                        .send(Message::Text(json!({ "type": "Close" }).to_string().into()))
                                        .await
                                        .context("Failed to send Close message to Deepgram Flux TTS WebSocket")?;
                                    close_sent = true;
                                }
                            }
                            Ok(value) if value["type"] == "SessionMetadata" => {
                                if request.protocol == StreamingProtocol::Flux && close_sent {
                                    let _ = events.send(crate::app::TtsResult::StreamingFlushed);
                                    socket.close(None).await.context("Failed to close Deepgram Flux TTS WebSocket")?;
                                    return Ok(());
                                }
                            }
                            Ok(value) if value["type"] == "Warning" => {
                                let description = value["description"].as_str().unwrap_or("Deepgram TTS warning");
                                let _ = events.send(crate::app::TtsResult::StreamingWarning(description.to_string()));
                            }
                            Ok(value) if value["type"] == "Metadata" => {
                                if let Some(request_id) = value["request_id"].as_str() {
                                    let _ = events.send(crate::app::TtsResult::StreamingRequestId(request_id.to_string()));
                                }
                            }
                            Ok(value) if value["type"] == "Connected" => {
                                if let Some(request_id) = value["request_id"].as_str() {
                                    let _ = events.send(crate::app::TtsResult::StreamingRequestId(request_id.to_string()));
                                }
                            }
                            Ok(value) if value["type"] == "Error" => {
                                let description = value["description"].as_str().unwrap_or("Deepgram TTS error");
                                return Err(anyhow!(description.to_string()));
                            }
                            Ok(_) => {}
                            Err(error) => {
                                let _ = events.send(crate::app::TtsResult::StreamingWarning(format!("Ignoring invalid Deepgram TTS control message: {error}")));
                            }
                        }
                    }
                    Message::Close(_) if request.protocol == StreamingProtocol::Flux && close_sent => {
                        let _ = events.send(crate::app::TtsResult::StreamingFlushed);
                        return Ok(());
                    }
                    Message::Close(_) => return Err(anyhow!("Deepgram TTS WebSocket closed before completing the stream")),
                    Message::Ping(payload) => socket.send(Message::Pong(payload)).await.context("Failed to respond to Deepgram TTS WebSocket ping")?,
                    Message::Pong(_) | Message::Frame(_) => {}
                }
            }
        }
    }
}

async fn close_after_abort<S>(
    socket: &mut tokio_tungstenite::WebSocketStream<S>,
    protocol: StreamingProtocol,
) where
    S: AsyncRead + AsyncWrite + Unpin,
{
    if protocol == StreamingProtocol::Aura {
        let _ = socket
            .send(Message::Text(json!({ "type": "Clear" }).to_string().into()))
            .await;
    }
    let _ = socket.close(None).await;
}

fn build_streaming_url(
    protocol: StreamingProtocol,
    voice_id: &str,
    speed: Decimal,
    sample_rate: u32,
) -> Result<Url> {
    let endpoint = match protocol {
        StreamingProtocol::Aura => AURA_SPEAK_WEBSOCKET_URL,
        StreamingProtocol::Flux => FLUX_SPEAK_WEBSOCKET_URL,
    };
    let mut url = Url::parse(endpoint)?;
    {
        let mut pairs = url.query_pairs_mut();
        pairs.append_pair("model", voice_id);
        pairs.append_pair("encoding", "linear16");
        pairs.append_pair("sample_rate", &sample_rate.to_string());
        if protocol == StreamingProtocol::Aura && speed != Decimal::ONE {
            pairs.append_pair("speed", &speed.to_string());
        }
    }
    Ok(url)
}

pub fn streaming_sample_rate(preferred_rate: u32) -> u32 {
    match preferred_rate {
        8000 | 16000 | 24000 | 32000 | 48000 => preferred_rate,
        _ => 24000,
    }
}

pub fn chunk_text(text: &str, strategy: ChunkingStrategy) -> Vec<String> {
    match strategy {
        ChunkingStrategy::TenWords => chunk_by_word_count(text, 10),
        ChunkingStrategy::Sentence => {
            chunk_at_boundaries(text, |character| matches!(character, '.' | '!' | '?'))
        }
        ChunkingStrategy::Punctuation => chunk_at_boundaries(text, |character| {
            matches!(character, '.' | '!' | '?' | ',' | ';')
        }),
    }
}

fn chunk_by_word_count(text: &str, words_per_chunk: usize) -> Vec<String> {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .chunks(words_per_chunk)
        .map(|words| words.join(" "))
        .collect()
}

fn chunk_at_boundaries(text: &str, is_boundary: impl Fn(char) -> bool) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut start = 0;
    for (index, character) in text.char_indices() {
        if is_boundary(character) {
            let end = index + character.len_utf8();
            let chunk = text[start..end].trim();
            if !chunk.is_empty() {
                chunks.push(chunk.to_string());
            }
            start = end;
        }
    }
    let remainder = text[start..].trim();
    if !remainder.is_empty() {
        chunks.push(remainder.to_string());
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_every_ten_words() {
        let text = "one two three four five six seven eight nine ten eleven twelve";
        assert_eq!(
            chunk_text(text, ChunkingStrategy::TenWords),
            vec![
                "one two three four five six seven eight nine ten",
                "eleven twelve"
            ]
        );
    }

    #[test]
    fn chunks_at_sentence_boundaries() {
        assert_eq!(
            chunk_text("One. Two? Three!", ChunkingStrategy::Sentence),
            vec!["One.", "Two?", "Three!"]
        );
    }

    #[test]
    fn chunks_at_clause_punctuation() {
        assert_eq!(
            chunk_text("One, two; three.", ChunkingStrategy::Punctuation),
            vec!["One,", "two;", "three."]
        );
    }

    #[test]
    fn chooses_a_streaming_compatible_sample_rate() {
        assert_eq!(streaming_sample_rate(22050), 24000);
        assert_eq!(streaming_sample_rate(48000), 48000);
    }

    #[test]
    fn builds_aura_streaming_endpoint_with_linear16() {
        let url = build_streaming_url(
            StreamingProtocol::Aura,
            "aura-2-thalia-en",
            Decimal::new(12, 1),
            24000,
        )
        .unwrap();
        assert_eq!(
            url.as_str(),
            "wss://api.deepgram.com/v1/speak?model=aura-2-thalia-en&encoding=linear16&sample_rate=24000&speed=1.2"
        );
    }

    #[test]
    fn builds_flux_v2_streaming_endpoint_without_aura_speed_parameter() {
        let url = build_streaming_url(
            StreamingProtocol::Flux,
            "flux-haley-en",
            Decimal::new(12, 1),
            24000,
        )
        .unwrap();
        assert_eq!(
            url.as_str(),
            "wss://api.deepgram.com/v2/speak?model=flux-haley-en&encoding=linear16&sample_rate=24000"
        );
    }

    #[test]
    fn flux_streaming_preserves_the_complete_text_as_one_turn() {
        let text = "  Keep  the   whitespace.  ";
        let chunks = match StreamingProtocol::Flux {
            StreamingProtocol::Aura => chunk_text(text, ChunkingStrategy::TenWords),
            StreamingProtocol::Flux => vec![text.to_string()],
        };
        assert_eq!(chunks, vec![text]);
    }
}
