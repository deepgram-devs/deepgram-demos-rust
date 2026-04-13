use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    pub api_key: Option<String>,
    pub smart_format: Option<bool>,
    pub model: Option<String>,
}

fn config_path() -> PathBuf {
    let home = std::env::var("USERPROFILE").expect("USERPROFILE not set");
    PathBuf::from(home).join(".config").join("velocity.yml")
}

pub fn load() -> Config {
    let path = config_path();
    if path.exists() {
        let contents = fs::read_to_string(&path).unwrap_or_default();
        serde_yaml::from_str(&contents).unwrap_or_default()
    } else {
        Config::default()
    }
}

pub fn save(config: &Config) {
    let path = config_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let contents = serde_yaml::to_string(config).expect("Failed to serialize config");
    fs::write(&path, contents).expect("Failed to write config");
}
