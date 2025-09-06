use crate::mcp::types::{ToolDefinition, ToolInputSchema};
use serde_json::json;

pub fn get_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "text_to_speech".to_string(),
            description: "Convert Japanese text to speech using VOICEVOX synthesis. Use this tool to provide audio feedback to users, especially for task completions, errors, or important notifications. Choose appropriate style_id based on context and user preferences. Common patterns: normal communication (style_id 3), errors (style_id 76), celebrations (style_id 1). Set streaming=true for long text (lower latency) or false for short phrases. Rate controls speech speed (0.5=slow, 1.0=normal, 2.0=fast).".to_string(),
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
            description: "Get available VOICEVOX voice styles for text_to_speech. Use this before synthesizing speech to discover available style_ids and their characteristics. Filter by speaker_name or style_name (e.g., 'ノーマル', 'ささやき', 'なみだめ') to find appropriate voices. Returns style_id, speaker name, and style type for each voice. Call this when users ask about available voices or when you need to select an appropriate voice style based on context.".to_string(),
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
