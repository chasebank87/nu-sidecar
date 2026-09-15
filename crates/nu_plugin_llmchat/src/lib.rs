use nu_plugin::{Plugin, PluginCommand};

mod chat;
mod commands;

pub use commands::{LlmSetup, LlmStatus, SidecarAsk, SidecarLogTurn, SidecarPlan};

pub struct LlmChatPlugin;

impl Plugin for LlmChatPlugin {
    fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").into()
    }

    fn commands(&self) -> Vec<Box<dyn PluginCommand<Plugin = Self>>> {
        vec![
            Box::new(SidecarAsk),
            Box::new(SidecarPlan),
            Box::new(SidecarLogTurn),
            Box::new(LlmSetup),
            Box::new(LlmStatus),
        ]
    }
}
