use crate::domain::text_to_speech::params as tts_params;
use crate::domain::text_to_speech::params::SynthesizeParams;
use crate::interface::mcp_server::tools::handle_text_to_speech;
use serde_json::{json, Value};

#[allow(clippy::future_not_send)]
async fn assert_tts_error_contains(args: Value, expected: &str) {
    let error_text = match handle_text_to_speech(args).await {
        Ok(result) => panic!("expected error, got success: {result:?}"),
        Err(error) => error.to_string(),
    };

    assert!(
        error_text.contains(expected),
        "expected error containing '{expected}', got '{error_text}'"
    );
}

#[tokio::test]
async fn test_text_to_speech_empty_text() {
    let args = json!({
        "text": "",
        "style_id": 3,
        "streaming": false
    });

    assert_tts_error_contains(args, "Text cannot be empty").await;
}

#[tokio::test]
async fn test_text_to_speech_text_too_long() {
    let long_text = "あ".repeat(10_001);
    let args = json!({
        "text": long_text,
        "style_id": 3,
        "streaming": false
    });

    assert_tts_error_contains(args, "Text too long").await;
}

#[tokio::test]
async fn test_text_to_speech_invalid_rate() {
    let args = json!({
        "text": "テスト",
        "style_id": 3,
        "rate": 3.0,
        "streaming": false
    });

    assert_tts_error_contains(args, "Rate must be between 0.5 and 2.0").await;
}

#[tokio::test]
async fn test_text_to_speech_invalid_style_id() {
    let args = json!({
        "text": "テスト",
        "style_id": tts_params::MAX_STYLE_ID + 1,
        "streaming": false
    });

    assert_tts_error_contains(args, "Invalid style_id").await;
}

#[test]
fn test_validate_synthesize_params_char_limit_uses_character_count() {
    let params = SynthesizeParams {
        text: "あ".repeat(tts_params::MAX_TEXT_LENGTH),
        style_id: 3,
        rate: 1.0,
        streaming: false,
    };

    let result = tts_params::validate_synthesize_params(&params);
    assert!(
        result.is_ok(),
        "expected char-limit boundary to pass: {result:?}"
    );
}
