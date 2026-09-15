use crate::provider::ProviderKind;
use crate::xdg;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub base_url: String,
    #[serde(default)]
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptConfig {
    pub max_chars_per_entry: usize,
    pub max_total_chars: usize,
}

impl Default for TranscriptConfig {
    fn default() -> Self {
        TranscriptConfig {
            max_chars_per_entry: 4000,
            max_total_chars: 32_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub active_provider: String,
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
    #[serde(default)]
    pub transcript: TranscriptConfig,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            active_provider: ProviderKind::Ollama.as_str().to_string(),
            providers: HashMap::new(),
            transcript: TranscriptConfig::default(),
        }
    }
}

#[derive(Debug)]
pub enum ConfigError {
    Io(io::Error),
    Parse(toml::de::Error),
    Serialize(toml::ser::Error),
    NotConfigured(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io(e) => write!(f, "io error: {e}"),
            ConfigError::Parse(e) => write!(f, "failed to parse config.toml: {e}"),
            ConfigError::Serialize(e) => write!(f, "failed to serialize config: {e}"),
            ConfigError::NotConfigured(p) => write!(
                f,
                "provider '{p}' is not configured yet; run `llm setup --provider {p}`"
            ),
        }
    }
}

impl std::error::Error for ConfigError {}

impl Config {
    pub fn load() -> Result<Config, ConfigError> {
        let path = xdg::config_file();
        if !path.exists() {
            return Ok(Config::default());
        }
        let raw = fs::read_to_string(&path).map_err(ConfigError::Io)?;
        toml::from_str(&raw).map_err(ConfigError::Parse)
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        let dir = xdg::config_dir();
        fs::create_dir_all(&dir).map_err(ConfigError::Io)?;
        let path = xdg::config_file();
        let raw = toml::to_string_pretty(self).map_err(ConfigError::Serialize)?;
        fs::write(&path, raw).map_err(ConfigError::Io)?;
        set_owner_only_permissions(&path).map_err(ConfigError::Io)?;
        Ok(())
    }

    pub fn active_provider_kind(&self) -> Option<ProviderKind> {
        ProviderKind::parse(&self.active_provider)
    }

    /// The fully-resolved config for the active provider, or a helpful error
    /// telling the user to run `llm setup` if it hasn't been configured.
    pub fn active_provider_config(&self) -> Result<(ProviderKind, ProviderConfig), ConfigError> {
        let kind = self
            .active_provider_kind()
            .ok_or_else(|| ConfigError::NotConfigured(self.active_provider.clone()))?;
        let cfg = self
            .providers
            .get(kind.as_str())
            .cloned()
            .ok_or_else(|| ConfigError::NotConfigured(kind.as_str().to_string()))?;
        Ok((kind, cfg))
    }

    pub fn set_provider(&mut self, kind: ProviderKind, cfg: ProviderConfig) {
        self.providers.insert(kind.as_str().to_string(), cfg);
        self.active_provider = kind.as_str().to_string();
    }
}

#[cfg(unix)]
fn set_owner_only_permissions(path: &std::path::Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn set_owner_only_permissions(_path: &std::path::Path) -> io::Result<()> {
    Ok(())
}
