use anyhow::{anyhow, Result};
use futures_util::{SinkExt, StreamExt};
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command as ProcessCommand;
use std::time::Duration;
use tokio::net::UnixStream;
use tokio::time::timeout;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

use crate::ipc::{DaemonRequest, OwnedRequest, OwnedResponse, OwnedSynthesizeOptions};
use crate::paths::get_socket_path;
use std::borrow::Cow;

fn find_daemon_binary() -> PathBuf {
    // Try current executable directory first
    if let Ok(current_exe) = std::env::current_exe() {
        let mut daemon_path = current_exe.clone();
        daemon_path.set_file_name("voicevox-daemon");
        if daemon_path.exists() {
            return daemon_path;
        }
    }

    // Try fallback paths
    let fallbacks = vec![
        PathBuf::from("./target/debug/voicevox-daemon"),
        PathBuf::from("./target/release/voicevox-daemon"),
        PathBuf::from("voicevox-daemon"), // In PATH
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

// Communicate with daemon
pub async fn daemon_mode(
    text: &str,
    style_id: u32,
    _voice_description: &str,
    options: OwnedSynthesizeOptions,
    output_file: Option<&String>,
    quiet: bool,
    socket_path: &PathBuf,
) -> Result<()> {
    // Connect to daemon with timeout
    let mut stream = timeout(Duration::from_secs(5), UnixStream::connect(socket_path))
        .await
        .map_err(|_| anyhow!("Daemon connection timeout"))?
        .map_err(|e| anyhow!("Failed to connect to daemon: {}", e))?;

    // Enable zero-copy if supported
    #[cfg(unix)]
    let mut options = options;
    #[cfg(unix)]
    {
        if crate::client::fd_receive::supports_zero_copy() {
            println!("DEBUG: Enabling zero-copy mode in client");
            options.zero_copy = true;
        }
    }
    #[cfg(not(unix))]
    let options = options;

    // Send request
    let request = OwnedRequest::Synthesize {
        text: Cow::Owned(text.to_string()),
        style_id,
        options,
    };

    let request_data =
        bincode::serialize(&request).map_err(|e| anyhow!("Failed to serialize request: {}", e))?;

    // Send using split temporarily
    {
        let (_reader, writer) = stream.split();
        let mut framed_writer = FramedWrite::new(writer, LengthDelimitedCodec::new());
        framed_writer
            .send(request_data.into())
            .await
            .map_err(|e| anyhow!("Failed to send request: {}", e))?;
    }

    // Receive response
    let response_frame = {
        let (reader, _writer) = stream.split();
        let mut framed_reader = FramedRead::new(reader, LengthDelimitedCodec::new());

        timeout(Duration::from_secs(30), framed_reader.next())
            .await
            .map_err(|_| anyhow!("Daemon response timeout"))?
            .ok_or_else(|| anyhow!("Connection closed by daemon"))?
            .map_err(|e| anyhow!("Failed to receive response: {}", e))?
    };

    let response: OwnedResponse = bincode::deserialize(&response_frame)
        .map_err(|e| anyhow!("Failed to deserialize response: {}", e))?;

    match response {
        OwnedResponse::SynthesizeResult { wav_data } => {
            // Handle output
            if let Some(output_file) = output_file {
                std::fs::write(output_file, &wav_data)?;
            }

            // Play audio if not quiet and no output file (like macOS say command)
            if !quiet && output_file.is_none() {
                if let Err(e) = crate::client::audio::play_audio_from_memory(&wav_data) {
                    eprintln!("Error: Audio playback failed: {}", e);
                    return Err(e);
                }
            }

            Ok(())
        }
        #[cfg(unix)]
        OwnedResponse::SynthesizeResultFd { size, format: _ } => {
            println!("DEBUG: Received FD response, waiting for FD...");

            // Small delay to ensure server sends FD
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            // Try to receive FD using stream's try_io
            let result = stream.try_io(tokio::io::Interest::READABLE, || {
                use crate::daemon::fd_passing::receive_fd;
                use std::os::unix::io::AsRawFd;

                let socket_fd = stream.as_raw_fd();
                let mut metadata_buf = vec![0u8; 16];
                match receive_fd(socket_fd, &mut metadata_buf) {
                    Ok((fd, size)) => Ok((fd, size)),
                    Err(e) => {
                        eprintln!("FD receive error: {}", e);
                        Err(std::io::Error::other(e.to_string()))
                    }
                }
            });

            match result {
                Ok((received_fd, _)) => {
                    println!("DEBUG: Successfully received FD: {}", received_fd);

                    // Create audio buffer from received FD
                    use crate::client::fd_receive::ReceivedAudioBuffer;
                    let audio_buffer = unsafe { ReceivedAudioBuffer::from_fd(received_fd, size)? };

                    // Get audio data
                    let wav_data = audio_buffer.to_vec();

                    // Handle output
                    if let Some(output_file) = output_file {
                        std::fs::write(output_file, &wav_data)?;
                    }

                    // Play audio if not quiet and no output file
                    if !quiet && output_file.is_none() {
                        if let Err(e) = crate::client::audio::play_audio_from_memory(&wav_data) {
                            eprintln!("Error: Audio playback failed: {}", e);
                            return Err(e);
                        }
                    }

                    Ok(())
                }
                _ => {
                    eprintln!("Failed to receive audio FD via try_io");
                    Err(anyhow!("Failed to receive audio file descriptor"))
                }
            }
        }
        OwnedResponse::Error { message } => Err(anyhow!("Daemon error: {}", message)),
        _ => Err(anyhow!("Unexpected response from daemon")),
    }
}

// List speakers via daemon
pub async fn list_speakers_daemon(socket_path: &PathBuf) -> Result<()> {
    let stream = UnixStream::connect(socket_path).await?;
    let (reader, writer) = stream.into_split();
    let mut framed_reader = FramedRead::new(reader, LengthDelimitedCodec::new());
    let mut framed_writer = FramedWrite::new(writer, LengthDelimitedCodec::new());

    // Send list speakers request
    let request = DaemonRequest::ListSpeakers;
    let request_data = bincode::serialize(&request)?;
    framed_writer.send(request_data.into()).await?;

    // Receive response
    if let Some(response_frame) = framed_reader.next().await {
        let response_frame = response_frame?;
        let response: OwnedResponse = bincode::deserialize(&response_frame)?;

        match response {
            OwnedResponse::SpeakersList { speakers } => {
                println!("All available speakers and styles from daemon:");
                for speaker in speakers.as_ref() {
                    println!("  {}", speaker.name);
                    for style in &speaker.styles {
                        println!("    {} (ID: {})", style.name, style.id);
                        if let Some(style_type) = &style.style_type {
                            println!("        Type: {}", style_type);
                        }
                    }
                    println!();
                }
                return Ok(());
            }
            OwnedResponse::Error { message } => {
                return Err(anyhow!("Daemon error: {}", message));
            }
            _ => {
                return Err(anyhow!("Unexpected response from daemon"));
            }
        }
    }

    Err(anyhow!("Failed to get speakers from daemon"))
}

// Start daemon process if not already running (with user confirmation)
pub async fn start_daemon_with_confirmation() -> Result<()> {
    let socket_path = get_socket_path();

    // Check if daemon is already running
    match UnixStream::connect(&socket_path).await {
        Ok(_) => {
            // Daemon is already running
            return Ok(());
        }
        Err(_) => {
            // Daemon not running, ask user for confirmation
        }
    }

    print!(
        "VOICEVOX daemon is not running.
Would you like to start the daemon automatically? [Y/n]: "
    );
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim().to_lowercase();

    if input.is_empty() || input == "y" || input == "yes" {
        start_daemon_automatically().await
    } else {
        Err(anyhow!(
            "Daemon startup declined by user. Use 'voicevox-daemon --start' to start manually."
        ))
    }
}

// Start daemon process if not already running (automatic, no confirmation)
pub async fn start_daemon_if_needed() -> Result<()> {
    let socket_path = get_socket_path();

    // Check if daemon is already running
    match UnixStream::connect(&socket_path).await {
        Ok(_) => {
            // Daemon is already running
            return Ok(());
        }
        Err(_) => {
            // Daemon not running, try to start it
        }
    }

    start_daemon_automatically().await
}

// Internal function to actually start the daemon
async fn start_daemon_automatically() -> Result<()> {
    let socket_path = get_socket_path();
    let daemon_path = find_daemon_binary();

    println!("🔄 Starting VOICEVOX daemon automatically...");

    // Start daemon with --start --detach for background operation
    let output = ProcessCommand::new(&daemon_path)
        .args(["--start", "--detach"])
        .output();

    match output {
        Ok(output) => {
            if output.status.success() {
                // Give daemon time to start
                tokio::time::sleep(Duration::from_millis(2000)).await;

                // Verify daemon is running
                match UnixStream::connect(&socket_path).await {
                    Ok(_) => {
                        println!("✅ VOICEVOX daemon started successfully");
                        Ok(())
                    }
                    Err(_) => {
                        eprintln!("❌ Daemon started but not responding on socket");
                        Err(anyhow!("Daemon not responding after startup"))
                    }
                }
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                if !stderr.is_empty() {
                    eprintln!("Daemon startup error: {}", stderr);
                }
                Err(anyhow!("Daemon failed to start"))
            }
        }
        Err(e) => {
            eprintln!("❌ Failed to execute daemon: {}", e);
            Err(anyhow!("Failed to execute daemon: {}", e))
        }
    }
}
