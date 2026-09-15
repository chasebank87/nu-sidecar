use nu_plugin::{EngineInterface, EvaluatedCall, SimplePluginCommand};
use nu_protocol::{Category, LabeledError, Signature, SyntaxShape, Value};

use sidecar_core::config::Config;
use sidecar_core::transcript::{self, TranscriptEntry};

use crate::LlmChatPlugin;

/// Called once per real user command from the `pre_prompt` hook in
/// nu/sidecar.nu -- never invoked directly. Appends one entry to the
/// current session's JSONL transcript, capped per-entry per the user's
/// `[transcript] max_chars_per_entry` config.
pub struct SidecarLogTurn;

impl SimplePluginCommand for SidecarLogTurn {
    type Plugin = LlmChatPlugin;

    fn name(&self) -> &str {
        "sidecar log-turn"
    }

    fn description(&self) -> &str {
        "Internal: append one command + output entry to the session transcript"
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name())
            .named("session-pid", SyntaxShape::Int, "$nu.pid", None)
            .named("cmd", SyntaxShape::String, "the command text", None)
            .named(
                "output",
                SyntaxShape::String,
                "captured output, or an explanatory note if not captured",
                None,
            )
            .named("exit-code", SyntaxShape::Int, "$env.LAST_EXIT_CODE", None)
            .category(Category::Custom("sidecar".into()))
    }

    fn run(
        &self,
        _plugin: &LlmChatPlugin,
        _engine: &EngineInterface,
        call: &EvaluatedCall,
        _input: &Value,
    ) -> Result<Value, LabeledError> {
        let session_pid: i64 = call
            .get_flag("session-pid")?
            .ok_or_else(|| LabeledError::new("Missing --session-pid").with_label("", call.head))?;
        let cmd: String = call
            .get_flag("cmd")?
            .ok_or_else(|| LabeledError::new("Missing --cmd").with_label("", call.head))?;
        let output: Option<String> = call.get_flag("output")?;
        let exit_code: i64 = call.get_flag("exit-code")?.unwrap_or(0);

        // Config load failure or a missing/unwritable state dir should never
        // break the user's actual command output -- fail soft.
        let max_chars = Config::load()
            .map(|c| c.transcript.max_chars_per_entry)
            .unwrap_or(4000);

        let entry = TranscriptEntry {
            ts: sidecar_core::now_iso(),
            cmd,
            output,
            exit_code,
        };

        let _ = transcript::append_entry(session_pid as u32, entry, max_chars);

        Ok(Value::nothing(call.head))
    }
}
