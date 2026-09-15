use std::path::PathBuf;

const APP_DIR: &str = "nu-sidecar";

fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

/// `$XDG_CONFIG_HOME/nu-sidecar` (default `~/.config/nu-sidecar`).
pub fn config_dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".config"))
        .join(APP_DIR)
}

/// `$XDG_STATE_HOME/nu-sidecar` (default `~/.local/state/nu-sidecar`).
pub fn state_dir() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".local").join("state"))
        .join(APP_DIR)
}

pub fn config_file() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn sessions_dir() -> PathBuf {
    state_dir().join("sessions")
}
