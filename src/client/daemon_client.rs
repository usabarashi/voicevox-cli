use anyhow::{anyhow, Result};
use futures_util::{SinkExt, StreamExt};
use std::path::PathBuf;
use std::process::Command as ProcessCommand;
use std::time::Duration;
use tokio::net::UnixStream;
use tokio::time::timeout;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

use crate::ipc::{DaemonRequest, OwnedRequest, OwnedResponse, OwnedSynthesizeOptions};
use crate::paths::get_socket_path;
use crate::voice::Speaker;
use std::borrow::Cow;

fn find_daemon_binary() -> PathBuf {
    if let Ok(current_exe) = std::env::current_exe() {
        let mut daemon_path = current_exe.clone();
        daemon_path.set_file_name("voicevox-daemon");
        if daemon_path.exists() {
            return daemon_path;
        }
    }

    let fallbacks = vec![
        PathBuf::from("./target/debug/voicevox-daemon"),
        PathBuf::from("./target/release/voicevox-daemon"),
        PathBuf::from("voicevox-daemon"),
    ];

    fallbacks
        .into_iter()
        .find(|p| {
            p.exists()
                || p.file_name()
                    .map(|f| f == "voicevox-daemon")
                    .unwrap_or(false)
        })
        .unwrap_or_else(|| PathBuf::from("voicevox-daemon"))
}

pub async fn daemon_mode(
    text: &str,
    style_id: u32,
    options: OwnedSynthesizeOptions,
    output_file: Option<&String>,
    quiet: bool,
    socket_path: &PathBuf,
) -> Result<()> {
    let mut stream = timeout(Duration::from_secs(5), UnixStream::connect(socket_path))
        .await
        .map_err(|_| anyhow!("Daemon connection timeout"))?
        .map_err(|e| anyhow!("Failed to connect to daemon: {e}"))?;

    let request = OwnedRequest::Synthesize {
        text: Cow::Owned(text.to_string()),
        style_id,
        options,
    };

    let request_data =
        bincode::serialize(&request).map_err(|e| anyhow!("Failed to serialize request: {e}"))?;

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

        timeout(Duration::from_secs(30), framed_reader.next())
            .await
            .map_err(|_| anyhow!("Daemon response timeout"))?
            .ok_or_else(|| anyhow!("Connection closed by daemon"))?
            .map_err(|e| anyhow!("Failed to receive response: {e}"))?
    };

    let response: OwnedResponse = bincode::deserialize(&response_frame)
        .map_err(|e| anyhow!("Failed to deserialize response: {e}"))?;

    match response {
        OwnedResponse::SynthesizeResult { wav_data } => {
            if let Some(output_file) = output_file {
                std::fs::write(output_file, &wav_data)?;
            }

            if !quiet && output_file.is_none() {
                if let Err(e) = crate::client::audio::play_audio_from_memory(&wav_data) {
                    eprintln!("Error: Audio playback failed: {e}");
                    return Err(e);
                }
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
    let request_data = bincode::serialize(&request)?;
    framed_writer.send(request_data.into()).await?;

    if let Some(response_frame) = framed_reader.next().await {
        let response_frame = response_frame?;
        let response: OwnedResponse = bincode::deserialize(&response_frame)?;

        match response {
            OwnedResponse::SpeakersList { speakers } => {
                println!("All available speakers and styles from daemon:");
                for speaker in speakers.as_ref() {
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
                for speaker in speakers.as_ref() {
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
    let socket_path = get_socket_path();
    let daemon_path = find_daemon_binary();

    let output = ProcessCommand::new(&daemon_path)
        .args(["--start", "--detach"])
        .output();

    match output {
        Ok(output) => {
            if output.status.success() {
                // Poll for daemon readiness with exponential backoff
                let max_retries = 10;
                let mut retry_delay = Duration::from_millis(100);

                for attempt in 0..max_retries {
                    match UnixStream::connect(&socket_path).await {
                        Ok(_) => return Ok(()),
                        Err(_) if attempt < max_retries - 1 => {
                            tokio::time::sleep(retry_delay).await;
                            retry_delay = (retry_delay * 2).min(Duration::from_secs(1));
                        }
                        Err(_) => {}
                    }
                }

                Err(anyhow!(
                    "Daemon not responding after {} attempts",
                    max_retries
                ))
            } else {
                Err(anyhow!("Daemon failed to start"))
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
        match UnixStream::connect(&socket_path).await {
            Ok(stream) => Ok(Self { stream }),
            Err(_) => {
                crate::voice::has_available_models()
                    .then_some(())
                    .ok_or_else(|| anyhow!(
                        "No VOICEVOX models found. Please download models first using 'voicevox-cli download' or place .vvm files in the models directory."
                    ))?;
                start_daemon_automatically().await?;
                let stream = UnixStream::connect(&socket_path).await.map_err(|e| {
                    anyhow!(
                        "Daemon started but failed to connect at {}: {e}",
                        socket_path.display()
                    )
                })?;
                Ok(Self { stream })
            }
        }
    }

    pub async fn synthesize(
        &mut self,
        text: &str,
        style_id: u32,
        options: OwnedSynthesizeOptions,
    ) -> Result<Vec<u8>> {
        let request = OwnedRequest::Synthesize {
            text: Cow::Owned(text.to_string()),
            style_id,
            options,
        };

        let request_data = bincode::serialize(&request)?;
        let mut framed_writer = FramedWrite::new(&mut self.stream, LengthDelimitedCodec::new());
        framed_writer.send(request_data.into()).await?;

        let mut framed_reader = FramedRead::new(&mut self.stream, LengthDelimitedCodec::new());
        if let Some(Ok(response_data)) = framed_reader.next().await {
            let response: OwnedResponse = bincode::deserialize(&response_data)?;
            match response {
                OwnedResponse::SynthesizeResult { wav_data } => Ok(wav_data.into_owned()),
                OwnedResponse::Error { message } => Err(anyhow!("Synthesis error: {message}")),
                _ => Err(anyhow!("Unexpected response type")),
            }
        } else {
            Err(anyhow!("No response from daemon"))
        }
    }

    pub async fn list_speakers(&mut self) -> Result<Vec<Speaker>> {
        let request = OwnedRequest::ListSpeakers;

        let request_data = bincode::serialize(&request)?;
        let mut framed_writer = FramedWrite::new(&mut self.stream, LengthDelimitedCodec::new());
        framed_writer.send(request_data.into()).await?;

        let mut framed_reader = FramedRead::new(&mut self.stream, LengthDelimitedCodec::new());
        if let Some(Ok(response_data)) = framed_reader.next().await {
            let response: OwnedResponse = bincode::deserialize(&response_data)?;
            match response {
                OwnedResponse::SpeakersList { speakers } => Ok(speakers.into_owned()),
                OwnedResponse::SpeakersListWithModels { speakers, .. } => Ok(speakers.into_owned()),
                OwnedResponse::Error { message } => Err(anyhow!("List speakers error: {message}")),
                _ => Err(anyhow!("Unexpected response type")),
            }
        } else {
            Err(anyhow!("No response from daemon"))
        }
    }
}
