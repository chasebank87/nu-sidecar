use nu_plugin::{EngineInterface, EvaluatedCall, SimplePluginCommand};
use nu_protocol::{Category, Example, LabeledError, Signature, SyntaxShape, Value};

use crate::chat;
use crate::LlmChatPlugin;

const SYSTEM_PROMPT: &str = "You are a terminal assistant embedded in the user's nushell \
    shell. Answer concisely and directly. When recent terminal activity is provided, use it \
    as context but don't just repeat it back.";

/// Internal command, called via the `ask` wrapper `def` in nu/sidecar.nu,
/// which injects `--session-pid $nu.pid` automatically.
pub struct SidecarAsk;

impl SimplePluginCommand for SidecarAsk {
    type Plugin = LlmChatPlugin;

    fn name(&self) -> &str {
        "sidecar ask"
    }

    fn description(&self) -> &str {
        "Ask the configured LLM a question, with recent terminal context"
    }

    fn extra_description(&self) -> &str {
        "Not meant to be called directly -- use the `ask` command, which wraps this with \
         the current session's id."
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name())
            .required("question", SyntaxShape::String, "the question to ask")
            .named(
                "session-pid",
                SyntaxShape::Int,
                "the nushell session's $nu.pid, used to find its transcript",
                None,
            )
            .named(
                "last",
                SyntaxShape::Int,
                "only include the last N terminal turns as context",
                None,
            )
            .category(Category::Custom("sidecar".into()))
    }

    fn examples(&self) -> Vec<Example<'_>> {
        vec![Example {
            example: r#"ask "why did that last command fail?""#,
            description: "ask about recent terminal activity",
            result: None,
        }]
    }

    fn run(
        &self,
        _plugin: &LlmChatPlugin,
        _engine: &EngineInterface,
        call: &EvaluatedCall,
        _input: &Value,
    ) -> Result<Value, LabeledError> {
        let question: String = call.req(0)?;
        let session_pid: i64 = call.get_flag("session-pid")?.ok_or_else(|| {
            LabeledError::new("Missing --session-pid")
                .with_label("call `ask`, not `sidecar ask` directly", call.head)
        })?;
        let last: Option<i64> = call.get_flag("last")?;

        let text = chat::answer(&question, session_pid, last, SYSTEM_PROMPT, call.head)?;
        Ok(Value::string(text, call.head))
    }
}
