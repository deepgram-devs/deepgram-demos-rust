use crate::config;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

const LOG_FILE_NAME: &str = "tts-tui.log";

#[derive(Clone, Copy)]
struct LogLimits {
    max_size_bytes: u64,
    max_files: usize,
}

static LOG_LIMITS: OnceLock<Mutex<LogLimits>> = OnceLock::new();

pub fn init(logging: &config::LoggingConfig) {
    let limits = LogLimits {
        max_size_bytes: logging.max_size_bytes.max(1),
        max_files: logging.max_files.max(1),
    };
    let _ = LOG_LIMITS.set(Mutex::new(limits));
}

pub fn write(level: &str, message: &str) {
    let Some(limits) = LOG_LIMITS
        .get()
        .and_then(|limits| limits.lock().ok().map(|guard| *guard))
    else {
        return;
    };
    let Some(directory) = config::config_directory() else {
        return;
    };
    let _ = fs::create_dir_all(&directory);
    let line = format!(
        "{} {:<7} {}\n",
        chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%.3f%:z"),
        level,
        message
    );
    let _ = append_rotating(&directory.join(LOG_FILE_NAME), line.as_bytes(), limits);
}

fn append_rotating(path: &Path, line: &[u8], limits: LogLimits) -> io::Result<()> {
    let current_size = fs::metadata(path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    if current_size > 0 && current_size.saturating_add(line.len() as u64) > limits.max_size_bytes {
        rotate(path, limits.max_files)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(line)
}

fn rotate(path: &Path, max_files: usize) -> io::Result<()> {
    if max_files == 1 {
        return fs::remove_file(path);
    }

    let oldest = backup_path(path, max_files - 1);
    if oldest.exists() {
        fs::remove_file(oldest)?;
    }
    for index in (1..max_files - 1).rev() {
        let source = backup_path(path, index);
        if source.exists() {
            fs::rename(source, backup_path(path, index + 1))?;
        }
    }
    fs::rename(path, backup_path(path, 1))
}

fn backup_path(path: &Path, index: usize) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(LOG_FILE_NAME);
    path.with_file_name(format!("{file_name}.{index}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotates_and_retains_the_requested_number_of_files() {
        let directory = std::env::temp_dir().join(format!(
            "tts-tui-log-test-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join(LOG_FILE_NAME);
        let limits = LogLimits {
            max_size_bytes: 10,
            max_files: 3,
        };

        append_rotating(&path, b"one\n", limits).unwrap();
        append_rotating(&path, b"two-two\n", limits).unwrap();
        append_rotating(&path, b"three-three\n", limits).unwrap();
        append_rotating(&path, b"four-four\n", limits).unwrap();

        assert!(path.exists());
        assert!(backup_path(&path, 1).exists());
        assert!(backup_path(&path, 2).exists());
        assert!(!backup_path(&path, 3).exists());

        fs::remove_dir_all(directory).unwrap();
    }
}
