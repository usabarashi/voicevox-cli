use anyhow::{anyhow, Context, Result};
use rodio::{OutputStream, Sink};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::client::{audio::play_audio_from_memory, DaemonClient};
use crate::synthesis::StreamingSynthesizer;
use crate::voice::Speaker;

#[derive(Debug, Deserialize)]
struct SynthesizeParams {
    text: String,
    style_id: u32,
    #[serde(default = "default_rate")]
    rate: f32,
    #[serde(default = "default_streaming")]
    streaming: bool,
}

fn default_rate() -> f32 {
    1.0
}

fn default_streaming() -> bool {
    true
}

#[derive(Debug, Deserialize)]
struct GetVoicesParams {
    speaker_name: Option<String>,
    style_name: Option<String>,
}

/// Handle text_to_speech tool call
pub async fn handle_text_to_speech(arguments: Value) -> Result<Value> {
    // Parse parameters
    let params: SynthesizeParams =
        serde_json::from_value(arguments).context("Invalid parameters for text_to_speech")?;

    // Validate parameters
    if params.text.trim().is_empty() {
        return Err(anyhow!("Text cannot be empty"));
    }

    if params.rate < 0.5 || params.rate > 2.0 {
        return Err(anyhow!("Rate must be between 0.5 and 2.0"));
    }

    if params.streaming {
        // Streaming playback

        // Create audio output - IMPORTANT: _stream must be kept alive
        let (_stream, stream_handle) =
            OutputStream::try_default().context("Failed to create audio output stream")?;
        let sink = Sink::try_new(&stream_handle).context("Failed to create audio sink")?;

        // Create streaming synthesizer
        let mut synthesizer = StreamingSynthesizer::new()
            .await
            .context("Failed to create streaming synthesizer")?;

        // Perform streaming synthesis
        synthesizer
            .synthesize_streaming(&params.text, params.style_id, &sink)
            .await
            .context("Streaming synthesis failed")?;

        // Wait for playback to complete
        sink.sleep_until_end();

        // Keep _stream alive until here
        drop(_stream);

        Ok(json!({
            "status": "success",
            "mode": "streaming",
            "text_length": params.text.len(),
            "style_id": params.style_id
        }))
    } else {
        // Non-streaming playback

        // Connect to daemon
        let mut client = DaemonClient::new()
            .await
            .context("Failed to connect to VOICEVOX daemon. Is it running?")?;

        // Synthesize
        let wav_data = client
            .synthesize(&params.text, params.style_id)
            .await
            .context("Synthesis failed")?;

        // Play audio
        play_audio_from_memory(&wav_data).context("Failed to play audio")?;

        Ok(json!({
            "status": "success",
            "mode": "non-streaming",
            "text_length": params.text.len(),
            "style_id": params.style_id,
            "audio_size": wav_data.len()
        }))
    }
}

/// Handle get_voices tool call
pub async fn handle_get_voices(arguments: Value) -> Result<Value> {
    // Parse parameters
    let params: GetVoicesParams =
        serde_json::from_value(arguments).context("Invalid parameters for get_voices")?;

    // Connect to daemon
    let mut client = DaemonClient::new()
        .await
        .context("Failed to connect to VOICEVOX daemon. Is it running?")?;

    // Get all speakers
    let speakers = client
        .list_speakers()
        .await
        .context("Failed to get speakers list")?;

    // Apply filters
    let filtered_speakers: Vec<Speaker> = speakers
        .into_iter()
        .filter(|speaker| {
            // Filter by speaker name
            if let Some(ref name_filter) = params.speaker_name {
                if !speaker
                    .name
                    .to_lowercase()
                    .contains(&name_filter.to_lowercase())
                {
                    return false;
                }
            }
            true
        })
        .filter(|speaker| {
            // Filter by style name
            if let Some(ref style_filter) = params.style_name {
                let has_matching_style = speaker.styles.iter().any(|style| {
                    style
                        .name
                        .to_lowercase()
                        .contains(&style_filter.to_lowercase())
                });
                if !has_matching_style {
                    return false;
                }
            }
            true
        })
        .collect();

    // Format response
    let response = json!({
        "speakers": filtered_speakers.iter().map(|speaker| {
            json!({
                "name": speaker.name,
                "speaker_uuid": speaker.speaker_uuid,
                "version": speaker.version,
                "styles": speaker.styles.iter().map(|style| {
                    json!({
                        "name": style.name,
                        "id": style.id,
                        "type": style.style_type
                    })
                }).collect::<Vec<_>>()
            })
        }).collect::<Vec<_>>(),
        "total_speakers": filtered_speakers.len(),
        "filters_applied": {
            "speaker_name": params.speaker_name,
            "style_name": params.style_name
        }
    });

    Ok(response)
}
