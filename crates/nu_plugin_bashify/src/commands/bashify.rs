use nu_plugin::{EngineInterface, EvaluatedCall, SimplePluginCommand};
use nu_protocol::{Category, Example, LabeledError, Signature, SyntaxShape, Value};

use crate::rules;
use crate::BashifyPlugin;

pub struct Bashify;

impl SimplePluginCommand for Bashify {
    type Plugin = BashifyPlugin;

    fn name(&self) -> &str {
        "bashify"
    }

    fn description(&self) -> &str {
        "Translate a bash-flavored command line into nushell syntax"
    }

    fn extra_description(&self) -> &str {
        "Pure text transform -- never executes anything. Unrecognized input is \
         returned unchanged. Used by the sidecar Enter-key translate-and-confirm \
         keybinding, but can be called directly too."
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name())
            .required(
                "input",
                SyntaxShape::String,
                "bash-ish command line to translate",
            )
            .switch("diff", "show a before/after diff instead of just the translation", None)
            .switch("explain", "annotate which rules fired", None)
            .category(Category::Strings)
    }

    fn search_terms(&self) -> Vec<&str> {
        vec!["bash", "nushell", "translate", "convert"]
    }

    fn examples(&self) -> Vec<Example<'_>> {
        vec![Example {
            example: r#"bashify "export FOO=bar && echo $FOO""#,
            description: "translate a bash chain into nu syntax",
            result: None,
        }]
    }

    fn run(
        &self,
        _plugin: &BashifyPlugin,
        _engine: &EngineInterface,
        call: &EvaluatedCall,
        _input: &Value,
    ) -> Result<Value, LabeledError> {
        let text: String = call.req(0)?;
        let result = rules::translate(&text);

        let mut output = if call.has_flag("diff")? {
            format!("- {text}\n+ {}", result.translated)
        } else {
            result.translated.clone()
        };

        if call.has_flag("explain")? {
            if result.rules_fired.is_empty() {
                output.push_str("\n# bashify: no rules fired, input unchanged");
            } else {
                output.push_str(&format!(
                    "\n# bashify: rules fired: {}",
                    result.rules_fired.join(", ")
                ));
            }
        }

        Ok(Value::string(output, call.head))
    }
}
