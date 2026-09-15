use nu_plugin::{EngineInterface, EvaluatedCall, SimplePluginCommand};
use nu_protocol::{Category, LabeledError, Record, Signature, Value};

use sidecar_core::config::Config;

use crate::LlmChatPlugin;

pub struct LlmStatus;

impl SimplePluginCommand for LlmStatus {
    type Plugin = LlmChatPlugin;

    fn name(&self) -> &str {
        "llm status"
    }

    fn description(&self) -> &str {
        "Show the active LLM provider and model (never prints the API key)"
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name()).category(Category::Custom("sidecar".into()))
    }

    fn run(
        &self,
        _plugin: &LlmChatPlugin,
        _engine: &EngineInterface,
        call: &EvaluatedCall,
        _input: &Value,
    ) -> Result<Value, LabeledError> {
        let cfg = Config::load().map_err(|e| {
            LabeledError::new("Failed to load config").with_label(e.to_string(), call.head)
        })?;

        let mut record = Record::new();
        match cfg.active_provider_config() {
            Ok((kind, provider_cfg)) => {
                record.push("provider", Value::string(kind.display_name(), call.head));
                record.push("base_url", Value::string(provider_cfg.base_url, call.head));
                record.push("model", Value::string(provider_cfg.model, call.head));
                record.push(
                    "api_key_set",
                    Value::bool(
                        provider_cfg.api_key.map(|k| !k.is_empty()).unwrap_or(false),
                        call.head,
                    ),
                );
            }
            Err(e) => {
                record.push("provider", Value::nothing(call.head));
                record.push("error", Value::string(e.to_string(), call.head));
            }
        }

        Ok(Value::record(record, call.head))
    }
}
