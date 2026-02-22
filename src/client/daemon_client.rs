use anyhow::{anyhow, Context, Result};
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::net::UnixStream;
use tokio::process::Command;
use tokio::time::timeout;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

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

/// Finds the daemon executable path using the current binary location and common fallbacks.
///
/// # Errors
///
/// Returns `DaemonBinaryNotFound` if no usable `voicevox-daemon` binary can be found.
pub fn find_daemon_binary() -> Result<PathBuf, crate::daemon::DaemonError> {
    if let Ok(current_exe) = std::env::current_exe() {
        let mut daemon_path = current_exe;
        daemon_path.set_file_name("voicevox-daemon");
        if daemon_path.exists() {
            return Ok(daemon_path);
        }
    }

    [
        PathBuf::from("./target/debug/voicevox-daemon"),
        PathBuf::from("./target/release/voicevox-daemon"),
    ]
    .into_iter()
    .find(|p| p.exists())
    .or_else(|| find_in_path("voicevox-daemon"))
    .ok_or(crate::daemon::DaemonError::DaemonBinaryNotFound)
}

fn find_in_path(binary_name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|path_var| {
        std::env::split_paths(&path_var)
            .map(|dir| dir.join(binary_name))
            .find(|candidate| candidate.is_file())
    })
}

fn daemon_response_error(context: &str, message: &str) -> anyhow::Error {
    anyhow!("{context}: {message}")
}

fn unexpected_daemon_response(context: &str) -> anyhow::Error {
    anyhow!("Unexpected response {context}")
}

async fn connect_socket_with_timeout(
    socket_path: &Path,
    timeout_duration: Duration,
) -> Result<UnixStream> {
    timeout(timeout_duration, UnixStream::connect(socket_path))
        .await
        .map_err(|_| anyhow!("Timeout connecting to daemon"))?
        .map_err(|e| {
            anyhow!(
                "Failed to connect to daemon at {}: {e}",
                socket_path.display()
            )
        })
}

async fn connect_daemon_with_timeout(
    socket_path: &Path,
    timeout_duration: Duration,
) -> Result<UnixStream> {
    connect_socket_with_timeout(socket_path, timeout_duration)
        .await
        .with_context(|| format!("Daemon connection failed at {}", socket_path.display()))
}

async fn wait_for_daemon_startup(socket_path: &Path) -> Result<()> {
    use std::io::Write as _;

    let max_retries = DAEMON_STARTUP_MAX_RETRIES;
    let mut retry_delay = DAEMON_STARTUP_INITIAL_DELAY;

    for attempt in 0..max_retries {
        match timeout(DAEMON_CONNECTION_TIMEOUT, UnixStream::connect(socket_path)).await {
            Ok(Ok(_)) => return Ok(()),
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
        "Daemon not responding after {max_retries} attempts (~{DAEMON_STARTUP_TOTAL_TIME_ESTIMATE}s total)"
    ))
}

async fn request_daemon_once(
    socket_path: &Path,
    request: &OwnedRequest,
    connect_timeout_duration: Duration,
    response_timeout_duration: Duration,
) -> Result<OwnedResponse> {
    let stream = connect_daemon_with_timeout(socket_path, connect_timeout_duration).await?;
    let mut framed = Framed::new(stream, LengthDelimitedCodec::new());

    let request_data = bincode::serde::encode_to_vec(request, bincode::config::standard())
        .map_err(|e| anyhow!("Failed to serialize request: {e}"))?;

    framed
        .send(request_data.into())
        .await
        .map_err(|e| anyhow!("Failed to send request: {e}"))?;

    let response_frame = timeout(response_timeout_duration, framed.next())
        .await
        .map_err(|_| anyhow!("Daemon response timeout"))?
        .ok_or_else(|| anyhow!("Connection closed by daemon"))?
        .map_err(|e| anyhow!("Failed to receive response: {e}"))?;

    bincode::serde::decode_from_slice(&response_frame, bincode::config::standard())
        .map(|(response, _)| response)
        .map_err(|e| anyhow!("Failed to deserialize response: {e}"))
}

/// Sends a synthesis request to an already running daemon and handles output/playback.
///
/// # Errors
///
/// Returns an error if daemon connection/setup fails, request/response framing fails,
/// synthesis returns an error response, file writing fails, or audio playback fails.
pub async fn daemon_mode(
    text: &str,
    style_id: u32,
    options: OwnedSynthesizeOptions,
    output_file: Option<&Path>,
    quiet: bool,
    socket_path: &Path,
) -> Result<()> {
    let request = OwnedRequest::Synthesize {
        text: text.to_string(),
        style_id,
        options,
    };
    let response = request_daemon_once(
        socket_path,
        &request,
        DAEMON_CONNECTION_TIMEOUT,
        DAEMON_RESPONSE_TIMEOUT,
    )
    .await?;

    match response {
        OwnedResponse::SynthesizeResult { wav_data } => {
            crate::client::audio::emit_synthesized_audio(&wav_data, output_file, quiet)?;
            Ok(())
        }
        OwnedResponse::Error { message } => Err(daemon_response_error("Daemon error", &message)),
        _ => Err(unexpected_daemon_response("from daemon")),
    }
}

/// Requests the speaker list from the daemon and prints it in CLI-friendly format.
///
/// # Errors
///
/// Returns an error if daemon connection, request/response serialization, or response
/// decoding fails, or if the daemon returns an error response.
pub async fn list_speakers_daemon(socket_path: &Path) -> Result<()> {
    let response = request_daemon_once(
        socket_path,
        &DaemonRequest::ListSpeakers,
        DAEMON_CONNECTION_TIMEOUT,
        DAEMON_RESPONSE_TIMEOUT,
    )
    .await?;

    match response {
        OwnedResponse::SpeakersList { speakers } => {
            print_speakers(&speakers, None);
            Ok(())
        }
        OwnedResponse::SpeakersListWithModels {
            speakers,
            style_to_model,
        } => {
            print_speakers(&speakers, Some(&style_to_model));
            Ok(())
        }
        OwnedResponse::Error { message } => Err(daemon_response_error("Daemon error", &message)),
        _ => Err(unexpected_daemon_response("from daemon")),
    }
}

fn print_speakers(speakers: &[Speaker], style_to_model: Option<&HashMap<u32, u32>>) {
    println!("All available speakers and styles from daemon:");
    for speaker in speakers {
        println!("  {}", speaker.name);
        for style in &speaker.styles {
            match style_to_model.and_then(|map| map.get(&style.id)) {
                Some(model_id) => {
                    println!(
                        "    {} (Model: {model_id}, Style ID: {})",
                        style.name, style.id
                    );
                }
                None => {
                    println!("    {} (Style ID: {})", style.name, style.id);
                }
            }

            if let Some(style_type) = &style.style_type {
                println!("        Type: {style_type}");
            }
        }
        println!();
    }
}

async fn start_daemon_automatically(socket_path: &Path) -> Result<()> {
    use std::io::Write;

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
        .arg("--start")
        .arg("--detach")
        .arg("--socket-path")
        .arg(socket_path)
        .output()
        .await;

    match output {
        Ok(output) => {
            if output.status.success() {
                wait_for_daemon_startup(socket_path).await?;
                println!(" done!");
                println!("VOICEVOX daemon started successfully");
                Ok(())
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
    /// Connects to the daemon using the default socket path.
    ///
    /// # Errors
    ///
    /// Returns an error if the daemon socket cannot be reached.
    pub async fn new() -> Result<Self> {
        Self::new_at(&get_socket_path()).await
    }

    /// Connects to the daemon using an explicit socket path.
    ///
    /// # Errors
    ///
    /// Returns an error if the daemon socket cannot be reached.
    pub async fn new_at(socket_path: &Path) -> Result<Self> {
        let stream = UnixStream::connect(socket_path).await.map_err(|e| {
            anyhow!(
                "Failed to connect to daemon at {}: {e}",
                socket_path.display()
            )
        })?;

        Ok(Self { stream })
    }

    /// Connects to the daemon with retry/backoff behavior.
    ///
    /// # Errors
    ///
    /// Returns an error if all retry attempts fail.
    pub async fn connect_with_retry() -> Result<Self> {
        Self::connect_with_retry_at(&get_socket_path()).await
    }

    /// Connects to the daemon with retry/backoff behavior using an explicit socket path.
    ///
    /// # Errors
    ///
    /// Returns an error if all retry attempts fail.
    pub async fn connect_with_retry_at(socket_path: &Path) -> Result<Self> {
        use crate::daemon::startup;

        let mut last_error = None;
        let mut retry_delay = startup::initial_retry_delay();

        for attempt in 0..startup::MAX_CONNECT_ATTEMPTS {
            match Self::new_at(socket_path).await {
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

    /// Creates a new `DaemonClient` with automatic daemon startup if not running.
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
        Self::new_with_auto_start_at(&get_socket_path()).await
    }

    /// Creates a new `DaemonClient` with automatic daemon startup using an explicit socket path.
    ///
    /// # Errors
    ///
    /// Returns an error if no models are available, daemon startup fails, or connection fails.
    pub async fn new_with_auto_start_at(socket_path: &Path) -> Result<Self> {
        if let Ok(stream) =
            connect_socket_with_timeout(socket_path, DAEMON_CONNECTION_TIMEOUT).await
        {
            return Ok(Self { stream });
        }

        crate::voice::has_available_models()
            .then_some(())
            .ok_or_else(|| anyhow!(
                "No VOICEVOX models found. Please download models first using 'voicevox-cli download' or place .vvm files in the models directory."
            ))?;
        start_daemon_automatically(socket_path).await?;

        tokio::time::sleep(DAEMON_STARTUP_GRACE_PERIOD).await;

        let stream = connect_socket_with_timeout(socket_path, DAEMON_FINAL_CONNECTION_TIMEOUT)
            .await
            .map_err(|e| {
                anyhow!(
                    "Daemon started but failed to connect at {}: {e}",
                    socket_path.display()
                )
            })?;
        Ok(Self { stream })
    }

    async fn send_request_and_receive_response(
        &mut self,
        request: OwnedRequest,
    ) -> Result<OwnedResponse> {
        let request_data = bincode::serde::encode_to_vec(&request, bincode::config::standard())?;
        let mut framed = Framed::new(&mut self.stream, LengthDelimitedCodec::new());
        framed.send(request_data.into()).await?;
        let response_data = framed
            .next()
            .await
            .ok_or_else(|| anyhow!("No response from daemon"))??;
        let (response, _) =
            bincode::serde::decode_from_slice(&response_data, bincode::config::standard())?;
        Ok(response)
    }

    /// Sends a synthesis request and returns the generated WAV bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if request transmission fails, the response is invalid, or the
    /// daemon reports a synthesis error.
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
            OwnedResponse::Error { message } => {
                Err(daemon_response_error("Synthesis error", &message))
            }
            _ => Err(unexpected_daemon_response("type")),
        }
    }

    /// Fetches speakers from the daemon.
    ///
    /// # Errors
    ///
    /// Returns an error if request/response I/O fails, decoding fails, or the daemon
    /// returns an error response.
    pub async fn list_speakers(&mut self) -> Result<Vec<Speaker>> {
        let request = OwnedRequest::ListSpeakers;

        let response = self.send_request_and_receive_response(request).await?;
        match response {
            OwnedResponse::SpeakersList { speakers }
            | OwnedResponse::SpeakersListWithModels { speakers, .. } => Ok(speakers),
            OwnedResponse::Error { message } => {
                Err(daemon_response_error("List speakers error", &message))
            }
            _ => Err(unexpected_daemon_response("type")),
        }
    }

    /// Fetches available models from the daemon.
    ///
    /// # Errors
    ///
    /// Returns an error if request/response I/O fails, decoding fails, or the daemon
    /// returns an error response.
    pub async fn list_models(&mut self) -> Result<Vec<AvailableModel>> {
        let request = OwnedRequest::ListModels;

        let response = self.send_request_and_receive_response(request).await?;
        match response {
            OwnedResponse::ModelsList { models } => Ok(models),
            OwnedResponse::Error { message } => {
                Err(daemon_response_error("List models error", &message))
            }
            _ => Err(unexpected_daemon_response("type")),
        }
    }
}
