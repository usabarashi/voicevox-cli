use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolCallResult {
    pub content: Vec<ToolContent>,
    #[serde(rename = "isError", skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ToolContent {
    #[serde(rename = "text")]
    Text { text: String },
}

fn text_content(text: impl Into<String>) -> ToolContent {
    ToolContent::Text { text: text.into() }
}

pub(crate) fn text_result(text: impl Into<String>, is_error: bool) -> ToolCallResult {
    ToolCallResult {
        content: vec![text_content(text)],
        is_error: is_error.then_some(true),
    }
}

pub(crate) fn success_result() -> ToolCallResult {
    ToolCallResult {
        content: vec![text_content("ok")],
        is_error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success_result_serializes_to_expected_json() {
        let result = success_result();

        let json = serde_json::to_value(&result).unwrap();

        assert_eq!(
            json,
            serde_json::json!({
                "content": [
                    {
                        "type": "text",
                        "text": "ok"
                    }
                ]
            })
        );
    }
}
