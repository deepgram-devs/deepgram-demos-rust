//! Provider-neutral LLM normalization for transcripts.

use crate::config::LlmProvider;
use crate::logger;
use serde_json::{Value, json};

const NORMALIZATION_PROMPT: &str = "Normalize the user's dictated message for readability. Preserve its meaning, intent, tone, and all important details. Fix obvious transcription errors, punctuation, capitalization, and spacing. Return only the normalized message, with no commentary or quotation marks.";

pub fn detect_provider(api_key: &str) -> Option<LlmProvider> {
    let key = api_key.trim();
    if key.starts_with("sk-ant-") {
        Some(LlmProvider::Anthropic)
    } else if key.starts_with("AIza") {
        Some(LlmProvider::Gemini)
    } else if key.starts_with("tgp_") {
        Some(LlmProvider::Together)
    } else if key.starts_with("sk-") {
        Some(LlmProvider::OpenAi)
    } else {
        None
    }
}

pub fn effective_provider(provider: LlmProvider, api_key: &str) -> Result<LlmProvider, String> {
    match provider {
        LlmProvider::Auto => detect_provider(api_key)
            .ok_or_else(|| "Unable to identify the LLM provider from this API key".to_string()),
        selected => Ok(selected),
    }
}

pub fn list_models(provider: LlmProvider, api_key: &str) -> Result<Vec<String>, String> {
    let provider = effective_provider(provider, api_key)?;
    let client = reqwest::blocking::Client::new();
    let response = match provider {
        LlmProvider::OpenAi | LlmProvider::Together => {
            let base = if provider == LlmProvider::OpenAi {
                "https://api.openai.com/v1/models"
            } else {
                "https://api.together.xyz/v1/models"
            };
            client.get(base).bearer_auth(api_key).send()
        }
        LlmProvider::Gemini => client
            .get("https://generativelanguage.googleapis.com/v1beta/models")
            .query(&[("key", api_key)])
            .send(),
        LlmProvider::Anthropic => client
            .get("https://api.anthropic.com/v1/models")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .send(),
        LlmProvider::Auto => return Err("Auto-detect did not resolve an LLM provider".to_string()),
    };

    let body = read_json(response, "model discovery")?;
    let mut models = body
        .get("data")
        .or_else(|| body.get("models"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|model| model.get("id").or_else(|| model.get("name")))
        .filter_map(Value::as_str)
        .map(|name| name.strip_prefix("models/").unwrap_or(name).to_string())
        .filter(|name| !name.is_empty())
        .collect::<Vec<_>>();
    models.sort_unstable();
    models.dedup();
    if models.is_empty() {
        return Err("The provider returned no usable text models".to_string());
    }
    Ok(models)
}

pub fn normalize(
    provider: LlmProvider,
    api_key: &str,
    model: &str,
    transcript: &str,
) -> Result<String, String> {
    let provider = effective_provider(provider, api_key)?;
    let model = model.trim();
    if model.is_empty() {
        return Err("Select an LLM model before enabling normalization".to_string());
    }

    let client = reqwest::blocking::Client::new();
    let response = match provider {
        LlmProvider::OpenAi | LlmProvider::Together => {
            let url = if provider == LlmProvider::OpenAi {
                "https://api.openai.com/v1/chat/completions"
            } else {
                "https://api.together.xyz/v1/chat/completions"
            };
            client
                .post(url)
                .bearer_auth(api_key)
                .json(&json!({
                    "model": model,
                    "temperature": 0,
                    "messages": [
                        {"role": "system", "content": NORMALIZATION_PROMPT},
                        {"role": "user", "content": transcript}
                    ]
                }))
                .send()
        }
        LlmProvider::Gemini => client
            .post(format!(
                "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent"
            ))
            .query(&[("key", api_key)])
            .json(&json!({
                "systemInstruction": {"parts": [{"text": NORMALIZATION_PROMPT}]},
                "contents": [{"parts": [{"text": transcript}]}],
                "generationConfig": {"temperature": 0}
            }))
            .send(),
        LlmProvider::Anthropic => client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&json!({
                "model": model,
                "max_tokens": 1024,
                "temperature": 0,
                "system": NORMALIZATION_PROMPT,
                "messages": [{"role": "user", "content": transcript}]
            }))
            .send(),
        LlmProvider::Auto => return Err("Auto-detect did not resolve an LLM provider".to_string()),
    };

    let body = read_json(response, "transcript normalization")?;
    let text = match provider {
        LlmProvider::Gemini => body["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .map(str::to_string),
        LlmProvider::Anthropic => body["content"][0]["text"].as_str().map(str::to_string),
        LlmProvider::OpenAi | LlmProvider::Together => body["choices"][0]["message"]["content"]
            .as_str()
            .map(str::to_string),
        LlmProvider::Auto => None,
    };
    text.map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "The LLM returned an empty normalization".to_string())
}

fn read_json(
    response: Result<reqwest::blocking::Response, reqwest::Error>,
    context: &str,
) -> Result<Value, String> {
    let response = response.map_err(|error| format!("{context} request failed: {error}"))?;
    let status = response.status();
    let text = response
        .text()
        .map_err(|error| format!("Failed to read {context} response: {error}"))?;
    if !status.is_success() {
        logger::verbose(&format!("LLM {context} HTTP {status}: {text}"));
        return Err(format!("LLM {context} failed ({status})"));
    }
    serde_json::from_str(&text).map_err(|error| format!("Invalid {context} response: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_supported_key_prefixes() {
        assert_eq!(detect_provider("sk-ant-test"), Some(LlmProvider::Anthropic));
        assert_eq!(detect_provider("AIza-test"), Some(LlmProvider::Gemini));
        assert_eq!(detect_provider("tgp_test"), Some(LlmProvider::Together));
        assert_eq!(detect_provider("sk-test"), Some(LlmProvider::OpenAi));
    }
}
