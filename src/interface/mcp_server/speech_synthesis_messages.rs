use tokio::sync::oneshot;

use crate::interface::mcp_server::tool_types::{text_result, ToolCallResult};

#[must_use]
pub fn synthesis_success_message(text_len: usize, style_id: u32) -> String {
    format!("Synthesized {text_len} characters using style ID {style_id}")
}

#[must_use]
pub fn cancellation_message(reason: &str) -> String {
    if reason.is_empty() {
        "Synthesis cancelled".to_string()
    } else {
        format!("Synthesis cancelled: {reason}")
    }
}

#[must_use]
pub fn cancellation_result(reason: String) -> ToolCallResult {
    text_result(cancellation_message(&reason), true)
}

#[must_use]
pub fn try_take_cancellation(cancel_rx: &mut oneshot::Receiver<String>) -> Option<String> {
    match cancel_rx.try_recv() {
        Ok(reason) => Some(reason),
        Err(oneshot::error::TryRecvError::Closed) => Some(String::new()),
        Err(oneshot::error::TryRecvError::Empty) => None,
    }
}
