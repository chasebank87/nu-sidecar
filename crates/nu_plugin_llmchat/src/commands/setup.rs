use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};

use nu_plugin::{EngineInterface, EvaluatedCall, SimplePluginCommand};
use nu_protocol::{Category, LabeledError, Signature, SyntaxShape, Value};

use sidecar_core::config::{Config, ProviderConfig};
use sidecar_core::provider::ProviderKind;

use crate::LlmChatPlugin;

pub struct LlmSetup;

impl SimplePluginCommand for LlmSetup {
    type Plugin = LlmChatPlugin;

    fn name(&self) -> &str {
        "llm setup"
    }

    fn description(&self) -> &str {
        "Configure the LLM provider used by `ask` and `plan` (one-time setup)"
    }

    fn extra_description(&self) -> &str {
        "Run with no flags for an interactive wizard (reads/writes your terminal directly \
         via /dev/tty, since plugin stdin/stdout is reserved for the nu-plugin protocol). \
         Pass --provider (and friends) to configure non-interactively instead."
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name())
            .named(
                "provider",
                SyntaxShape::String,
                "ollama | lmStudio | openRouter | hermes | openClaw",
                None,
            )
            .named("base-url", SyntaxShape::String, "override the provider's default base URL", None)
            .named("model", SyntaxShape::String, "model name", None)
            .named("api-key", SyntaxShape::String, "API key, if the provider requires one", None)
            .category(Category::Custom("sidecar".into()))
    }

    fn run(
        &self,
        _plugin: &LlmChatPlugin,
        _engine: &EngineInterface,
        call: &EvaluatedCall,
        _input: &Value,
    ) -> Result<Value, LabeledError> {
        let provider_flag: Option<String> = call.get_flag("provider")?;
        let base_url_flag: Option<String> = call.get_flag("base-url")?;
        let model_flag: Option<String> = call.get_flag("model")?;
        let api_key_flag: Option<String> = call.get_flag("api-key")?;

        let (kind, base_url, model, api_key) = if let Some(provider_str) = provider_flag {
            let kind = ProviderKind::parse(&provider_str).ok_or_else(|| {
                LabeledError::new("Unknown provider").with_label(
                    format!(
                        "expected one of: {}",
                        ProviderKind::ALL
                            .iter()
                            .map(|p| p.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    call.head,
                )
            })?;
            let base_url = base_url_flag.unwrap_or_else(|| kind.default_base_url().to_string());
            let model = model_flag.unwrap_or_else(|| kind.suggested_model().to_string());
            (kind, base_url, model, api_key_flag)
        } else {
            run_interactive_wizard(call.head)?
        };

        if kind.requires_api_key() && api_key.as_deref().unwrap_or("").is_empty() {
            return Err(LabeledError::new("Missing API key").with_label(
                format!("{} requires an API key (--api-key, or answer the prompt)", kind.display_name()),
                call.head,
            ));
        }

        let mut cfg = Config::load().unwrap_or_default();
        cfg.set_provider(
            kind,
            ProviderConfig {
                base_url,
                model,
                api_key,
            },
        );
        cfg.save()
            .map_err(|e| LabeledError::new("Failed to save config").with_label(e.to_string(), call.head))?;

        Ok(Value::string(
            format!(
                "Configured {} as the active provider. Try: ask \"hello\"",
                kind.display_name()
            ),
            call.head,
        ))
    }
}

fn run_interactive_wizard(
    head: nu_protocol::Span,
) -> Result<(ProviderKind, String, String, Option<String>), LabeledError> {
    let tty_err = |e: std::io::Error| {
        LabeledError::new("Can't run the interactive wizard")
            .with_label(
                format!(
                    "failed to open /dev/tty ({e}) -- pass --provider/--base-url/--model/--api-key instead"
                ),
                head,
            )
    };

    let tty_in = OpenOptions::new().read(true).open("/dev/tty").map_err(tty_err)?;
    let mut tty_out = OpenOptions::new().write(true).open("/dev/tty").map_err(tty_err)?;
    let mut reader = BufReader::new(tty_in);

    let prompt = |out: &mut std::fs::File, reader: &mut BufReader<std::fs::File>, text: &str| -> Result<String, LabeledError> {
        write!(out, "{text}").map_err(tty_err)?;
        out.flush().map_err(tty_err)?;
        let mut line = String::new();
        reader.read_line(&mut line).map_err(tty_err)?;
        Ok(line.trim().to_string())
    };

    writeln!(tty_out, "nu-sidecar LLM setup").map_err(tty_err)?;
    writeln!(
        tty_out,
        "Providers: {}",
        ProviderKind::ALL
            .iter()
            .enumerate()
            .map(|(i, p)| format!("{}) {}", i + 1, p.display_name()))
            .collect::<Vec<_>>()
            .join("  ")
    )
    .map_err(tty_err)?;

    let choice = prompt(&mut tty_out, &mut reader, "Provider [1-5]: ")?;
    let idx: usize = choice.parse().unwrap_or(0);
    let kind = ProviderKind::ALL
        .get(idx.saturating_sub(1))
        .copied()
        .or_else(|| ProviderKind::parse(&choice))
        .ok_or_else(|| LabeledError::new("Invalid provider choice").with_label(choice.clone(), head))?;

    let default_url = kind.default_base_url();
    let base_url_in = prompt(&mut tty_out, &mut reader, &format!("Base URL [{default_url}]: "))?;
    let base_url = if base_url_in.is_empty() {
        default_url.to_string()
    } else {
        base_url_in
    };

    let suggested = kind.suggested_model();
    let model_prompt = if suggested.is_empty() {
        "Model: ".to_string()
    } else {
        format!("Model [{suggested}]: ")
    };
    let model_in = prompt(&mut tty_out, &mut reader, &model_prompt)?;
    let model = if model_in.is_empty() {
        suggested.to_string()
    } else {
        model_in
    };

    let api_key = if kind.requires_api_key() {
        let key = prompt(&mut tty_out, &mut reader, "API key: ")?;
        Some(key)
    } else {
        let key = prompt(&mut tty_out, &mut reader, "API key (optional, press Enter to skip): ")?;
        (!key.is_empty()).then_some(key)
    };

    Ok((kind, base_url, model, api_key))
}
