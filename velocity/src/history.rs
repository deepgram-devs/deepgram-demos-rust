use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoryEntry {
    pub created_at_unix: u64,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TranscriptHistory {
    #[serde(default)]
    pub entries: Vec<HistoryEntry>,
}

impl TranscriptHistory {
    pub fn load(path: &Path) -> Self {
        if !path.exists() {
            return Self::default();
        }

        fs::read_to_string(path)
            .ok()
            .and_then(|contents| serde_yaml::from_str(&contents).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create {}: {e}", parent.display()))?;
        }

        let contents =
            serde_yaml::to_string(self).map_err(|e| format!("Failed to serialize history: {e}"))?;
        fs::write(path, contents).map_err(|e| format!("Failed to write {}: {e}", path.display()))
    }

    pub fn push(&mut self, text: String, limit: usize) {
        if text.trim().is_empty() {
            return;
        }

        self.entries.retain(|entry| entry.text != text);
        self.entries.insert(
            0,
            HistoryEntry {
                created_at_unix: now_unix(),
                text,
            },
        );
        self.entries.truncate(limit);
    }

    pub fn trim_to(&mut self, limit: usize) {
        self.entries.truncate(limit);
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_moves_newest_entry_to_front() {
        let mut history = TranscriptHistory::default();
        history.push("first".into(), 20);
        history.push("second".into(), 20);

        assert_eq!(history.entries[0].text, "second");
    }

    #[test]
    fn push_deduplicates_and_trims() {
        let mut history = TranscriptHistory::default();
        history.push("one".into(), 2);
        history.push("two".into(), 2);
        history.push("one".into(), 2);

        assert_eq!(history.entries.len(), 2);
        assert_eq!(history.entries[0].text, "one");
    }
}
