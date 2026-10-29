use serde::Serialize;
use serde_json::{Value, json};

use crate::interface::mcp_server::protocol::{RESULT_TYPE_COMPLETE, ResultMeta};

#[derive(Debug, Serialize)]
pub struct ToolCallResult {
    #[serde(rename = "resultType")]
    pub result_type: String,
    pub content: Vec<ToolContent>,
    #[serde(rename = "isError", skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
    #[serde(rename = "_meta")]
    pub meta: Value,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum ToolContent {
    #[serde(rename = "text")]
    Text { text: String },
}

fn text_content(text: impl Into<String>) -> ToolContent {
    ToolContent::Text { text: text.into() }
}

fn result_meta() -> Value {
    serde_json::to_value(ResultMeta::default()).unwrap_or_else(|_| json!({}))
}

pub(crate) fn text_result(text: impl Into<String>, is_error: bool) -> ToolCallResult {
    ToolCallResult {
        result_type: RESULT_TYPE_COMPLETE.to_string(),
        content: vec![text_content(text)],
        is_error: is_error.then_some(true),
        meta: result_meta(),
    }
}

pub(crate) fn success_result() -> ToolCallResult {
    ToolCallResult {
        result_type: RESULT_TYPE_COMPLETE.to_string(),
        content: vec![text_content("ok")],
        is_error: None,
        meta: result_meta(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interface::mcp_server::protocol::{META_SERVER_INFO, SERVER_NAME};

    #[test]
    fn success_result_serializes_to_expected_json() {
        let result = success_result();

        let json = serde_json::to_value(&result).unwrap();

        assert_eq!(
            json,
            json!({
                "resultType": "complete",
                "content": [
                    {
                        "type": "text",
                        "text": "ok"
                    }
                ],
                "_meta": {
                    META_SERVER_INFO: { "name": SERVER_NAME, "version": env!("CARGO_PKG_VERSION") }
                }
            })
        );
    }
}
