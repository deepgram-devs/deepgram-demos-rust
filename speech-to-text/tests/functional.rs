use serde_json::Value;
use std::env;
use std::error::Error;
use std::io;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::process::Command;
use tokio::time::timeout;

const SAMPLE_AUDIO_URL: &str = "https://dpgr.am/spacewalk.wav";
const FUNCTIONAL_TIMEOUT: Duration = Duration::from_secs(90);

fn api_key() -> Option<String> {
    env::var("DEEPGRAM_API_KEY")
        .ok()
        .filter(|key| !key.trim().is_empty())
}

fn binary_path() -> &'static str {
    env!("CARGO_BIN_EXE_dg-stt")
}

fn temp_audio_path() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    env::temp_dir().join(format!(
        "dg-stt-functional-{}-{nonce}.wav",
        std::process::id()
    ))
}

fn timeout_error(operation: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::TimedOut,
        format!(
            "{operation} exceeded {} seconds",
            FUNCTIONAL_TIMEOUT.as_secs()
        ),
    )
}

#[tokio::test]
async fn lists_models_when_api_key_is_available() -> Result<(), Box<dyn Error>> {
    let Some(api_key) = api_key() else {
        eprintln!("skipped: DEEPGRAM_API_KEY is not set");
        return Ok(());
    };

    let mut command = Command::new(binary_path());
    command
        .arg("list-models")
        .env("DEEPGRAM_API_KEY", api_key)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let output = timeout(FUNCTIONAL_TIMEOUT, command.output())
        .await
        .map_err(|_| timeout_error("list-models"))??;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "list-models failed\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("nova"),
        "expected at least one Nova model\nstdout:\n{stdout}"
    );
    Ok(())
}

#[tokio::test]
async fn fast_file_stream_finalizes_when_api_key_is_available() -> Result<(), Box<dyn Error>> {
    let Some(api_key) = api_key() else {
        eprintln!("skipped: DEEPGRAM_API_KEY is not set");
        return Ok(());
    };

    let audio_path = temp_audio_path();
    let audio = reqwest::get(SAMPLE_AUDIO_URL)
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    std::fs::write(&audio_path, audio)?;

    let mut command = Command::new(binary_path());
    command
        .args(["stream", "file", "--file"])
        .arg(&audio_path)
        .args(["--fast", "--output", "json", "--verbose"])
        .env("DEEPGRAM_API_KEY", api_key)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let output_result = timeout(FUNCTIONAL_TIMEOUT, command.output()).await;
    let _ = std::fs::remove_file(&audio_path);
    let output = output_result.map_err(|_| timeout_error("fast file streaming"))??;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "file streaming failed\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    let responses: Vec<Value> = stdout
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    let has_transcript = responses.iter().any(|response| {
        response["type"] == "Results"
            && response["channel"]["alternatives"][0]["transcript"]
                .as_str()
                .is_some_and(|transcript| !transcript.trim().is_empty())
    });
    let finalized = responses
        .iter()
        .any(|response| response["from_finalize"].as_bool() == Some(true));

    assert!(has_transcript, "no non-empty transcript received\n{stdout}");
    assert!(finalized, "no from_finalize response received\n{stdout}");
    assert!(
        stderr.contains("WebSocket sender transmitted Finalize"),
        "Finalize was not transmitted\nstderr:\n{stderr}"
    );
    Ok(())
}
