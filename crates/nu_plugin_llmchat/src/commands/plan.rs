use nu_plugin::{EngineInterface, EvaluatedCall, SimplePluginCommand};
use nu_protocol::{Category, Example, LabeledError, Signature, SyntaxShape, Value};

use crate::chat;
use crate::LlmChatPlugin;

const SYSTEM_PROMPT: &str = "You are a terminal assistant embedded in the user's nushell \
    shell. The user wants a concrete, numbered step-by-step plan to accomplish a goal. Use \
    any recent terminal activity provided as context to ground the plan in their actual \
    current state, but do not execute anything yourself -- only propose the steps.";

/// Internal command, called via the `plan` wrapper `def` in nu/sidecar.nu,
/// which injects `--session-pid $nu.pid` automatically.
pub struct SidecarPlan;

impl SimplePluginCommand for SidecarPlan {
    type Plugin = LlmChatPlugin;

    fn name(&self) -> &str {
        "sidecar plan"
    }

    fn description(&self) -> &str {
        "Ask the configured LLM for a step-by-step plan, with recent terminal context"
    }

    fn extra_description(&self) -> &str {
        "Not meant to be called directly -- use the `plan` command, which wraps this with \
         the current session's id."
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name())
            .required("goal", SyntaxShape::String, "the goal to plan for")
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
            example: r#"plan "get this repo's tests passing""#,
            description: "get a step-by-step plan grounded in recent terminal activity",
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
        let goal: String = call.req(0)?;
        let session_pid: i64 = call.get_flag("session-pid")?.ok_or_else(|| {
            LabeledError::new("Missing --session-pid")
                .with_label("call `plan`, not `sidecar plan` directly", call.head)
        })?;
        let last: Option<i64> = call.get_flag("last")?;

        let text = chat::answer(&goal, session_pid, last, SYSTEM_PROMPT, call.head)?;
        Ok(Value::string(text, call.head))
    }
}
