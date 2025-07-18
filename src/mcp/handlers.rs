use anyhow::{anyhow, Context, Result};
use futures_util::{SinkExt, StreamExt};
use rodio::{OutputStream, Sink};
use serde::Deserialize;
use serde_json::Value;
use tokio::net::UnixStream;
use tokio::time::{timeout, Duration as TokioDuration};
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

use crate::client::{audio::play_audio_from_memory, DaemonClient};
use crate::ipc::{DaemonRequest, OwnedResponse};
use crate::mcp::types::{ToolCallResult, ToolContent};
use crate::paths::get_socket_path;
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

pub async fn handle_text_to_speech(arguments: Value) -> Result<ToolCallResult> {
    let params: SynthesizeParams =
        serde_json::from_value(arguments).context("Invalid parameters for text_to_speech")?;

    if params.text.trim().is_empty() {
        return Err(anyhow!("Text cannot be empty"));
    }

    if params.rate < 0.5 || params.rate > 2.0 {
        return Err(anyhow!("Rate must be between 0.5 and 2.0"));
    }

    if params.streaming {
        let (_stream, stream_handle) =
            OutputStream::try_default().context("Failed to create audio output stream")?;
        let sink = Sink::try_new(&stream_handle).context("Failed to create audio sink")?;

        let mut synthesizer = StreamingSynthesizer::new()
            .await
            .context("Failed to create streaming synthesizer")?;

        synthesizer
            .synthesize_streaming(&params.text, params.style_id, &sink)
            .await
            .context("Streaming synthesis failed")?;

        sink.sleep_until_end();
        drop(_stream);

        Ok(ToolCallResult {
            content: vec![ToolContent {
                content_type: "text".to_string(),
                text: format!(
                    "Successfully synthesized {} characters using style ID {} in streaming mode",
                    params.text.len(),
                    params.style_id
                ),
            }],
            is_error: Some(false),
        })
    } else {
        let mut client = match DaemonClient::new().await {
            Ok(client) => client,
            Err(e) => {
                return Ok(ToolCallResult {
                    content: vec![ToolContent {
                        content_type: "text".to_string(),
                        text: format!("Failed to connect to VOICEVOX daemon: {e}"),
                    }],
                    is_error: Some(true),
                });
            }
        };

        let wav_data = client
            .synthesize(&params.text, params.style_id)
            .await
            .context("Synthesis failed")?;

        play_audio_from_memory(&wav_data).context("Failed to play audio")?;

        Ok(ToolCallResult {
            content: vec![ToolContent {
                content_type: "text".to_string(),
                text: format!(
                    "Successfully synthesized {} characters using style ID {} (audio size: {} bytes)",
                    params.text.len(),
                    params.style_id,
                    wav_data.len()
                ),
            }],
            is_error: Some(false),
        })
    }
}

pub async fn handle_get_voices(arguments: Value) -> Result<ToolCallResult> {
    let params: GetVoicesParams =
        serde_json::from_value(arguments).context("Invalid parameters for get_voices")?;

    let socket_path = get_socket_path();
    let connect_timeout = TokioDuration::from_secs(5);
    let stream = timeout(connect_timeout, UnixStream::connect(&socket_path))
        .await
        .map_err(|_| anyhow!("Connection timeout after 5 seconds"))?
        .context("Failed to connect to VOICEVOX daemon. Is it running?")?;

    let (reader, writer) = stream.into_split();
    let mut framed_reader = FramedRead::new(reader, LengthDelimitedCodec::new());
    let mut framed_writer = FramedWrite::new(writer, LengthDelimitedCodec::new());

    let request = DaemonRequest::ListSpeakers;
    let request_data = bincode::serialize(&request)?;
    framed_writer.send(request_data.into()).await?;

    let speakers = if let Some(response_frame) = framed_reader.next().await {
        let response_frame = response_frame?;
        let response: OwnedResponse = bincode::deserialize(&response_frame)?;

        match response {
            OwnedResponse::SpeakersList { speakers } => speakers.into_owned(),
            OwnedResponse::SpeakersListWithModels { speakers, .. } => speakers.into_owned(),
            OwnedResponse::Error { message } => {
                return Err(anyhow!("Daemon error: {message}"));
            }
            _ => {
                return Err(anyhow!("Unexpected response from daemon"));
            }
        }
    } else {
        return Err(anyhow!("No response from daemon"));
    };

    let filtered_speakers: Vec<Speaker> = speakers
        .into_iter()
        .filter(|speaker| match &params.speaker_name {
            Some(name_filter) => speaker
                .name
                .to_lowercase()
                .contains(&name_filter.to_lowercase()),
            None => true,
        })
        .filter(|speaker| match &params.style_name {
            Some(style_filter) => speaker.styles.iter().any(|style| {
                style
                    .name
                    .to_lowercase()
                    .contains(&style_filter.to_lowercase())
            }),
            None => true,
        })
        .collect();

    let mut result_text = String::new();
    if filtered_speakers.is_empty() {
        result_text.push_str("No speakers found matching the criteria.");
    } else {
        for speaker in &filtered_speakers {
            result_text.push_str(&format!("Speaker: {}\n", speaker.name));
            result_text.push_str("Styles:\n");
            for style in &speaker.styles {
                result_text.push_str(&format!("  - {} (ID: {})\n", style.name, style.id));
            }
            result_text.push('\n');
        }
        result_text.push_str(&format!(
            "Total speakers found: {}",
            filtered_speakers.len()
        ));
    }
    Ok(ToolCallResult {
        content: vec![ToolContent {
            content_type: "text".to_string(),
            text: result_text.trim().to_string(),
        }],
        is_error: Some(false),
    })
}
