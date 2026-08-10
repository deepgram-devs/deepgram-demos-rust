pub const DEFAULT_REQUEST_TAGS: [&str; 3] = ["tts-tui", "appeng", "deepgram-demos-rust"];

pub fn request_tags(custom_tags: Option<&str>) -> Vec<String> {
    let mut tags = DEFAULT_REQUEST_TAGS
        .iter()
        .map(|tag| (*tag).to_string())
        .collect::<Vec<_>>();

    if let Some(custom_tags) = custom_tags {
        tags.extend(
            custom_tags
                .split(',')
                .map(str::trim)
                .filter(|tag| !tag.is_empty())
                .map(ToString::to_string),
        );
    }

    tags
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn includes_defaults_and_custom_tags() {
        assert_eq!(
            request_tags(Some("production, demo")),
            vec![
                "tts-tui",
                "appeng",
                "deepgram-demos-rust",
                "production",
                "demo"
            ]
        );
    }

    #[test]
    fn ignores_empty_custom_tags() {
        assert_eq!(
            request_tags(Some(" ,production,,")),
            vec!["tts-tui", "appeng", "deepgram-demos-rust", "production"]
        );
    }
}
