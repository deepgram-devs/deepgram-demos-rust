use std::path::PathBuf;
use std::process::{Child, Command};
use std::thread;
use std::time::{Duration, Instant};

use crate::config;

const API_KEY_WAIT_TIMEOUT: Duration = Duration::from_secs(300);
const API_KEY_POLL_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Clone, Copy)]
pub enum SidecarPage {
    Settings,
    ApiKey,
}

pub fn launch_settings() -> Result<bool, String> {
    launch_detached(SidecarPage::Settings)
}

pub fn prompt_for_api_key() -> Option<String> {
    let exe = sidecar_exe_path()?;
    let mut child = spawn_sidecar(&exe, SidecarPage::ApiKey).ok()?;
    wait_for_api_key(&mut child)
}

fn launch_detached(page: SidecarPage) -> Result<bool, String> {
    let Some(exe) = sidecar_exe_path() else {
        return Ok(false);
    };

    spawn_sidecar(&exe, page).map(|_| true)
}

fn spawn_sidecar(exe: &PathBuf, page: SidecarPage) -> Result<Child, String> {
    let mut command = Command::new(exe);
    command.arg("--page");
    command.arg(match page {
        SidecarPage::Settings => "settings",
        SidecarPage::ApiKey => "api-key",
    });

    command
        .spawn()
        .map_err(|error| format!("Failed to launch {}: {error}", exe.display()))
}

fn wait_for_api_key(child: &mut Child) -> Option<String> {
    let started = Instant::now();
    loop {
        if started.elapsed() > API_KEY_WAIT_TIMEOUT {
            return None;
        }

        if let Ok(state) = config::load_state() {
            if let Some(api_key) = state.config.api_key.filter(|value| !value.trim().is_empty()) {
                return Some(api_key);
            }
        }

        match child.try_wait() {
            Ok(Some(_status)) => {
                if let Ok(state) = config::load_state() {
                    return state.config.api_key.filter(|value| !value.trim().is_empty());
                }
                return None;
            }
            Ok(None) => thread::sleep(API_KEY_POLL_INTERVAL),
            Err(_) => return None,
        }
    }
}

fn sidecar_exe_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("VELOCITY_SETTINGS_EXE") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Some(path);
        }
    }

    sidecar_candidates()
        .into_iter()
        .find(|candidate| candidate.exists())
}

fn sidecar_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            candidates.push(parent.join("Velocity.Settings.exe"));
        }
    }

    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    candidates.extend(dev_build_candidates(&repo_root));

    if let Ok(current_dir) = std::env::current_dir() {
        if current_dir != repo_root {
            candidates.extend(dev_build_candidates(&current_dir));
        }
    }

    candidates
}

fn dev_build_candidates(root: &std::path::Path) -> Vec<PathBuf> {
    let sidecar_root = root.join("Velocity.Settings").join("bin");
    [
        sidecar_root.join("x64").join("Release").join("net10.0-windows10.0.19041.0").join("Velocity.Settings.exe"),
        sidecar_root.join("x64").join("Debug").join("net10.0-windows10.0.19041.0").join("Velocity.Settings.exe"),
        sidecar_root.join("Release").join("net10.0-windows10.0.19041.0").join("Velocity.Settings.exe"),
        sidecar_root.join("Debug").join("net10.0-windows10.0.19041.0").join("Velocity.Settings.exe"),
        sidecar_root.join("x64").join("Release").join("net8.0-windows10.0.19041.0").join("Velocity.Settings.exe"),
        sidecar_root.join("x64").join("Debug").join("net8.0-windows10.0.19041.0").join("Velocity.Settings.exe"),
        sidecar_root.join("Release").join("net8.0-windows10.0.19041.0").join("Velocity.Settings.exe"),
        sidecar_root.join("Debug").join("net8.0-windows10.0.19041.0").join("Velocity.Settings.exe"),
    ]
    .into_iter()
    .collect()
}
