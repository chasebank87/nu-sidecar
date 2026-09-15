use nu_plugin::{serve_plugin, MsgPackSerializer};
use nu_plugin_llmchat::LlmChatPlugin;

fn main() {
    serve_plugin(&LlmChatPlugin, MsgPackSerializer {})
}
