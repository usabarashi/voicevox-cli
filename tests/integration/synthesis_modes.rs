mod common;

use anyhow::Result;
use common::{get_server_path, is_daemon_running, JsonRpcRequest, McpClient};
use serde_json::json;

#[test]
#[ignore = "requires daemon running"]
fn test_daemon_mode_synthesis() -> Result<()> {
    if !is_daemon_running() {
        eprintln!("Skipping: daemon not running");
        eprintln!("Start with: ./target/debug/voicevox-daemon --start --detach");
        return Ok(());
    }

    let server_path = get_server_path();
    let mut client = McpClient::start(&server_path)?;

    client.initialize()?;

    let request = JsonRpcRequest::new("tools/call").with_id(2).with_params(
        json!({
            "name": "text_to_speech",
            "arguments": {
                "text": "デーモンモードのテストなのだ",
                "style_id": 3,
                "rate": 1.0,
                "streaming": false
            }
        }),
    );

    let response = client.call(&request)?;

    assert!(
        response.result.is_some(),
        "Daemon synthesis should return result"
    );

    let result = response.result.unwrap();
    let is_error = result["isError"].as_bool().unwrap_or(false);

    if is_error {
        let error_msg = result["content"][0]["text"].as_str().unwrap_or("Unknown error");
        panic!("Daemon synthesis failed: {}", error_msg);
    }

    // Check success message contains expected info
    let success_msg = result["content"][0]["text"].as_str().unwrap();
    assert!(
        success_msg.contains("Successfully synthesized"),
        "Should contain success message"
    );
    assert!(
        success_msg.contains("audio size:"),
        "Should mention audio size for daemon mode"
    );

    Ok(())
}

#[test]
#[ignore = "requires daemon running and plays audio"]
fn test_streaming_mode_synthesis() -> Result<()> {
    if !is_daemon_running() {
        eprintln!("Skipping: daemon not running");
        return Ok(());
    }

    let server_path = get_server_path();
    let mut client = McpClient::start(&server_path)?;

    client.initialize()?;

    let request = JsonRpcRequest::new("tools/call").with_id(3).with_params(
        json!({
            "name": "text_to_speech",
            "arguments": {
                "text": "ストリーミングモードのテストなのだ",
                "style_id": 3,
                "rate": 1.0,
                "streaming": true
            }
        }),
    );

    let response = client.call(&request)?;

    assert!(
        response.result.is_some(),
        "Streaming synthesis should return result"
    );

    let result = response.result.unwrap();
    let is_error = result["isError"].as_bool().unwrap_or(false);

    if is_error {
        let error_msg = result["content"][0]["text"].as_str().unwrap_or("Unknown error");
        panic!("Streaming synthesis failed: {}", error_msg);
    }

    // Check success message mentions streaming
    let success_msg = result["content"][0]["text"].as_str().unwrap();
    assert!(
        success_msg.contains("streaming mode"),
        "Should mention streaming mode"
    );

    Ok(())
}

#[test]
fn test_synthesis_without_daemon() -> Result<()> {
    // This test checks behavior when daemon is not available
    // Streaming mode should work without daemon

    let server_path = get_server_path();
    let mut client = McpClient::start(&server_path)?;

    client.initialize()?;

    let request = JsonRpcRequest::new("tools/call").with_id(4).with_params(
        json!({
            "name": "text_to_speech",
            "arguments": {
                "text": "短いテスト",
                "style_id": 3,
                "rate": 1.0,
                "streaming": true
            }
        }),
    );

    let response = client.call(&request)?;

    assert!(
        response.result.is_some(),
        "Should return result even without daemon"
    );

    // Result may be success (streaming works) or error (daemon needed)
    // Both are valid responses
    let result = response.result.unwrap();
    assert!(result.is_object(), "Result should be an object");

    Ok(())
}

#[test]
#[ignore = "requires daemon running"]
fn test_different_voice_styles() -> Result<()> {
    if !is_daemon_running() {
        eprintln!("Skipping: daemon not running");
        return Ok(());
    }

    let server_path = get_server_path();
    let mut client = McpClient::start(&server_path)?;

    client.initialize()?;

    // Test different style IDs
    for (style_id, style_name) in [(3, "ノーマル"), (1, "あまあま"), (22, "ささやき")] {
        let request = JsonRpcRequest::new("tools/call")
            .with_id(10 + style_id as u64)
            .with_params(json!({
                "name": "text_to_speech",
                "arguments": {
                    "text": format!("{}のテストなのだ", style_name),
                    "style_id": style_id,
                    "rate": 1.0,
                    "streaming": false
                }
            }));

        let response = client.call(&request)?;

        assert!(
            response.result.is_some(),
            "Style ID {} should work",
            style_id
        );

        let result = response.result.unwrap();
        let is_error = result["isError"].as_bool().unwrap_or(false);

        assert!(
            !is_error,
            "Style ID {} should not error: {:?}",
            style_id,
            result["content"]
        );
    }

    Ok(())
}

#[test]
#[ignore = "requires daemon running"]
fn test_different_speech_rates() -> Result<()> {
    if !is_daemon_running() {
        eprintln!("Skipping: daemon not running");
        return Ok(());
    }

    let server_path = get_server_path();
    let mut client = McpClient::start(&server_path)?;

    client.initialize()?;

    // Test different rates
    for rate in [0.5, 1.0, 1.5, 2.0] {
        let request = JsonRpcRequest::new("tools/call")
            .with_id(20)
            .with_params(json!({
                "name": "text_to_speech",
                "arguments": {
                    "text": "速度テストなのだ",
                    "style_id": 3,
                    "rate": rate,
                    "streaming": false
                }
            }));

        let response = client.call(&request)?;

        let result = response.result.expect("Should have result");
        let is_error = result["isError"].as_bool().unwrap_or(false);

        assert!(
            !is_error,
            "Rate {} should not error: {:?}",
            rate,
            result["content"]
        );
    }

    Ok(())
}
