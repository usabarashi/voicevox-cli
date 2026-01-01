use anyhow::{Context as AnyhowContext, Result};
use rmcp::model::*;
use rmcp::service::{NotificationContext, Peer, RequestContext, RoleServer};
use rmcp::{ErrorData as McpError, ServerHandler};
use rodio::Sink;
use schemars::JsonSchema;
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::client::{audio::play_audio_from_memory, DaemonClient};
use crate::synthesis::StreamingSynthesizer;

const MAX_STYLE_ID: u32 = 1000;
const MAX_TEXT_LENGTH: usize = 10_000;

// Note: We attempted to use custom extractors with #[tool] macro,
// but rmcp 0.8 doesn't expose FromToolCallContextPart in a compatible way.
// Therefore, we use manual tool routing in ServerHandler::call_tool instead.
// This gives us direct access to RequestContext for each tool call.

/// VOICEVOX MCP Service providing text-to-speech tools
#[derive(Clone)]
pub struct VoicevoxService {
    // No state needed - stateless service
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

// Old #[tool_router] impl block removed - using manual routing in ServerHandler instead
// This gives us direct access to RequestContext for progress notifications and cancellation

impl VoicevoxService {
    pub fn new() -> Self {
        Self {}
    }
}

impl VoicevoxService {
    /// text_to_speech tool implementation with RequestContext access
    async fn text_to_speech_impl(
        &self,
        arguments: serde_json::Value,
        context: &RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        // Parse parameters
        let params: TextToSpeechParams = serde_json::from_value(arguments)
            .map_err(|e| McpError::invalid_params(format!("Invalid parameters: {}", e), None))?;

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

        // Execute synthesis with context
        let result = if params.streaming {
            self.handle_streaming_synthesis_with_context(params, context)
                .await
        } else {
            // For non-streaming, we don't have cancellation support yet
            self.handle_daemon_synthesis(params).await
        };

        match result {
            Ok(msg) => Ok(CallToolResult::success(vec![Content::text(msg)])),
            Err(e) if e.to_string().contains("cancelled") => {
                Ok(CallToolResult::error(vec![Content::text(
                    "Synthesis cancelled by user".to_string(),
                )]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(format!(
                "Synthesis failed: {}",
                e
            ))])),
        }
    }

    /// list_voice_styles tool implementation
    async fn list_voice_styles_impl(
        &self,
        arguments: serde_json::Value,
    ) -> Result<CallToolResult, McpError> {
        // Parse parameters
        let params: ListVoiceStylesParams = serde_json::from_value(arguments)
            .map_err(|e| McpError::invalid_params(format!("Invalid parameters: {}", e), None))?;

        let result = self.handle_list_voice_styles(params).await;

        match result {
            Ok(text) => Ok(CallToolResult::success(vec![Content::text(text)])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(format!(
                "Failed to list voice styles: {}",
                e
            ))])),
        }
    }

    /// Handle streaming synthesis with cancellation and progress support
    async fn handle_streaming_synthesis_with_context(
        &self,
        params: TextToSpeechParams,
        ctx: &RequestContext<RoleServer>,
    ) -> Result<String> {
        let cancel_token = ctx.ct.clone();
        let progress_token = ctx.meta.get_progress_token();
        let peer = ctx.peer.clone();

        self.handle_streaming_synthesis_cancellable(params, cancel_token, peer, progress_token)
            .await
    }

    /// Handle streaming synthesis with cancellation and progress (implementation)
    async fn handle_streaming_synthesis_cancellable(
        &self,
        params: TextToSpeechParams,
        cancel_token: CancellationToken,
        peer: Peer<RoleServer>,
        progress_token: Option<ProgressToken>,
    ) -> Result<String> {
        let text_len = params.text.len();
        let style_id = params.style_id;

        // Channel for progress updates from blocking context
        let (progress_tx, mut progress_rx) =
            mpsc::channel::<(f64, Option<f64>, Option<String>)>(32);

        // Clone for use in blocking task
        let progress_tx_clone = progress_tx.clone();

        // Spawn task to forward progress notifications
        let progress_peer = peer.clone();
        let progress_token_clone = progress_token.clone();
        let progress_forwarder = tokio::spawn(async move {
            if let Some(token) = progress_token_clone {
                while let Some((current, total, message)) = progress_rx.recv().await {
                    let _ = progress_peer
                        .notify_progress(ProgressNotificationParam {
                            progress_token: token.clone(),
                            progress: current,
                            total,
                            message,
                        })
                        .await;
                }
            }
        });

        // Spawn blocking task for audio
        tokio::task::spawn_blocking(move || -> Result<()> {
            let progress_tx = progress_tx_clone;
            let runtime = tokio::runtime::Runtime::new()
                .context("Failed to create runtime for audio playback")?;

            let stream = rodio::OutputStreamBuilder::open_default_stream()
                .context("Failed to create audio output stream")?;
            let sink = Arc::new(Sink::connect_new(stream.mixer()));

            let mut synthesizer = runtime
                .block_on(StreamingSynthesizer::new())
                .context("Failed to create streaming synthesizer")?;

            // Synthesize with progress and cancellation
            let cancelled = runtime
                .block_on(async {
                    synthesizer
                        .synthesize_streaming_with_cancellation(
                            &params.text,
                            params.style_id,
                            params.rate,
                            &sink,
                            &cancel_token,
                            Some(&progress_tx),
                        )
                        .await
                })
                .context("Streaming synthesis failed")?;

            if cancelled {
                return Err(anyhow::anyhow!("Synthesis cancelled by user"));
            }

            // Wait for playback with cancellation support and progress updates
            runtime.block_on(async {
                const POLL_INTERVAL: Duration = Duration::from_millis(100);
                const PROGRESS_UPDATE_INTERVAL: Duration = Duration::from_secs(1);

                let mut last_progress_update = std::time::Instant::now();

                loop {
                    if cancel_token.is_cancelled() {
                        sink.stop();
                        return Err(anyhow::anyhow!("Playback cancelled by user"));
                    }

                    if sink.empty() {
                        return Ok(());
                    }

                    // Send periodic progress updates during playback to prevent timeout
                    if last_progress_update.elapsed() >= PROGRESS_UPDATE_INTERVAL {
                        let _ = progress_tx
                            .send((100.0, Some(100.0), Some("Reading aloud...".to_string())))
                            .await;
                        last_progress_update = std::time::Instant::now();
                    }

                    tokio::time::sleep(POLL_INTERVAL).await;
                }
            })
        })
        .await
        .context("Audio playback task failed")??;

        // progress_tx was moved into spawn_blocking, drop happens automatically
        // Wait for progress forwarder to finish
        let _ = progress_forwarder.await;

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

        // Pre-convert filter strings to lowercase to avoid repeated conversions
        let speaker_filter_lower = params.speaker_name.as_ref().map(|s| s.to_lowercase());
        let style_filter_lower = params.style_name.as_ref().map(|s| s.to_lowercase());

        let mut result_text = String::new();
        let mut speaker_count = 0;

        for speaker in speakers {
            // Filter by speaker name
            if let Some(ref filter) = speaker_filter_lower {
                if !speaker.name.to_lowercase().contains(filter) {
                    continue;
                }
            }

            // Filter styles
            let filtered_styles: Vec<_> = if let Some(ref filter) = style_filter_lower {
                speaker
                    .styles
                    .into_iter()
                    .filter(|style| style.name.to_lowercase().contains(filter))
                    .collect()
            } else {
                speaker.styles.into_iter().collect()
            };

            if !filtered_styles.is_empty() {
                result_text.push_str(&format!("Speaker: {}\n", speaker.name));
                result_text.push_str("Styles:\n");
                for style in filtered_styles {
                    result_text.push_str(&format!("  - {} (ID: {})\n", style.name, style.id));
                }
                result_text.push('\n');
                speaker_count += 1;
            }
        }

        if speaker_count == 0 {
            result_text.push_str("No speakers found matching the criteria.");
        } else {
            result_text.push_str(&format!("Total speakers found: {}", speaker_count));
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

// Manual ServerHandler implementation without #[rmcp::tool_handler] macro
// This allows us to access RequestContext for progress notifications and cancellation
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

    async fn call_tool(
        &self,
        request: CallToolRequestParam,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        // Convert arguments from Option<Map> to Value
        let arguments = request
            .arguments
            .map_or_else(|| serde_json::json!({}), serde_json::Value::Object);

        match request.name.as_ref() {
            "text_to_speech" => self.text_to_speech_impl(arguments, &context).await,
            "list_voice_styles" => self.list_voice_styles_impl(arguments).await,
            _ => Err(McpError::invalid_request(
                format!("Unknown tool: {}", request.name),
                None,
            )),
        }
    }

    async fn list_tools(
        &self,
        _pagination: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        use std::sync::Arc;

        let text_to_speech_schema = serde_json::json!({
            "type": "object",
            "properties": {
                "text": {
                    "type": "string",
                    "description": "Japanese text to synthesize (15-50 chars optimal, 100+ may need splitting)"
                },
                "style_id": {
                    "type": "integer",
                    "description": "Voice style ID (3=normal, 1=happy, 22=whisper, 76=sad, 75=confused)"
                },
                "rate": {
                    "type": "number",
                    "minimum": 0.5,
                    "maximum": 2.0,
                    "default": 1.0,
                    "description": "Speech rate (0.5-2.0, default 1.0)"
                },
                "streaming": {
                    "type": "boolean",
                    "default": true,
                    "description": "Enable streaming mode for lower latency (default true)"
                }
            },
            "required": ["text", "style_id"]
        });
        let text_to_speech_map = text_to_speech_schema.as_object().unwrap().clone();

        let list_voice_styles_schema = serde_json::json!({
            "type": "object",
            "properties": {
                "speaker_name": {
                    "type": "string",
                    "description": "Filter by speaker name (partial match)"
                },
                "style_name": {
                    "type": "string",
                    "description": "Filter by style name (partial match)"
                }
            }
        });
        let list_voice_styles_map = list_voice_styles_schema.as_object().unwrap().clone();

        Ok(ListToolsResult {
            tools: vec![
                Tool {
                    name: "text_to_speech".into(),
                    description: Some("Convert Japanese text to speech with VOICEVOX. Supports progress notifications for long text synthesis and cancellation via Ctrl+C. Splits long messages automatically for client compatibility.".into()),
                    input_schema: Arc::new(text_to_speech_map),
                    title: None,
                    output_schema: None,
                    icons: None,
                    annotations: None,
                },
                Tool {
                    name: "list_voice_styles".into(),
                    description: Some("Get available VOICEVOX voice styles for text_to_speech. Use this before synthesizing speech to discover available style_ids and their characteristics. Filter by speaker_name or style_name (e.g., 'ノーマル', 'ささやき', 'なみだめ') to find appropriate voices. Returns style_id, speaker name, and style type for each voice.".into()),
                    input_schema: Arc::new(list_voice_styles_map),
                    title: None,
                    output_schema: None,
                    icons: None,
                    annotations: None,
                },
            ],
            next_cursor: None,
        })
    }

    async fn on_cancelled(
        &self,
        notification: CancelledNotificationParam,
        _context: NotificationContext<RoleServer>,
    ) {
        // CancellationToken in RequestContext is automatically cancelled by rmcp framework
        // This handler is primarily for logging
        eprintln!("Request {:?} cancelled by client", notification.request_id);
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
