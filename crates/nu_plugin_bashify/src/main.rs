use nu_plugin::{serve_plugin, MsgPackSerializer};
use nu_plugin_bashify::BashifyPlugin;

fn main() {
    serve_plugin(&BashifyPlugin, MsgPackSerializer {})
}
