use anyhow::{anyhow, Context, Result};
use aws_config::{BehaviorVersion, Region};
use aws_sdk_sagemakerruntime::{primitives::Blob, Client};
use rust_decimal::Decimal;
use serde_json::json;
use url::form_urlencoded;

pub async fn fetch_sagemaker_tts(
    endpoint_name: &str,
    region: &str,
    text: &str,
    voice_id: &str,
    speed: Decimal,
    sample_rate: u32,
    encoding: &str,
    normalize_volume: bool,
) -> Result<Vec<u8>> {
    let body = serde_json::to_vec(&json!({ "text": text }))
        .context("Failed to serialize SageMaker TTS request body")?;

    let sdk_config = aws_config::defaults(BehaviorVersion::latest())
        .region(Region::new(region.to_string()))
        .load()
        .await;
    let client = Client::new(&sdk_config);

    let response = client
        .invoke_endpoint()
        .endpoint_name(endpoint_name)
        .content_type("application/json")
        .accept(accept_mime_type(encoding))
        .custom_attributes(build_custom_attributes(
            voice_id,
            speed,
            sample_rate,
            encoding,
            normalize_volume,
        ))
        .body(Blob::new(body))
        .send()
        .await
        .with_context(|| {
            format!(
                "Failed to invoke SageMaker endpoint '{}' in region '{}'",
                endpoint_name, region
            )
        })?;

    response
        .body
        .map(|blob| blob.into_inner())
        .ok_or_else(|| anyhow!("SageMaker InvokeEndpoint returned an empty response body"))
}

fn build_custom_attributes(
    voice_id: &str,
    speed: Decimal,
    sample_rate: u32,
    encoding: &str,
    normalize_volume: bool,
) -> String {
    let mut serializer = form_urlencoded::Serializer::new(String::new());
    serializer.append_pair("model", voice_id);
    serializer.append_pair("encoding", encoding);
    if speed != Decimal::new(10, 1) {
        serializer.append_pair("speed", &speed.to_string());
    }
    if encoding != "mp3" && encoding != "aac" {
        serializer.append_pair("sample_rate", &sample_rate.to_string());
    }
    if normalize_volume {
        serializer.append_pair("normalize_volume", "true");
    }

    format!("v1/speak?{}", serializer.finish())
}

fn accept_mime_type(encoding: &str) -> &'static str {
    match encoding {
        "mp3" => "audio/mpeg",
        "linear16" => "audio/wav",
        "mulaw" | "alaw" => "application/octet-stream",
        "flac" => "audio/flac",
        "aac" => "audio/aac",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_attributes_include_deepgram_path_and_query() {
        let attrs = build_custom_attributes(
            "aura-2-thalia-en",
            Decimal::new(10, 1),
            24000,
            "linear16",
            false,
        );

        assert_eq!(
            attrs,
            "v1/speak?model=aura-2-thalia-en&encoding=linear16&sample_rate=24000"
        );
    }

    #[test]
    fn custom_attributes_omit_fixed_sample_rate_for_mp3() {
        let attrs =
            build_custom_attributes("aura-2-orion-en", Decimal::new(12, 1), 22050, "mp3", false);

        assert_eq!(
            attrs,
            "v1/speak?model=aura-2-orion-en&encoding=mp3&speed=1.2"
        );
    }

    #[test]
    fn custom_attributes_add_volume_normalization_when_enabled() {
        let attrs = build_custom_attributes(
            "aura-2-thalia-en",
            Decimal::new(10, 1),
            24000,
            "linear16",
            true,
        );

        assert_eq!(
            attrs,
            "v1/speak?model=aura-2-thalia-en&encoding=linear16&sample_rate=24000&normalize_volume=true"
        );
    }
}
