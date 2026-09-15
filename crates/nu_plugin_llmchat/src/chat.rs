//! Shared implementation behind the `sidecar ask` / `sidecar plan` commands.

use nu_protocol::{LabeledError, Span};
use sidecar_core::config::Config;
use sidecar_core::llm_client::{self, ChatMessage};
use sidecar_core::transcript;

pub fn answer(
    prompt: &str,
    session_pid: i64,
    last: Option<i64>,
    system_prompt: &str,
    head: Span,
) -> Result<String, LabeledError> {
    let cfg = Config::load().map_err(|e| {
        LabeledError::new("Failed to load config").with_label(e.to_string(), head)
    })?;

    let (kind, provider_cfg) = cfg.active_provider_config().map_err(|e| {
        LabeledError::new("No LLM provider configured").with_label(e.to_string(), head)
    })?;

    let context_entries = transcript::read_context(
        session_pid as u32,
        last.map(|n| n as usize),
        cfg.transcript.max_total_chars,
    )
    .unwrap_or_default();

    let mut messages = vec![ChatMessage::system(system_prompt)];

    if !context_entries.is_empty() {
        let mut context_text = String::from(
            "Recent terminal activity in this session (most recent last):\n",
        );
        for entry in &context_entries {
            context_text.push_str(&format!("$ {}\n", entry.cmd));
            if let Some(output) = &entry.output {
                context_text.push_str(output);
                if !output.ends_with('\n') {
                    context_text.push('\n');
                }
            }
        }
        messages.push(ChatMessage::system(context_text));
    }

    messages.push(ChatMessage::user(prompt));

    llm_client::chat_completion(kind, &provider_cfg, &messages)
        .map_err(|e| LabeledError::new("LLM request failed").with_label(e.to_string(), head))
}
