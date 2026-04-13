use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;

static VERBOSE: OnceLock<bool> = OnceLock::new();

pub fn init(verbose: bool) {
    let _ = VERBOSE.set(verbose);
}

pub fn is_verbose() -> bool {
    *VERBOSE.get().unwrap_or(&false)
}

fn log_path() -> PathBuf {
    let home = std::env::var("USERPROFILE").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".config").join("velocity.log")
}

/// Writes a line to the log file. Always appends; creates the file if needed.
pub fn log(msg: &str) {
    let path = log_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&path) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let _ = writeln!(f, "[{now}] {msg}");
    }
}

/// Logs only when --verbose was passed.
pub fn verbose(msg: &str) {
    if is_verbose() {
        log(msg);
    }
}
