mod common;

use anyhow::Result;
use common::{get_server_path, JsonRpcRequest, McpClient};
use serde_json::json;

#[test]
fn test_initialize_sequence() -> Result<()> {
    let server_path = get_server_path();
    let mut client = McpClient::start(&server_path)?;

    let response = client.initialize()?;

    // Verify response structure
    assert!(response.result.is_some(), "Initialize should return result");
    let result = response.result.unwrap();

    // Check protocol version
    assert_eq!(
        result["protocolVersion"].as_str(),
        Some("2024-11-05"),
        "Protocol version should be 2024-11-05"
    );

    // Check server info
    assert_eq!(
        result["serverInfo"]["name"].as_str(),
        Some("voicevox-mcp"),
        "Server name should be voicevox-mcp"
    );

    // Check capabilities
    assert!(
        result["capabilities"]["tools"].is_object(),
        "Server should advertise tools capability"
    );

    Ok(())
}

#[test]
fn test_tools_list() -> Result<()> {
    let server_path = get_server_path();
    let mut client = McpClient::start(&server_path)?;

    client.initialize()?;

    // Request tools list
    let request = JsonRpcRequest::new("tools/list")
        .with_id(2)
        .with_params(json!({}));

    let response = client.call(&request)?;

    assert!(response.result.is_some(), "tools/list should return result");
    let result = response.result.unwrap();

    // Check tools array
    let tools = result["tools"]
        .as_array()
        .expect("tools should be an array");

    assert_eq!(tools.len(), 2, "Should have 2 tools");

    // Check tool names
    let tool_names: Vec<&str> = tools
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();

    assert!(
        tool_names.contains(&"text_to_speech"),
        "Should have text_to_speech tool"
    );
    assert!(
        tool_names.contains(&"list_voice_styles"),
        "Should have list_voice_styles tool"
    );

    Ok(())
}

#[test]
fn test_invalid_tool_call() -> Result<()> {
    let server_path = get_server_path();
    let mut client = McpClient::start(&server_path)?;

    client.initialize()?;

    // Call non-existent tool
    let request = JsonRpcRequest::new("tools/call").with_id(3).with_params(
        json!({
            "name": "nonexistent_tool",
            "arguments": {}
        }),
    );

    let response = client.call(&request)?;

    // Should get error or error result
    let is_error = response.error.is_some()
        || (response.result.is_some()
            && response.result.as_ref().unwrap()["isError"]
                .as_bool()
                .unwrap_or(false));

    assert!(is_error, "Invalid tool should return error");

    Ok(())
}

#[test]
fn test_list_voice_styles() -> Result<()> {
    let server_path = get_server_path();
    let mut client = McpClient::start(&server_path)?;

    client.initialize()?;

    let request = JsonRpcRequest::new("tools/call").with_id(4).with_params(
        json!({
            "name": "list_voice_styles",
            "arguments": {}
        }),
    );

    let response = client.call(&request)?;

    assert!(
        response.result.is_some(),
        "list_voice_styles should return result"
    );

    // Result may be error if daemon not running, but should be valid response
    let result = response.result.unwrap();
    assert!(
        result.is_object(),
        "Result should be an object"
    );

    Ok(())
}

#[test]
fn test_parameter_validation_empty_text() -> Result<()> {
    let server_path = get_server_path();
    let mut client = McpClient::start(&server_path)?;

    client.initialize()?;

    let request = JsonRpcRequest::new("tools/call").with_id(5).with_params(
        json!({
            "name": "text_to_speech",
            "arguments": {
                "text": "",
                "style_id": 3,
                "rate": 1.0,
                "streaming": false
            }
        }),
    );

    let response = client.call(&request)?;

    // Should get error for empty text
    let has_error = response.error.is_some()
        || (response.result.is_some()
            && response.result.as_ref().unwrap()["isError"]
                .as_bool()
                .unwrap_or(false));

    assert!(has_error, "Empty text should be rejected");

    Ok(())
}

#[test]
fn test_parameter_validation_invalid_rate() -> Result<()> {
    let server_path = get_server_path();
    let mut client = McpClient::start(&server_path)?;

    client.initialize()?;

    let request = JsonRpcRequest::new("tools/call").with_id(6).with_params(
        json!({
            "name": "text_to_speech",
            "arguments": {
                "text": "テスト",
                "style_id": 3,
                "rate": 5.0,  // Invalid: must be 0.5-2.0
                "streaming": false
            }
        }),
    );

    let response = client.call(&request)?;

    // Should get error for invalid rate
    let has_error = response.error.is_some()
        || (response.result.is_some()
            && response.result.as_ref().unwrap()["isError"]
                .as_bool()
                .unwrap_or(false));

    assert!(has_error, "Invalid rate should be rejected");

    Ok(())
}
