use nu_plugin::{Plugin, PluginCommand};

mod commands;
pub mod rules;

pub use commands::Bashify;

pub struct BashifyPlugin;

impl Plugin for BashifyPlugin {
    fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").into()
    }

    fn commands(&self) -> Vec<Box<dyn PluginCommand<Plugin = Self>>> {
        vec![Box::new(Bashify)]
    }
}
