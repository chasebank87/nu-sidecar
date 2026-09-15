use crate::config::ProviderConfig;
use crate::provider::ProviderKind;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        ChatMessage {
            role: "system".into(),
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        ChatMessage {
            role: "user".into(),
            content: content.into(),
        }
    }
}

#[derive(Debug)]
pub enum ChatError {
    NoModel,
    Http { status: u16, body: String },
    Transport(String),
    Decoding(String),
    Empty,
}

impl fmt::Display for ChatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChatError::NoModel => write!(
                f,
                "no model configured; run `llm setup` to set one"
            ),
            ChatError::Http { status, body } => write!(f, "HTTP {status}: {body}"),
            ChatError::Transport(e) => write!(f, "request failed: {e}"),
            ChatError::Decoding(e) => write!(f, "failed to decode response: {e}"),
            ChatError::Empty => write!(f, "provider returned an empty response"),
        }
    }
}

impl std::error::Error for ChatError {}

/// `{base}/v1` unless `base` already ends in `/v1`, mirroring
/// `LLMClient.openAIRoot` in Sauron's Swift client.
fn openai_root(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    if trimmed.ends_with("/v1") {
        trimmed.to_string()
    } else {
        format!("{trimmed}/v1")
    }
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    stream: bool,
    temperature: f32,
    messages: &'a [ChatMessage],
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatResponseMessage,
}

#[derive(Deserialize)]
struct ChatResponseMessage {
    content: Option<String>,
}

/// Non-streaming chat completion against any of the five OpenAI-compatible
/// providers. All five share one request/response shape; the only per-provider
/// divergence is OpenRouter's extra attribution headers (see `apply_auth`).
pub fn chat_completion(
    kind: ProviderKind,
    provider: &ProviderConfig,
    messages: &[ChatMessage],
) -> Result<String, ChatError> {
    if provider.model.trim().is_empty() {
        return Err(ChatError::NoModel);
    }
    let url = format!("{}/chat/completions", openai_root(&provider.base_url));
    let body = ChatRequest {
        model: &provider.model,
        stream: false,
        temperature: 0.2,
        messages,
    };

    let mut request = ureq::post(&url).set("Content-Type", "application/json");
    request = apply_auth(request, kind, provider);

    let response = match request.send_json(body) {
        Ok(r) => r,
        Err(ureq::Error::Status(status, r)) => {
            let text = r.into_string().unwrap_or_default();
            return Err(ChatError::Http { status, body: text });
        }
        Err(e) => return Err(ChatError::Transport(e.to_string())),
    };

    let parsed: ChatResponse = response
        .into_json()
        .map_err(|e| ChatError::Decoding(e.to_string()))?;

    let content = parsed
        .choices
        .into_iter()
        .next()
        .and_then(|c| c.message.content)
        .unwrap_or_default();

    if content.trim().is_empty() {
        return Err(ChatError::Empty);
    }
    Ok(content)
}

fn apply_auth(
    request: ureq::Request,
    kind: ProviderKind,
    provider: &ProviderConfig,
) -> ureq::Request {
    let request = match &provider.api_key {
        Some(key) if !key.is_empty() => {
            request.set("Authorization", &format!("Bearer {key}"))
        }
        _ => request,
    };
    match kind {
        ProviderKind::OpenRouter => request
            .set("HTTP-Referer", "https://github.com/nu-sidecar/nu-sidecar")
            .set("X-Title", "nu-sidecar"),
        _ => request,
    }
}
