use crate::mcp::types::{ToolDefinition, ToolInputSchema};
use serde_json::json;

pub fn get_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "text_to_speech".to_string(),
            description: "Convert Japanese text to speech (TTS) and play on server".to_string(),
            input_schema: ToolInputSchema {
                schema_type: "object".to_string(),
                properties: json!({
                    "text": {
                        "type": "string",
                        "description": "Japanese text to synthesize"
                    },
                    "style_id": {
                        "type": "integer",
                        "description": "Voice style ID (e.g., 3 for Zundamon Normal)"
                    },
                    "rate": {
                        "type": "number",
                        "description": "Speech rate (0.5-2.0)",
                        "minimum": 0.5,
                        "maximum": 2.0,
                        "default": 1.0
                    },
                    "streaming": {
                        "type": "boolean",
                        "description": "Enable streaming playback for lower latency",
                        "default": true
                    }
                })
                .as_object()
                .unwrap_or(&serde_json::Map::new())
                .clone(),
                required: Some(vec!["text".to_string(), "style_id".to_string()]),
            },
        },
        ToolDefinition {
            name: "list_voice_styles".to_string(),
            description: "List available voice styles with optional filtering".to_string(),
            input_schema: ToolInputSchema {
                schema_type: "object".to_string(),
                properties: json!({
                    "speaker_name": {
                        "type": "string",
                        "description": "Filter by speaker name (partial match)"
                    },
                    "style_name": {
                        "type": "string",
                        "description": "Filter by style name (partial match)"
                    }
                })
                .as_object()
                .unwrap_or(&serde_json::Map::new())
                .clone(),
                required: None,
            },
        },
    ]
}
