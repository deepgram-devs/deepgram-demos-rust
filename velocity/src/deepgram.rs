pub const DEFAULT_MODEL: &str = "nova-3";
pub const DO_NOT_SPECIFY_LANGUAGE_LABEL: &str = "Do not specify";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LanguageOption {
    pub code: &'static str,
    pub label: &'static str,
}

const SUPPORTED_MODELS: [&str; 2] = ["nova-2", "nova-3"];

const NOVA_2_LANGUAGES: [LanguageOption; 54] = [
    LanguageOption { code: "multi", label: "Multilingual (English + Spanish)" },
    LanguageOption { code: "bg", label: "Bulgarian" },
    LanguageOption { code: "ca", label: "Catalan" },
    LanguageOption { code: "zh", label: "Chinese (Mandarin, Simplified)" },
    LanguageOption { code: "zh-CN", label: "Chinese (Mandarin, Simplified, China)" },
    LanguageOption { code: "zh-Hans", label: "Chinese (Mandarin, Simplified Script)" },
    LanguageOption { code: "zh-TW", label: "Chinese (Mandarin, Traditional, Taiwan)" },
    LanguageOption { code: "zh-Hant", label: "Chinese (Mandarin, Traditional Script)" },
    LanguageOption { code: "zh-HK", label: "Chinese (Cantonese, Traditional, Hong Kong)" },
    LanguageOption { code: "cs", label: "Czech" },
    LanguageOption { code: "da", label: "Danish" },
    LanguageOption { code: "da-DK", label: "Danish (Denmark)" },
    LanguageOption { code: "nl", label: "Dutch" },
    LanguageOption { code: "en", label: "English" },
    LanguageOption { code: "en-US", label: "English (United States)" },
    LanguageOption { code: "en-AU", label: "English (Australia)" },
    LanguageOption { code: "en-GB", label: "English (United Kingdom)" },
    LanguageOption { code: "en-NZ", label: "English (New Zealand)" },
    LanguageOption { code: "en-IN", label: "English (India)" },
    LanguageOption { code: "et", label: "Estonian" },
    LanguageOption { code: "fi", label: "Finnish" },
    LanguageOption { code: "nl-BE", label: "Flemish" },
    LanguageOption { code: "fr", label: "French" },
    LanguageOption { code: "fr-CA", label: "French (Canada)" },
    LanguageOption { code: "de", label: "German" },
    LanguageOption { code: "de-CH", label: "German (Switzerland)" },
    LanguageOption { code: "el", label: "Greek" },
    LanguageOption { code: "hi", label: "Hindi" },
    LanguageOption { code: "hu", label: "Hungarian" },
    LanguageOption { code: "id", label: "Indonesian" },
    LanguageOption { code: "it", label: "Italian" },
    LanguageOption { code: "ja", label: "Japanese" },
    LanguageOption { code: "ko", label: "Korean" },
    LanguageOption { code: "ko-KR", label: "Korean (South Korea)" },
    LanguageOption { code: "lv", label: "Latvian" },
    LanguageOption { code: "lt", label: "Lithuanian" },
    LanguageOption { code: "ms", label: "Malay" },
    LanguageOption { code: "no", label: "Norwegian" },
    LanguageOption { code: "pl", label: "Polish" },
    LanguageOption { code: "pt", label: "Portuguese" },
    LanguageOption { code: "pt-BR", label: "Portuguese (Brazil)" },
    LanguageOption { code: "pt-PT", label: "Portuguese (Portugal)" },
    LanguageOption { code: "ro", label: "Romanian" },
    LanguageOption { code: "ru", label: "Russian" },
    LanguageOption { code: "sk", label: "Slovak" },
    LanguageOption { code: "es", label: "Spanish" },
    LanguageOption { code: "es-419", label: "Spanish (Latin America)" },
    LanguageOption { code: "sv", label: "Swedish" },
    LanguageOption { code: "sv-SE", label: "Swedish (Sweden)" },
    LanguageOption { code: "th", label: "Thai" },
    LanguageOption { code: "th-TH", label: "Thai (Thailand)" },
    LanguageOption { code: "tr", label: "Turkish" },
    LanguageOption { code: "uk", label: "Ukrainian" },
    LanguageOption { code: "vi", label: "Vietnamese" },
];

const NOVA_3_LANGUAGES: [LanguageOption; 78] = [
    LanguageOption { code: "multi", label: "Multilingual" },
    LanguageOption { code: "ar", label: "Arabic" },
    LanguageOption { code: "ar-AE", label: "Arabic (United Arab Emirates)" },
    LanguageOption { code: "ar-SA", label: "Arabic (Saudi Arabia)" },
    LanguageOption { code: "ar-QA", label: "Arabic (Qatar)" },
    LanguageOption { code: "ar-KW", label: "Arabic (Kuwait)" },
    LanguageOption { code: "ar-SY", label: "Arabic (Syria)" },
    LanguageOption { code: "ar-LB", label: "Arabic (Lebanon)" },
    LanguageOption { code: "ar-PS", label: "Arabic (Palestine)" },
    LanguageOption { code: "ar-JO", label: "Arabic (Jordan)" },
    LanguageOption { code: "ar-EG", label: "Arabic (Egypt)" },
    LanguageOption { code: "ar-SD", label: "Arabic (Sudan)" },
    LanguageOption { code: "ar-TD", label: "Arabic (Chad)" },
    LanguageOption { code: "ar-MA", label: "Arabic (Morocco)" },
    LanguageOption { code: "ar-DZ", label: "Arabic (Algeria)" },
    LanguageOption { code: "ar-TN", label: "Arabic (Tunisia)" },
    LanguageOption { code: "ar-IQ", label: "Arabic (Iraq)" },
    LanguageOption { code: "ar-IR", label: "Arabic (Iran)" },
    LanguageOption { code: "be", label: "Belarusian" },
    LanguageOption { code: "bn", label: "Bengali" },
    LanguageOption { code: "bs", label: "Bosnian" },
    LanguageOption { code: "bg", label: "Bulgarian" },
    LanguageOption { code: "ca", label: "Catalan" },
    LanguageOption { code: "hr", label: "Croatian" },
    LanguageOption { code: "cs", label: "Czech" },
    LanguageOption { code: "da", label: "Danish" },
    LanguageOption { code: "da-DK", label: "Danish (Denmark)" },
    LanguageOption { code: "nl", label: "Dutch" },
    LanguageOption { code: "en", label: "English" },
    LanguageOption { code: "en-US", label: "English (United States)" },
    LanguageOption { code: "en-AU", label: "English (Australia)" },
    LanguageOption { code: "en-GB", label: "English (United Kingdom)" },
    LanguageOption { code: "en-IN", label: "English (India)" },
    LanguageOption { code: "en-NZ", label: "English (New Zealand)" },
    LanguageOption { code: "et", label: "Estonian" },
    LanguageOption { code: "fi", label: "Finnish" },
    LanguageOption { code: "nl-BE", label: "Flemish" },
    LanguageOption { code: "fr", label: "French" },
    LanguageOption { code: "fr-CA", label: "French (Canada)" },
    LanguageOption { code: "de", label: "German" },
    LanguageOption { code: "de-CH", label: "German (Switzerland)" },
    LanguageOption { code: "el", label: "Greek" },
    LanguageOption { code: "he", label: "Hebrew" },
    LanguageOption { code: "hi", label: "Hindi" },
    LanguageOption { code: "hu", label: "Hungarian" },
    LanguageOption { code: "id", label: "Indonesian" },
    LanguageOption { code: "it", label: "Italian" },
    LanguageOption { code: "ja", label: "Japanese" },
    LanguageOption { code: "kn", label: "Kannada" },
    LanguageOption { code: "ko", label: "Korean" },
    LanguageOption { code: "ko-KR", label: "Korean (South Korea)" },
    LanguageOption { code: "lv", label: "Latvian" },
    LanguageOption { code: "lt", label: "Lithuanian" },
    LanguageOption { code: "mk", label: "Macedonian" },
    LanguageOption { code: "ms", label: "Malay" },
    LanguageOption { code: "mr", label: "Marathi" },
    LanguageOption { code: "no", label: "Norwegian" },
    LanguageOption { code: "fa", label: "Persian" },
    LanguageOption { code: "pl", label: "Polish" },
    LanguageOption { code: "pt", label: "Portuguese" },
    LanguageOption { code: "pt-BR", label: "Portuguese (Brazil)" },
    LanguageOption { code: "pt-PT", label: "Portuguese (Portugal)" },
    LanguageOption { code: "ro", label: "Romanian" },
    LanguageOption { code: "ru", label: "Russian" },
    LanguageOption { code: "sr", label: "Serbian" },
    LanguageOption { code: "sk", label: "Slovak" },
    LanguageOption { code: "sl", label: "Slovenian" },
    LanguageOption { code: "es", label: "Spanish" },
    LanguageOption { code: "es-419", label: "Spanish (Latin America)" },
    LanguageOption { code: "sv", label: "Swedish" },
    LanguageOption { code: "sv-SE", label: "Swedish (Sweden)" },
    LanguageOption { code: "tl", label: "Tagalog" },
    LanguageOption { code: "ta", label: "Tamil" },
    LanguageOption { code: "te", label: "Telugu" },
    LanguageOption { code: "tr", label: "Turkish" },
    LanguageOption { code: "uk", label: "Ukrainian" },
    LanguageOption { code: "ur", label: "Urdu" },
    LanguageOption { code: "vi", label: "Vietnamese" },
];

pub fn supported_models() -> &'static [&'static str] {
    &SUPPORTED_MODELS
}

pub fn normalize_model(model: &str) -> Option<&'static str> {
    let trimmed = model.trim();
    supported_models()
        .iter()
        .copied()
        .find(|candidate| candidate.eq_ignore_ascii_case(trimmed))
}

pub fn languages_for_model(model: &str) -> &'static [LanguageOption] {
    match normalize_model(model).unwrap_or(DEFAULT_MODEL) {
        "nova-2" => &NOVA_2_LANGUAGES,
        _ => &NOVA_3_LANGUAGES,
    }
}

pub fn normalize_language(model: &str, language: Option<&str>) -> Result<Option<String>, String> {
    let Some(trimmed) = language.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };

    languages_for_model(model)
        .iter()
        .find(|option| option.code.eq_ignore_ascii_case(trimmed))
        .map(|option| Some(option.code.to_string()))
        .ok_or_else(|| format!("Language '{trimmed}' is not supported by model {model}"))
}

pub fn language_display(option: &LanguageOption) -> String {
    format!("{} ({})", option.label, option.code)
}

pub fn language_code_from_display(model: &str, display: &str) -> Option<String> {
    let trimmed = display.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case(DO_NOT_SPECIFY_LANGUAGE_LABEL) {
        return None;
    }

    if let Some(start) = trimmed.rfind('(') {
        if trimmed.ends_with(')') && start + 1 < trimmed.len() - 1 {
            let code = &trimmed[start + 1..trimmed.len() - 1];
            return normalize_language(model, Some(code)).ok().flatten();
        }
    }

    normalize_language(model, Some(trimmed)).ok().flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_supported_model_names() {
        assert_eq!(normalize_model("NOVA-2"), Some("nova-2"));
        assert_eq!(normalize_model("nova-3"), Some("nova-3"));
        assert_eq!(normalize_model("flux"), None);
    }

    #[test]
    fn rejects_language_not_supported_by_model() {
        assert!(normalize_language("nova-2", Some("ar")).is_err());
        assert_eq!(
            normalize_language("nova-3", Some("ar")).unwrap(),
            Some("ar".to_string())
        );
    }

    #[test]
    fn parses_display_text_into_language_code() {
        let display = language_display(&LanguageOption {
            code: "en-US",
            label: "English (United States)",
        });

        assert_eq!(
            language_code_from_display("nova-2", &display),
            Some("en-US".to_string())
        );
        assert_eq!(language_code_from_display("nova-2", DO_NOT_SPECIFY_LANGUAGE_LABEL), None);
    }
}
