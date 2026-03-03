use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::Value;

use super::types::{ToolCallResult, text_result};
use crate::domain::voice::{
    ListVoiceStylesFilter, SpeakerStyles, VoiceStyle, filter_speakers, normalized_filters,
};
use crate::infrastructure::daemon::client::DaemonClient;
use crate::interface::synthesis::flow::connect_daemon_client_auto_start;

async fn connect_daemon_client_for_tool() -> Result<DaemonClient> {
    let socket_path = crate::infrastructure::paths::get_socket_path();
    connect_daemon_client_auto_start(&socket_path)
        .await
        .context("Failed to connect to VOICEVOX daemon")
}

#[derive(Debug, Deserialize)]
struct ListVoiceStylesParams {
    speaker_name: Option<String>,
    style_name: Option<String>,
}

fn render_voice_styles_result(filtered_results: &[SpeakerStyles]) -> String {
    if filtered_results.is_empty() {
        return "No speakers found matching the criteria.".to_string();
    }

    let blocks = filtered_results
        .iter()
        .map(|speaker| {
            let style_lines = speaker
                .styles
                .iter()
                .map(|style| format!("  - {} (ID: {})", style.name, style.id))
                .collect::<Vec<_>>()
                .join("\n");
            format!("Speaker: {}\nStyles:\n{style_lines}", speaker.speaker_name)
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    format!("{blocks}\nTotal speakers found: {}", filtered_results.len())
}

/// Executes the `list_voice_styles` tool with optional speaker/style filters.
///
/// # Errors
///
/// Returns an error if parameters are invalid or the daemon cannot be contacted.
pub async fn handle_voice_style_list_tool(arguments: Value) -> Result<ToolCallResult> {
    let params: ListVoiceStylesParams =
        serde_json::from_value(arguments).context("Invalid parameters for list_voice_styles")?;
    let filter = ListVoiceStylesFilter {
        speaker_name: params.speaker_name,
        style_name: params.style_name,
    };

    let mut client = connect_daemon_client_for_tool().await?;
    let speakers = client
        .list_speakers()
        .await?
        .into_iter()
        .map(|speaker| SpeakerStyles {
            speaker_name: speaker.name.to_string(),
            styles: speaker
                .styles
                .into_iter()
                .map(|style| VoiceStyle {
                    name: style.name.to_string(),
                    id: style.id,
                })
                .collect(),
        })
        .collect::<Vec<_>>();

    let (speaker_name_filter, style_name_filter) = normalized_filters(&filter);
    let filtered_results = filter_speakers(
        speakers,
        speaker_name_filter.as_deref(),
        style_name_filter.as_deref(),
    );

    let result_text = render_voice_styles_result(&filtered_results);
    Ok(text_result(result_text, false))
}
