mod execution_runtime;
pub mod protocol;
pub mod requests;
pub mod server;
mod speech_synthesis_messages;
pub mod speech_synthesis_tool;
pub mod startup;
pub mod tool_catalog;
pub mod tool_execution;
pub mod tool_types;
pub mod voice_style_tool;

#[cfg(test)]
mod tool_execution_tests;

pub use server::run_mcp_server;
pub use startup::run_mcp_server_app;
