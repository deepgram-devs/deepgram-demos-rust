//! Flux TTS voice catalog.
//!
//! Keep this table aligned with <https://developers.deepgram.com/docs/flux-tts/voices>.
//! Flux model IDs have the form `flux-{voice}-{language code}`. Languages other than
//! those in [`GENERALLY_AVAILABLE_LANGUAGE_CODES`] are Early Access.

use crate::app::{Gender, Voice};

use Gender::{Female as F, Male as M};

/// Language codes for which Flux voices are generally available. Every other
/// language in the catalog is Early Access and has no text normalization yet,
/// so numbers, dates, and abbreviations should be written out as words.
pub const GENERALLY_AVAILABLE_LANGUAGE_CODES: [&str; 10] =
    ["en", "nl", "fr", "de", "hi", "it", "ja", "pt", "ru", "es"];

/// Model label for generally available Flux voices.
pub const FLUX_MODEL: &str = "flux";
/// Model label for Early Access Flux voices; contains `flux` so the `flux`
/// filter still matches and `early` matches only Early Access voices.
pub const FLUX_EARLY_ACCESS_MODEL: &str = "flux early access";

/// One language (or regional variant) and its voices as `(voice name, gender)`.
struct FluxLanguage {
    /// ISO language code used as the model ID suffix.
    code: &'static str,
    /// Label shown as the language header in the Voices panel.
    language: &'static str,
    voices: &'static [(&'static str, Gender)],
}

#[rustfmt::skip]
const FLUX_LANGUAGES: &[FluxLanguage] = &[
    FluxLanguage { code: "en", language: "English", voices: &[("hannah", F), ("kit", M), ("alexis", F), ("cliff", M), ("sienna", F), ("cole", M), ("brooke", F), ("colin", M), ("gemma", F), ("haley", F), ("heather", F), ("miles", M), ("sean", M), ("bree", F), ("brittany", F), ("bruce", M), ("conor", M), ("donovan", M), ("drew", M), ("elise", F), ("jack", M), ("kai", M), ("kelsey", F), ("maeve", F), ("marcelo", M), ("marcus", M), ("meena", F), ("meghan", F), ("naveen", M), ("paige", F), ("priya", F), ("rufus", M), ("sharon", F), ("tanner", M), ("wade", M), ("wes", M)] },
    FluxLanguage { code: "nl", language: "Dutch", voices: &[("anouk", F), ("bram", M), ("fleur", F), ("irene", F), ("luuk", M), ("monique", F), ("ruben", M), ("sanne", F), ("yvonne", F)] },
    FluxLanguage { code: "nl", language: "Dutch (Flemish)", voices: &[("dirk", M), ("lotte", F), ("niels", M), ("veerle", F)] },
    FluxLanguage { code: "fr", language: "French", voices: &[("arnaud", M), ("benoit", M), ("brigitte", F), ("camille", F), ("chloe", F), ("florian", M), ("julien", M), ("mathieu", M), ("pierre", M), ("thierry", M), ("veronique", F), ("yasmine", F)] },
    FluxLanguage { code: "de", language: "German", voices: &[("benjamin", M), ("christian", M), ("janine", F), ("magdalena", F), ("moritz", M), ("nadine", F), ("rolf", M), ("sonja", F), ("sven", M), ("tobias", M), ("verena", F)] },
    FluxLanguage { code: "de", language: "German (Swiss)", voices: &[("leonie", F), ("yannick", M)] },
    FluxLanguage { code: "hi", language: "Hindi", voices: &[("ajay", M), ("anjali", F), ("arjun", M), ("divya", F), ("jyoti", F), ("nikhil", M), ("pallavi", F), ("prachi", F), ("rahul", M), ("rajesh", M), ("ritu", F), ("shreya", F), ("sunita", F), ("vikram", M)] },
    FluxLanguage { code: "it", language: "Italian", voices: &[("angela", F), ("chiara", F), ("federico", M), ("giorgio", M), ("matteo", M), ("paola", F), ("rocco", M), ("sofia", F), ("stefano", M), ("vincenzo", M), ("vito", M)] },
    FluxLanguage { code: "ja", language: "Japanese", voices: &[("ayumi", F), ("hayato", M), ("nozomi", F), ("sayaka", F)] },
    FluxLanguage { code: "pt", language: "Portuguese (Brazil)", voices: &[("bianca", F), ("bruno", M), ("claudia", F), ("leandro", M), ("marcia", F), ("mariana", F), ("marta", F), ("mateus", M), ("miguel", M), ("renato", M), ("roberto", M), ("rodrigo", M)] },
    FluxLanguage { code: "pt", language: "Portuguese (Portugal)", voices: &[("artur", M), ("benedita", F), ("catarina", F), ("hugo", M), ("pedro", M), ("ricardo", M)] },
    FluxLanguage { code: "ru", language: "Russian", voices: &[("andrei", M), ("kirill", M), ("maxim", M), ("mikhail", M), ("natalia", F), ("nikolai", M), ("olga", F), ("polina", F), ("vladimir", M)] },
    FluxLanguage { code: "es", language: "Spanish", voices: &[("aina", F), ("alfredo", M), ("carmen", F), ("cecilia", F), ("duvan", M), ("eduardo", M), ("ernesto", M), ("fernanda", F), ("fiorella", F), ("gustavo", M), ("jaime", M), ("martin", M), ("mauricio", M), ("patricia", F), ("pilar", F), ("rafael", M), ("tomas", M), ("valentina", F), ("ysabel", F)] },
    FluxLanguage { code: "bn", language: "Bengali", voices: &[("afsana", F), ("ishrat", F), ("mamun", M), ("sabbir", M), ("shuvo", M), ("sumaiya", F)] },
    FluxLanguage { code: "bg", language: "Bulgarian", voices: &[("desislava", F), ("gergana", F)] },
    FluxLanguage { code: "ca", language: "Catalan", voices: &[("jordi", M), ("judit", F)] },
    FluxLanguage { code: "zh", language: "Chinese (Mandarin)", voices: &[("bo", M), ("haoran", M), ("jian", M), ("jie", M), ("lijuan", F), ("shufen", F), ("wenbo", M), ("wenjing", F), ("zhen", F)] },
    FluxLanguage { code: "zh", language: "Chinese (Taiwanese Mandarin)", voices: &[("chiaying", F), ("kuanyu", M), ("yawen", F), ("yuting", F)] },
    FluxLanguage { code: "zh", language: "Chinese (Cantonese)", voices: &[("manlok", M)] },
    FluxLanguage { code: "hr", language: "Croatian", voices: &[("dalibor", M), ("danijela", F), ("domagoj", M), ("jasna", F), ("josipa", F), ("mihaela", F), ("vedran", M)] },
    FluxLanguage { code: "cs", language: "Czech", voices: &[("jakub", M)] },
    FluxLanguage { code: "da", language: "Danish", voices: &[("pernille", F), ("rasmus", M), ("rikke", M)] },
    FluxLanguage { code: "et", language: "Estonian", voices: &[("triin", F)] },
    FluxLanguage { code: "fi", language: "Finnish", voices: &[("kirsi", F), ("teemu", M)] },
    FluxLanguage { code: "el", language: "Greek", voices: &[("kostas", M)] },
    FluxLanguage { code: "gu", language: "Gujarati", voices: &[("bhavna", F), ("hetal", F), ("jignesh", M)] },
    FluxLanguage { code: "hu", language: "Hungarian", voices: &[("bence", M), ("eszter", F), ("kinga", F), ("tibor", M)] },
    FluxLanguage { code: "id", language: "Indonesian", voices: &[("dewi", F), ("fajar", M), ("fitri", F), ("rizky", M), ("teguh", M)] },
    FluxLanguage { code: "kn", language: "Kannada", voices: &[("manjunath", M)] },
    FluxLanguage { code: "ko", language: "Korean", voices: &[("eunji", F), ("hyunwoo", M), ("mikyung", F), ("minjun", M), ("soojin", F), ("sungmin", M)] },
    FluxLanguage { code: "lv", language: "Latvian", voices: &[("agnese", F), ("edgars", M)] },
    FluxLanguage { code: "lt", language: "Lithuanian", voices: &[("goda", F), ("neringa", F)] },
    FluxLanguage { code: "ms", language: "Malay", voices: &[("haziq", M), ("izzah", F)] },
    FluxLanguage { code: "mr", language: "Marathi", voices: &[("pooja", F), ("pratik", M), ("sneha", F)] },
    FluxLanguage { code: "no", language: "Norwegian", voices: &[("erlend", M), ("espen", M), ("ida", F), ("signe", F), ("silje", F), ("vegard", M)] },
    FluxLanguage { code: "pl", language: "Polish", voices: &[("bartosz", M), ("ewa", F), ("justyna", F), ("kacper", M), ("konrad", M), ("zuzanna", F)] },
    FluxLanguage { code: "ro", language: "Romanian", voices: &[("cosmin", M), ("ioana", F)] },
    FluxLanguage { code: "sk", language: "Slovak", voices: &[("dominika", F), ("juraj", M)] },
    FluxLanguage { code: "sl", language: "Slovenian", voices: &[("gregor", M), ("katja", F)] },
    FluxLanguage { code: "sv", language: "Swedish", voices: &[("axel", M), ("charlotta", F), ("frida", F), ("ludvig", M), ("oskar", M), ("pontus", M), ("wilma", F)] },
    FluxLanguage { code: "tl", language: "Tagalog", voices: &[("jerome", M), ("jomar", M), ("liezel", F), ("noel", M), ("trisha", F)] },
    FluxLanguage { code: "ta", language: "Tamil", voices: &[("gokul", M), ("kavya", F), ("sowmya", F), ("vinoth", M)] },
    FluxLanguage { code: "te", language: "Telugu", voices: &[("pavan", M), ("ravi", M), ("sravani", F)] },
    FluxLanguage { code: "tr", language: "Turkish", voices: &[("dilek", F), ("ecem", F), ("emre", M), ("gizem", F), ("mehmet", M), ("serkan", M), ("zeynep", F)] },
    FluxLanguage { code: "uk", language: "Ukrainian", voices: &[("denys", M), ("solomiya", F)] },
    FluxLanguage { code: "ur", language: "Urdu", voices: &[("ambreen", F), ("usman", M)] },
    FluxLanguage { code: "vi", language: "Vietnamese", voices: &[("hoa", F), ("minh", M)] },
];

fn capitalize(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Returns `true` when `voice_id` is a Flux voice in an Early Access language.
pub fn is_early_access_voice(voice_id: &str) -> bool {
    voice_id
        .strip_prefix("flux-")
        .and_then(|rest| rest.rsplit('-').next())
        .is_some_and(|code| !GENERALLY_AVAILABLE_LANGUAGE_CODES.contains(&code))
}

/// Builds every Flux voice in documentation order.
pub fn flux_voices() -> Vec<Voice> {
    FLUX_LANGUAGES
        .iter()
        .flat_map(|language| {
            let early_access = !GENERALLY_AVAILABLE_LANGUAGE_CODES.contains(&language.code);
            language.voices.iter().map(move |(name, gender)| Voice {
                id: format!("flux-{}-{}", name, language.code),
                name: if early_access {
                    format!("{} (flux EA)", capitalize(name))
                } else {
                    format!("{} (flux)", capitalize(name))
                },
                vendor: "Deepgram".to_string(),
                model: if early_access {
                    FLUX_EARLY_ACCESS_MODEL.to_string()
                } else {
                    FLUX_MODEL.to_string()
                },
                language: language.language.to_string(),
                gender: gender.clone(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn catalog_contains_all_documented_voices() {
        let voices = flux_voices();
        assert_eq!(voices.len(), 266);
        let english = voices.iter().filter(|v| v.id.ends_with("-en")).count();
        assert_eq!(english, 36);
        let ids: HashSet<_> = voices.iter().map(|v| v.id.as_str()).collect();
        assert_eq!(ids.len(), voices.len(), "voice IDs must be unique");
    }

    #[test]
    fn voice_ids_follow_flux_naming() {
        for voice in flux_voices() {
            let parts: Vec<_> = voice.id.split('-').collect();
            assert_eq!(parts.len(), 3, "unexpected id {}", voice.id);
            assert_eq!(parts[0], "flux");
            assert_eq!(parts[2].len(), 2, "unexpected language code in {}", voice.id);
            assert!(voice.id.chars().all(|c| c.is_ascii_lowercase() || c == '-'));
        }
    }

    #[test]
    fn early_access_flag_follows_language() {
        assert!(!is_early_access_voice("flux-hannah-en"));
        assert!(!is_early_access_voice("flux-leonie-de"));
        assert!(!is_early_access_voice("flux-dirk-nl"));
        assert!(is_early_access_voice("flux-afsana-bn"));
        assert!(is_early_access_voice("flux-manlok-zh"));
        assert!(!is_early_access_voice("aura-2-thalia-en"));
    }

    #[test]
    fn early_access_voices_are_labelled() {
        let voices = flux_voices();
        let afsana = voices.iter().find(|v| v.id == "flux-afsana-bn").unwrap();
        assert_eq!(afsana.name, "Afsana (flux EA)");
        assert_eq!(afsana.model, FLUX_EARLY_ACCESS_MODEL);
        assert_eq!(afsana.language, "Bengali");
        let hannah = voices.iter().find(|v| v.id == "flux-hannah-en").unwrap();
        assert_eq!(hannah.name, "Hannah (flux)");
        assert_eq!(hannah.model, FLUX_MODEL);
        assert_eq!(hannah.language, "English");
        assert_eq!(hannah.gender, Gender::Female);
        let early = voices.iter().filter(|v| is_early_access_voice(&v.id)).count();
        // 149 generally available voices (en, nl, fr, de, hi, it, ja, pt, ru, es).
        assert_eq!(early, 266 - 149);
    }
}
