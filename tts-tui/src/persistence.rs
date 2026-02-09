use anyhow::Result;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
struct SavedData {
    texts: Vec<String>,
}

fn get_data_file_path() -> Result<PathBuf> {
    if let Some(proj_dirs) = ProjectDirs::from("com", "deepgram", "tts-tui") {
        let data_dir = proj_dirs.data_dir();
        fs::create_dir_all(data_dir)?;
        Ok(data_dir.join("saved_texts.json"))
    } else {
        Err(anyhow::anyhow!("Could not determine project directories"))
    }
}

fn get_default_texts() -> Vec<String> {
    vec![
        "Hello, this is a test of the Deepgram Text-to-Speech API.".to_string(),
        "The quick brown fox jumps over the lazy dog.".to_string(),
        "Rust is a systems programming language that focuses on safety, speed, and concurrency.".to_string(),
        "Gemini is a family of multimodal models developed by Google AI.".to_string(),
        "This is a longer text to demonstrate scrolling and playback features.".to_string(),
        "Another example sentence for testing purposes.".to_string(),
        "One more for good measure.".to_string(),
    ]
}

pub fn load_saved_texts() -> Vec<String> {
    match get_data_file_path() {
        Ok(path) => {
            if path.exists() {
                match fs::read_to_string(&path) {
                    Ok(contents) => {
                        match serde_json::from_str::<SavedData>(&contents) {
                            Ok(data) => {
                                // Return loaded texts, or defaults if empty
                                if data.texts.is_empty() {
                                    get_default_texts()
                                } else {
                                    data.texts
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to parse saved texts (using defaults): {}", e);
                                // Backup corrupted file
                                let backup_path = path.with_extension("json.backup");
                                let _ = fs::copy(&path, backup_path);
                                get_default_texts()
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to read saved texts file (using defaults): {}", e);
                        get_default_texts()
                    }
                }
            } else {
                // First run - use defaults and save them
                let defaults = get_default_texts();
                let _ = save_saved_texts(&defaults); // Ignore error on first save
                defaults
            }
        }
        Err(e) => {
            eprintln!("Failed to get data directory (using defaults): {}", e);
            get_default_texts()
        }
    }
}

pub fn save_saved_texts(texts: &[String]) -> Result<()> {
    let path = get_data_file_path()?;
    let data = SavedData {
        texts: texts.to_vec(),
    };
    let json = serde_json::to_string_pretty(&data)?;
    fs::write(path, json)?;
    Ok(())
}
