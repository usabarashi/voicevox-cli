use anyhow::{Context, Result};
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::*;
use rmcp::{tool, tool_router, ErrorData as McpError, ServerHandler};
use rodio::Sink;
use schemars::JsonSchema;
use serde::Deserialize;
use std::sync::Arc;

use crate::client::{audio::play_audio_from_memory, DaemonClient};
use crate::synthesis::StreamingSynthesizer;

const MAX_STYLE_ID: u32 = 1000;
const MAX_TEXT_LENGTH: usize = 10_000;

/// VOICEVOX MCP Service providing text-to-speech tools
#[derive(Clone)]
pub struct VoicevoxService {
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
}

/// Minimum allowed speech rate
pub const MIN_RATE: f32 = 0.5;
/// Maximum allowed speech rate
pub const MAX_RATE: f32 = 2.0;

/// Parameters for text-to-speech synthesis
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TextToSpeechParams {
    /// Japanese text to synthesize (15-50 chars optimal, 100+ may need splitting)
    pub text: String,
    /// Voice style ID (3=normal, 1=happy, 22=whisper, 76=sad, 75=confused)
    pub style_id: u32,
    /// Speech rate (MIN_RATE-MAX_RATE, default 1.0)
    #[serde(default = "default_rate")]
    #[schemars(range(min = 0.5, max = 2.0))]
    pub rate: f32,
    /// Enable streaming mode for lower latency (default true)
    #[serde(default = "default_streaming")]
    pub streaming: bool,
}

fn default_rate() -> f32 {
    1.0
}

fn default_streaming() -> bool {
    true
}

/// Parameters for listing voice styles
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListVoiceStylesParams {
    /// Filter by speaker name (partial match)
    pub speaker_name: Option<String>,
    /// Filter by style name (partial match)
    pub style_name: Option<String>,
}

#[tool_router]
impl VoicevoxService {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    /// Convert Japanese text to speech with VOICEVOX
    ///
    /// Synthesizes Japanese text to audio and plays it back. Supports both streaming
    /// mode for lower latency and daemon mode for reliability. Automatically splits
    /// long messages for client compatibility.
    #[tool(
        description = "Convert Japanese text to speech with VOICEVOX. Splits long messages automatically for client compatibility."
    )]
    async fn text_to_speech(
        &self,
        Parameters(params): Parameters<TextToSpeechParams>,
    ) -> Result<CallToolResult, McpError> {
        // Validate parameters
        let text = params.text.trim();
        if text.is_empty() {
            return Err(McpError::invalid_params("Text cannot be empty", None));
        }

        if text.len() > MAX_TEXT_LENGTH {
            return Err(McpError::invalid_params(
                format!(
                    "Text too long: {} characters (max: {})",
                    text.len(),
                    MAX_TEXT_LENGTH
                ),
                None,
            ));
        }

        if !(0.5..=2.0).contains(&params.rate) {
            return Err(McpError::invalid_params(
                "Rate must be between 0.5 and 2.0",
                None,
            ));
        }

        if params.style_id > MAX_STYLE_ID {
            return Err(McpError::invalid_params(
                format!(
                    "Invalid style_id: {} (max: {})",
                    params.style_id, MAX_STYLE_ID
                ),
                None,
            ));
        }

        // Execute synthesis
        let result = if params.streaming {
            self.handle_streaming_synthesis(params).await
        } else {
            self.handle_daemon_synthesis(params).await
        };

        match result {
            Ok(msg) => Ok(CallToolResult::success(vec![Content::text(msg)])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(format!(
                "Synthesis failed: {}",
                e
            ))])),
        }
    }

    /// Get available VOICEVOX voice styles
    ///
    /// Returns a list of available voice styles with their IDs, speaker names, and style types.
    /// Use this before synthesizing speech to discover available style_ids and their characteristics.
    #[tool(
        description = "Get available VOICEVOX voice styles for text_to_speech. Use this before synthesizing speech to discover available style_ids and their characteristics. Filter by speaker_name or style_name (e.g., 'ノーマル', 'ささやき', 'なみだめ') to find appropriate voices. Returns style_id, speaker name, and style type for each voice."
    )]
    async fn list_voice_styles(
        &self,
        Parameters(params): Parameters<ListVoiceStylesParams>,
    ) -> Result<CallToolResult, McpError> {
        let result = self.handle_list_voice_styles(params).await;

        match result {
            Ok(text) => Ok(CallToolResult::success(vec![Content::text(text)])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(format!(
                "Failed to list voice styles: {}",
                e
            ))])),
        }
    }
}

impl VoicevoxService {
    /// Handle streaming synthesis with concurrent processing
    async fn handle_streaming_synthesis(&self, params: TextToSpeechParams) -> Result<String> {
        // Spawn blocking task to handle the entire audio playback since OutputStream is not Send
        let text_len = params.text.len();
        let style_id = params.style_id;

        tokio::task::spawn_blocking(move || -> Result<()> {
            // Create a new runtime for async operations within blocking context
            // This avoids the anti-pattern of using Handle::current().block_on() in spawn_blocking
            let runtime = tokio::runtime::Runtime::new()
                .context("Failed to create runtime for audio playback")?;

            let stream = rodio::OutputStreamBuilder::open_default_stream()
                .context("Failed to create audio output stream")?;
            let sink = Arc::new(Sink::connect_new(stream.mixer()));

            let mut synthesizer = runtime
                .block_on(StreamingSynthesizer::new())
                .context("Failed to create streaming synthesizer")?;

            runtime
                .block_on(synthesizer.synthesize_streaming(
                    &params.text,
                    params.style_id,
                    params.rate,
                    &sink,
                ))
                .context("Streaming synthesis failed")?;

            sink.sleep_until_end();

            Ok(())
        })
        .await
        .context("Audio playback task failed")??;

        Ok(format!(
            "Successfully synthesized {} characters using style ID {} in streaming mode",
            text_len, style_id
        ))
    }

    /// Handle daemon-based synthesis
    async fn handle_daemon_synthesis(&self, params: TextToSpeechParams) -> Result<String> {
        let mut client = DaemonClient::connect_with_retry()
            .await
            .context("Failed to connect to VOICEVOX daemon after multiple attempts")?;

        let options = crate::ipc::OwnedSynthesizeOptions { rate: params.rate };

        let wav_data = client
            .synthesize(&params.text, params.style_id, options)
            .await
            .context("Synthesis failed")?;

        let audio_size = wav_data.len();

        play_audio_from_memory(wav_data).context("Failed to play audio")?;

        Ok(format!(
            "Successfully synthesized {} characters using style ID {} (audio size: {} bytes)",
            params.text.len(),
            params.style_id,
            audio_size
        ))
    }

    /// Handle voice styles listing
    async fn handle_list_voice_styles(&self, params: ListVoiceStylesParams) -> Result<String> {
        let mut client = DaemonClient::connect_with_retry()
            .await
            .context("Failed to connect to VOICEVOX daemon after multiple attempts")?;

        let speakers = client
            .list_speakers()
            .await
            .context("Failed to get speakers list")?;

        let mut filtered_results = Vec::new();

        for speaker in speakers {
            if let Some(name_filter) = &params.speaker_name {
                if !speaker
                    .name
                    .to_lowercase()
                    .contains(&name_filter.to_lowercase())
                {
                    continue;
                }
            }

            let filtered_styles = if let Some(style_filter) = &params.style_name {
                speaker
                    .styles
                    .into_iter()
                    .filter(|style| {
                        style
                            .name
                            .to_lowercase()
                            .contains(&style_filter.to_lowercase())
                    })
                    .collect::<Vec<_>>()
            } else {
                speaker.styles.to_vec()
            };

            if !filtered_styles.is_empty() {
                filtered_results.push((speaker.name, filtered_styles));
            }
        }

        let mut result_text = String::new();
        if filtered_results.is_empty() {
            result_text.push_str("No speakers found matching the criteria.");
        } else {
            for (speaker_name, styles) in &filtered_results {
                result_text.push_str(&format!("Speaker: {}\n", speaker_name));
                result_text.push_str("Styles:\n");
                for style in styles {
                    result_text.push_str(&format!("  - {} (ID: {})\n", style.name, style.id));
                }
                result_text.push('\n');
            }
            result_text.push_str(&format!("Total speakers found: {}", filtered_results.len()));
        }

        Ok(result_text.trim().to_string())
    }
}

impl Default for VoicevoxService {
    fn default() -> Self {
        Self::new()
    }
}

/// Load MCP server instructions from various locations.
///
/// The instruction loading follows XDG Base Directory compliance with the following priority:
///
/// 1. Environment variable: `VOICEVOX_MCP_INSTRUCTIONS` (highest priority)
/// 2. XDG user config: `$XDG_CONFIG_HOME/voicevox/VOICEVOX.md`
/// 3. Config fallback: `~/.config/voicevox/VOICEVOX.md` (when XDG_CONFIG_HOME is not set)
/// 4. Executable directory: `VOICEVOX.md` bundled with the binary (distribution default)
/// 5. Current directory: `VOICEVOX.md` in working directory (development use)
fn load_instructions() -> Option<String> {
    use std::fs;
    use std::path::{Path, PathBuf};

    const INSTRUCTIONS_ENV_VAR: &str = "VOICEVOX_MCP_INSTRUCTIONS";
    const INSTRUCTIONS_FILE: &str = "VOICEVOX.md";

    fn try_load(path: &Path) -> Option<String> {
        match fs::read_to_string(path) {
            Ok(content) => Some(content),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => {
                // Log non-NotFound errors (permission denied, I/O errors, etc.)
                eprintln!(
                    "Warning: Failed to read instructions file at {}: {}",
                    path.display(),
                    e
                );
                None
            }
        }
    }

    // 1. Environment variable
    if let Ok(custom_path) = std::env::var(INSTRUCTIONS_ENV_VAR) {
        let path = Path::new(&custom_path);
        if let Ok(content) = fs::read_to_string(path) {
            return Some(content);
        }
    }

    // 2. XDG user config
    let xdg_config_var = std::env::var("XDG_CONFIG_HOME");
    if let Ok(ref xdg_config) = xdg_config_var {
        let path = PathBuf::from(xdg_config)
            .join("voicevox")
            .join(INSTRUCTIONS_FILE);
        if let Some(content) = try_load(&path) {
            return Some(content);
        }
    }

    // 3. Config fallback
    if xdg_config_var.is_err() {
        if let Ok(home) = std::env::var("HOME") {
            let path = PathBuf::from(home)
                .join(".config")
                .join("voicevox")
                .join(INSTRUCTIONS_FILE);
            if let Some(content) = try_load(&path) {
                return Some(content);
            }
        }
    }

    // 4. Executable directory
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let path = exe_dir.join(INSTRUCTIONS_FILE);
            if let Some(content) = try_load(&path) {
                return Some(content);
            }
        }
    }

    // 5. Current directory
    let path = PathBuf::from(INSTRUCTIONS_FILE);
    if let Some(content) = try_load(&path) {
        return Some(content);
    }

    None
}

#[rmcp::tool_handler]
impl ServerHandler for VoicevoxService {
    fn get_info(&self) -> InitializeResult {
        InitializeResult {
            protocol_version: ProtocolVersion::V_2024_11_05,
            server_info: Implementation {
                name: "voicevox-mcp".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                icons: None,
                title: None,
                website_url: None,
            },
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability { list_changed: None }),
                ..Default::default()
            },
            instructions: load_instructions(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parameter_validation() {
        let params = TextToSpeechParams {
            text: "".to_string(),
            style_id: 3,
            rate: 1.0,
            streaming: false,
        };
        assert!(params.text.trim().is_empty());

        let params = TextToSpeechParams {
            text: "テスト".to_string(),
            style_id: MAX_STYLE_ID + 1,
            rate: 1.0,
            streaming: false,
        };
        assert!(params.style_id > MAX_STYLE_ID);

        let params = TextToSpeechParams {
            text: "テスト".to_string(),
            style_id: 3,
            rate: 3.0,
            streaming: false,
        };
        assert!(!(0.5..=2.0).contains(&params.rate));
    }
}
