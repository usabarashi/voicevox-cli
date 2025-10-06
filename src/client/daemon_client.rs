use anyhow::{anyhow, Result};
use futures_util::{SinkExt, StreamExt};
use std::path::PathBuf;
use std::time::Duration;
use tokio::net::UnixStream;
use tokio::process::Command;
use tokio::time::timeout;
use tokio_util::codec::{Framed, FramedRead, FramedWrite, LengthDelimitedCodec};

const DAEMON_CONNECTION_TIMEOUT: Duration = Duration::from_secs(2);
const DAEMON_RESPONSE_TIMEOUT: Duration = Duration::from_secs(30);
const DAEMON_STARTUP_MAX_RETRIES: u32 = 20;
const DAEMON_STARTUP_INITIAL_DELAY: Duration = Duration::from_millis(500);
const DAEMON_STARTUP_MAX_DELAY: Duration = Duration::from_secs(4);
const DAEMON_STARTUP_GRACE_PERIOD: Duration = Duration::from_millis(1000);
const DAEMON_FINAL_CONNECTION_TIMEOUT: Duration = Duration::from_secs(5);
const DAEMON_STARTUP_TOTAL_TIME_ESTIMATE: u32 = 80;

use crate::ipc::{DaemonRequest, OwnedRequest, OwnedResponse, OwnedSynthesizeOptions};
use crate::paths::get_socket_path;
use crate::voice::{AvailableModel, Speaker};

pub fn find_daemon_binary() -> Result<PathBuf, crate::daemon::DaemonError> {
    if let Ok(current_exe) = std::env::current_exe() {
        let mut daemon_path = current_exe.clone();
        daemon_path.set_file_name("voicevox-daemon");
        if daemon_path.exists() {
            return Ok(daemon_path);
        }
    }

    let fallbacks = vec![
        PathBuf::from("./target/debug/voicevox-daemon"),
        PathBuf::from("./target/release/voicevox-daemon"),
        PathBuf::from("voicevox-daemon"),
    ];

    fallbacks
        .into_iter()
        .find(|p| p.exists())
        .ok_or(crate::daemon::DaemonError::DaemonBinaryNotFound)
}

pub async fn daemon_mode(
    text: &str,
    style_id: u32,
    options: OwnedSynthesizeOptions,
    output_file: Option<&String>,
    quiet: bool,
    socket_path: &PathBuf,
) -> Result<()> {
    let mut stream = timeout(DAEMON_CONNECTION_TIMEOUT, UnixStream::connect(socket_path))
        .await
        .map_err(|_| anyhow!("Daemon connection timeout"))?
        .map_err(|e| anyhow!("Failed to connect to daemon: {e}"))?;

    let request = OwnedRequest::Synthesize {
        text: text.to_string(),
        style_id,
        options,
    };

    let request_data = bincode::serde::encode_to_vec(&request, bincode::config::standard())
        .map_err(|e| anyhow!("Failed to serialize request: {e}"))?;

    {
        let (_reader, writer) = stream.split();
        let mut framed_writer = FramedWrite::new(writer, LengthDelimitedCodec::new());
        framed_writer
            .send(request_data.into())
            .await
            .map_err(|e| anyhow!("Failed to send request: {e}"))?;
    }

    let response_frame = {
        let (reader, _writer) = stream.split();
        let mut framed_reader = FramedRead::new(reader, LengthDelimitedCodec::new());

        timeout(DAEMON_RESPONSE_TIMEOUT, framed_reader.next())
            .await
            .map_err(|_| anyhow!("Daemon response timeout"))?
            .ok_or_else(|| anyhow!("Connection closed by daemon"))?
            .map_err(|e| anyhow!("Failed to receive response: {e}"))?
    };

    let response: OwnedResponse =
        bincode::serde::decode_from_slice(&response_frame, bincode::config::standard())
            .map_err(|e| anyhow!("Failed to deserialize response: {e}"))?
            .0;

    match response {
        OwnedResponse::SynthesizeResult { wav_data } => {
            if let Some(output_file) = output_file {
                std::fs::write(output_file, &wav_data)?;
            }

            if !quiet && output_file.is_none() {
                crate::client::audio::play_audio_from_memory(wav_data)?;
            }
            Ok(())
        }
        OwnedResponse::Error { message } => Err(anyhow!("Daemon error: {message}")),
        _ => Err(anyhow!("Unexpected response from daemon")),
    }
}

pub async fn list_speakers_daemon(socket_path: &PathBuf) -> Result<()> {
    let stream = UnixStream::connect(socket_path).await?;
    let (reader, writer) = stream.into_split();
    let mut framed_reader = FramedRead::new(reader, LengthDelimitedCodec::new());
    let mut framed_writer = FramedWrite::new(writer, LengthDelimitedCodec::new());

    let request = DaemonRequest::ListSpeakers;
    let request_data = bincode::serde::encode_to_vec(&request, bincode::config::standard())?;
    framed_writer.send(request_data.into()).await?;

    if let Some(response_frame) = framed_reader.next().await {
        let response_frame = response_frame?;
        let response: OwnedResponse =
            bincode::serde::decode_from_slice(&response_frame, bincode::config::standard())?.0;

        match response {
            OwnedResponse::SpeakersList { speakers } => {
                println!("All available speakers and styles from daemon:");
                for speaker in &speakers {
                    println!("  {}", speaker.name);
                    for style in &speaker.styles {
                        println!("    {} (Style ID: {})", style.name, style.id);
                        if let Some(style_type) = &style.style_type {
                            println!("        Type: {style_type}");
                        }
                    }
                    println!();
                }
                return Ok(());
            }
            OwnedResponse::SpeakersListWithModels {
                speakers,
                style_to_model,
            } => {
                println!("All available speakers and styles from daemon:");
                for speaker in &speakers {
                    println!("  {}", speaker.name);
                    for style in &speaker.styles {
                        let model_id = style_to_model
                            .get(&style.id)
                            .map(|id| format!("{id}"))
                            .unwrap_or_else(|| "?".to_string());
                        println!(
                            "    {} (Model: {model_id}, Style ID: {})",
                            style.name, style.id
                        );
                        if let Some(style_type) = &style.style_type {
                            println!("        Type: {style_type}");
                        }
                    }
                    println!();
                }
                return Ok(());
            }
            OwnedResponse::Error { message } => {
                return Err(anyhow!("Daemon error: {message}"));
            }
            _ => {
                return Err(anyhow!("Unexpected response from daemon"));
            }
        }
    }

    Err(anyhow!("Failed to get speakers from daemon"))
}

async fn start_daemon_automatically() -> Result<()> {
    use std::io::Write;

    let socket_path = get_socket_path();
    let daemon_path = find_daemon_binary()?;

    println!("Starting VOICEVOX daemon (first startup may take a few seconds)...");
    println!("  Resources:");

    let onnx_path = crate::paths::find_onnxruntime()?;
    println!("    ONNX Runtime: {}", onnx_path.display());

    let dict_path = crate::paths::find_openjtalk_dict()?;
    println!("    OpenJTalk Dictionary: {}", dict_path.display());

    let models_dir = crate::paths::find_models_dir()?;
    let models = crate::voice::scan_available_models()?;
    println!(
        "    Voice Models: {} in {}",
        models.len(),
        models_dir.display()
    );

    println!("  Building voice model mappings (this may take a moment)...");

    print!("  Starting daemon process");
    std::io::stdout().flush()?;

    let output = Command::new(&daemon_path)
        .args(["--start", "--detach"])
        .output()
        .await;

    match output {
        Ok(output) => {
            if output.status.success() {
                let max_retries = DAEMON_STARTUP_MAX_RETRIES;
                let mut retry_delay = DAEMON_STARTUP_INITIAL_DELAY;

                for attempt in 0..max_retries {
                    match timeout(DAEMON_CONNECTION_TIMEOUT, UnixStream::connect(&socket_path))
                        .await
                    {
                        Ok(Ok(_)) => {
                            println!(" done!");
                            println!("VOICEVOX daemon started successfully");
                            return Ok(());
                        }
                        Ok(Err(_)) | Err(_) if attempt < max_retries - 1 => {
                            print!(".");
                            std::io::stdout().flush()?;
                            tokio::time::sleep(retry_delay).await;
                            retry_delay = (retry_delay * 2).min(DAEMON_STARTUP_MAX_DELAY);
                        }
                        Ok(Err(_)) | Err(_) => {}
                    }
                }

                Err(anyhow!(
                    "Daemon not responding after {} attempts (~{}s total)",
                    max_retries,
                    DAEMON_STARTUP_TOTAL_TIME_ESTIMATE
                ))
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(anyhow!("Daemon failed to start: {}", stderr.trim()))
            }
        }
        Err(e) => Err(anyhow!("Failed to execute daemon: {e}")),
    }
}

pub struct DaemonClient {
    stream: UnixStream,
}

impl DaemonClient {
    pub async fn new() -> Result<Self> {
        let socket_path = get_socket_path();
        let stream = UnixStream::connect(&socket_path).await.map_err(|e| {
            anyhow!(
                "Failed to connect to daemon at {}: {e}",
                socket_path.display()
            )
        })?;

        Ok(Self { stream })
    }

    pub async fn connect_with_retry() -> Result<Self> {
        use crate::daemon::startup;

        let mut last_error = None;
        let mut retry_delay = startup::initial_retry_delay();

        for attempt in 0..startup::MAX_CONNECT_ATTEMPTS {
            match Self::new().await {
                Ok(client) => return Ok(client),
                Err(e) => {
                    last_error = Some(e);
                    if attempt < startup::MAX_CONNECT_ATTEMPTS - 1 {
                        tokio::time::sleep(retry_delay).await;
                        retry_delay = (retry_delay * 2).min(startup::max_retry_delay());
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            anyhow!(
                "Failed to connect to daemon after {} attempts",
                startup::MAX_CONNECT_ATTEMPTS
            )
        }))
    }

    /// Creates a new DaemonClient with automatic daemon startup if not running.
    ///
    /// This method attempts to connect to an existing daemon first. If the connection
    /// fails, it checks for available models and automatically starts the daemon
    /// without user interaction.
    ///
    /// # Returns
    ///
    /// * `Ok(DaemonClient)` - Successfully connected to daemon (existing or newly started)
    /// * `Err` - No models available or daemon startup failed
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// * No VOICEVOX models are found in the models directory
    /// * The daemon fails to start
    /// * The daemon starts but doesn't respond to connections
    ///
    /// # Note
    ///
    /// This is a non-interactive method suitable for use in automated environments
    /// like MCP servers or streaming synthesizers. For interactive CLI use, consider
    /// using `new()` with appropriate user prompts.
    pub async fn new_with_auto_start() -> Result<Self> {
        let socket_path = get_socket_path();
        match timeout(DAEMON_CONNECTION_TIMEOUT, UnixStream::connect(&socket_path)).await {
            Ok(Ok(stream)) => Ok(Self { stream }),
            Ok(Err(_)) | Err(_) => {
                crate::voice::has_available_models()
                    .then_some(())
                    .ok_or_else(|| anyhow!(
                        "No VOICEVOX models found. Please download models first using 'voicevox-cli download' or place .vvm files in the models directory."
                    ))?;
                start_daemon_automatically().await?;

                tokio::time::sleep(DAEMON_STARTUP_GRACE_PERIOD).await;

                let stream = timeout(
                    DAEMON_FINAL_CONNECTION_TIMEOUT,
                    UnixStream::connect(&socket_path),
                )
                .await
                .map_err(|_| anyhow!("Timeout connecting to daemon"))?
                .map_err(|e| {
                    anyhow!(
                        "Daemon started but failed to connect at {}: {e}",
                        socket_path.display()
                    )
                })?;
                Ok(Self { stream })
            }
        }
    }

    async fn send_request_and_receive_response(
        &mut self,
        request: OwnedRequest,
    ) -> Result<OwnedResponse> {
        let request_data = bincode::serde::encode_to_vec(&request, bincode::config::standard())?;
        let mut framed = Framed::new(&mut self.stream, LengthDelimitedCodec::new());
        framed.send(request_data.into()).await?;
        if let Some(response_frame) = framed.next().await {
            let response_data = response_frame?;
            let response: OwnedResponse =
                bincode::serde::decode_from_slice(&response_data, bincode::config::standard())?.0;
            Ok(response)
        } else {
            Err(anyhow!("No response from daemon"))
        }
    }

    pub async fn synthesize(
        &mut self,
        text: &str,
        style_id: u32,
        options: OwnedSynthesizeOptions,
    ) -> Result<Vec<u8>> {
        let request = OwnedRequest::Synthesize {
            text: text.to_string(),
            style_id,
            options,
        };

        let response = self.send_request_and_receive_response(request).await?;
        match response {
            OwnedResponse::SynthesizeResult { wav_data } => Ok(wav_data),
            OwnedResponse::Error { message } => Err(anyhow!("Synthesis error: {message}")),
            _ => Err(anyhow!("Unexpected response type")),
        }
    }

    pub async fn list_speakers(&mut self) -> Result<Vec<Speaker>> {
        let request = OwnedRequest::ListSpeakers;

        let response = self.send_request_and_receive_response(request).await?;
        match response {
            OwnedResponse::SpeakersList { speakers } => Ok(speakers),
            OwnedResponse::SpeakersListWithModels { speakers, .. } => Ok(speakers),
            OwnedResponse::Error { message } => Err(anyhow!("List speakers error: {message}")),
            _ => Err(anyhow!("Unexpected response type")),
        }
    }

    pub async fn list_models(&mut self) -> Result<Vec<AvailableModel>> {
        let request = OwnedRequest::ListModels;

        let response = self.send_request_and_receive_response(request).await?;
        match response {
            OwnedResponse::ModelsList { models } => Ok(models),
            OwnedResponse::Error { message } => Err(anyhow!("List models error: {message}")),
            _ => Err(anyhow!("Unexpected response type")),
        }
    }
}
