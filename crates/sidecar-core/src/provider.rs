use serde::{Deserialize, Serialize};
use std::fmt;

/// Mirrors `LLMProviderKind` in Sauron's `Providers/LLMClient.swift`, so this
/// tool talks to the same five backends with the same defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProviderKind {
    Ollama,
    LmStudio,
    OpenRouter,
    Hermes,
    OpenClaw,
}

impl ProviderKind {
    pub const ALL: [ProviderKind; 5] = [
        ProviderKind::Ollama,
        ProviderKind::LmStudio,
        ProviderKind::OpenRouter,
        ProviderKind::Hermes,
        ProviderKind::OpenClaw,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            ProviderKind::Ollama => "ollama",
            ProviderKind::LmStudio => "lmStudio",
            ProviderKind::OpenRouter => "openRouter",
            ProviderKind::Hermes => "hermes",
            ProviderKind::OpenClaw => "openClaw",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            ProviderKind::Ollama => "Ollama",
            ProviderKind::LmStudio => "LM Studio",
            ProviderKind::OpenRouter => "OpenRouter",
            ProviderKind::Hermes => "Hermes",
            ProviderKind::OpenClaw => "OpenClaw",
        }
    }

    pub fn default_base_url(&self) -> &'static str {
        match self {
            ProviderKind::Ollama => "http://127.0.0.1:11434",
            ProviderKind::LmStudio => "http://127.0.0.1:1234",
            ProviderKind::OpenRouter => "https://openrouter.ai/api/v1",
            ProviderKind::Hermes => "http://127.0.0.1:8642",
            ProviderKind::OpenClaw => "http://127.0.0.1:18789",
        }
    }

    pub fn requires_api_key(&self) -> bool {
        matches!(
            self,
            ProviderKind::OpenRouter | ProviderKind::Hermes | ProviderKind::OpenClaw
        )
    }

    pub fn suggested_model(&self) -> &'static str {
        match self {
            ProviderKind::Ollama | ProviderKind::LmStudio => "",
            ProviderKind::OpenRouter => "openai/gpt-4o-mini",
            ProviderKind::Hermes => "hermes-agent",
            ProviderKind::OpenClaw => "openclaw/default",
        }
    }

    pub fn parse(s: &str) -> Option<ProviderKind> {
        Self::ALL
            .into_iter()
            .find(|p| p.as_str().eq_ignore_ascii_case(s) || p.display_name().eq_ignore_ascii_case(s))
    }
}

impl fmt::Display for ProviderKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
