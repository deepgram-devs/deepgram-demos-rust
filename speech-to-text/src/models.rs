use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    #[serde(default)]
    stt: Vec<SttModel>,
}

#[derive(Debug, Deserialize)]
struct SttModel {
    #[serde(default)]
    canonical_name: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    uuid: String,
    #[serde(default)]
    languages: Vec<String>,
    #[serde(default)]
    batch: bool,
    #[serde(default)]
    streaming: bool,
}

#[derive(Debug, PartialEq, Eq)]
struct ModelRow {
    canonical_name: String,
    mode: String,
    version: String,
    uuid: String,
    languages: String,
}

pub(crate) async fn run_list_models(
    api_key: Option<String>,
    endpoint: Option<String>,
    include_outdated: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let base_url = endpoint.unwrap_or_else(|| "https://api.deepgram.com".to_string());
    let mut url = format!("{}/v1/models", base_url.trim_end_matches('/'));
    if include_outdated {
        url.push_str("?include_outdated=true");
    }

    let client = reqwest::Client::new();
    let mut request = client.get(url);
    if let Some(api_key) = api_key {
        request = request.header("Authorization", format!("Token {api_key}"));
    }

    let response = request.send().await?.error_for_status()?;
    let response = response.json::<ModelsResponse>().await?;
    let rows: Vec<ModelRow> = response.stt.into_iter().map(model_row).collect();

    if rows.is_empty() {
        println!("No speech-to-text models found.");
    } else {
        print_table(&rows);
    }

    Ok(())
}

fn model_row(model: SttModel) -> ModelRow {
    let mode = match (model.batch, model.streaming) {
        (true, true) => "batch, streaming",
        (true, false) => "batch",
        (false, true) => "streaming",
        (false, false) => "-",
    };

    ModelRow {
        canonical_name: model.canonical_name,
        mode: mode.to_string(),
        version: model.version,
        uuid: model.uuid,
        languages: model
            .languages
            .into_iter()
            .take(3)
            .collect::<Vec<_>>()
            .join(", "),
    }
}

fn print_table(rows: &[ModelRow]) {
    let headers = ["Canonical Name", "Mode", "Version", "UUID", "Languages"];
    let values: Vec<[&str; 5]> = rows
        .iter()
        .map(|row| {
            [
                row.canonical_name.as_str(),
                row.mode.as_str(),
                row.version.as_str(),
                row.uuid.as_str(),
                row.languages.as_str(),
            ]
        })
        .collect();

    let widths: [usize; 5] = std::array::from_fn(|index| {
        std::iter::once(headers[index].len())
            .chain(values.iter().map(|row| row[index].len()))
            .max()
            .unwrap_or(0)
    });

    print_separator(&widths);
    print_row(&headers, &widths);
    print_separator(&widths);
    for row in values {
        print_row(&row, &widths);
    }
    print_separator(&widths);
}

fn print_separator(widths: &[usize; 5]) {
    println!(
        "+-{}-+-{}-+-{}-+-{}-+-{}-+",
        "-".repeat(widths[0]),
        "-".repeat(widths[1]),
        "-".repeat(widths[2]),
        "-".repeat(widths[3]),
        "-".repeat(widths[4])
    );
}

fn print_row(values: &[&str; 5], widths: &[usize; 5]) {
    println!(
        "| {:width0$} | {:width1$} | {:width2$} | {:width3$} | {:width4$} |",
        values[0],
        values[1],
        values[2],
        values[3],
        values[4],
        width0 = widths[0],
        width1 = widths[1],
        width2 = widths[2],
        width3 = widths[3],
        width4 = widths[4],
    );
}

#[cfg(test)]
mod tests {
    use super::{ModelsResponse, SttModel, model_row};

    #[test]
    fn parses_stt_model_metadata() {
        let response: ModelsResponse = serde_json::from_str(
            r#"{"stt":[{"canonical_name":"nova-3-en","version":"2025-01-01.0","uuid":"model-uuid","languages":["en","es"],"batch":true,"streaming":true}]}"#,
        )
        .unwrap();

        assert_eq!(
            model_row(response.stt.into_iter().next().unwrap()),
            super::ModelRow {
                canonical_name: "nova-3-en".to_string(),
                mode: "batch, streaming".to_string(),
                version: "2025-01-01.0".to_string(),
                uuid: "model-uuid".to_string(),
                languages: "en, es".to_string(),
            }
        );
    }

    #[test]
    fn missing_optional_model_fields_use_empty_values() {
        let model: SttModel = serde_json::from_str(r#"{}"#).unwrap();
        let row = model_row(model);

        assert_eq!(row.version, "");
        assert_eq!(row.canonical_name, "");
        assert_eq!(row.uuid, "");
        assert_eq!(row.languages, "");
        assert_eq!(row.mode, "-");
    }

    #[test]
    fn limits_displayed_languages_to_three() {
        let model: SttModel =
            serde_json::from_str(r#"{"languages":["en","es","fr","de"]}"#).unwrap();

        assert_eq!(model_row(model).languages, "en, es, fr");
    }
}
