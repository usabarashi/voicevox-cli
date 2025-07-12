use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::{UnixListener, UnixStream};
use tokio::signal;
use tokio::sync::Mutex;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

use crate::core::VoicevoxCore;
use crate::ipc::{DaemonRequest, OwnedRequest, OwnedResponse};
use std::borrow::Cow;

pub struct DaemonState {
    core: VoicevoxCore,
    style_to_model_map: Arc<Mutex<HashMap<u32, u32>>>,
    all_speakers: Arc<Mutex<Vec<crate::voice::Speaker>>>,
    #[cfg(unix)]
    pending_fd: Arc<Mutex<Option<std::os::unix::io::RawFd>>>,
}

impl DaemonState {
    pub async fn new() -> Result<Self> {
        let core = VoicevoxCore::new()?;
        let style_to_model_map = Arc::new(Mutex::new(HashMap::new()));

        // Build dynamic style-to-model mapping
        println!("Building dynamic style-to-model mapping...");
        let (mapping, speakers) = crate::voice::build_style_to_model_map_async(&core).await?;
        *style_to_model_map.lock().await = mapping;
        let all_speakers = Arc::new(Mutex::new(speakers));
        println!(
            "  ✓ Discovered {} style mappings",
            style_to_model_map.lock().await.len()
        );

        // No persistent loading - models are loaded and unloaded per request
        println!("Models will be loaded and unloaded per synthesis request.");

        Ok(DaemonState {
            core,
            style_to_model_map,
            all_speakers,
            #[cfg(unix)]
            pending_fd: Arc::new(Mutex::new(None)),
        })
    }

    // Get pending FD and clear it
    #[cfg(unix)]
    pub async fn take_pending_fd(&self) -> Option<std::os::unix::io::RawFd> {
        self.pending_fd.lock().await.take()
    }

    // Helper function to extract model_id from style_id using dynamic mapping
    async fn get_model_id_from_style(&self, style_id: u32) -> u32 {
        let map = self.style_to_model_map.lock().await;

        // Use dynamic mapping if available
        if let Some(&model_id) = map.get(&style_id) {
            return model_id;
        }

        // Fallback: use style_id as model_id (for backward compatibility)
        eprintln!(
            "Warning: Style {} not found in dynamic mapping, using style ID as model ID",
            style_id
        );
        style_id
    }

    pub async fn handle_request(&self, request: OwnedRequest) -> OwnedResponse {
        match request {
            OwnedRequest::Ping => OwnedResponse::Pong,

            OwnedRequest::Synthesize {
                text,
                style_id,
                options,
            } => {
                // Get the required model ID
                let model_id = self.get_model_id_from_style(style_id).await;

                // Load model for this request only
                match self.core.load_specific_model(&model_id.to_string()) {
                    Ok(_) => {
                        println!("  ✓ Loaded model {} for synthesis", model_id);

                        // Perform synthesis
                        let synthesis_result = self.core.synthesize(&text, style_id);

                        // Always unload model after synthesis
                        // Note: Since we just loaded the model successfully, we should have a valid path
                        match crate::paths::find_models_dir_client() {
                            Ok(models_dir) => {
                                let model_path = models_dir.join(format!("{}.vvm", model_id));
                                match self
                                    .core
                                    .unload_voice_model_by_path(model_path.to_str().unwrap_or(""))
                                {
                                    Ok(_) => {
                                        println!("  ✓ Unloaded model {} after synthesis", model_id)
                                    }
                                    Err(e) => {
                                        eprintln!("  ✗ Failed to unload model {}: {}", model_id, e)
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("  ✗ Failed to find models directory for unload: {}", e);
                                // Model will remain loaded but will be cleaned up on daemon shutdown
                            }
                        }

                        match synthesis_result {
                            Ok(wav_data) => {
                                // Check if client supports zero-copy
                                #[cfg(unix)]
                                if options.zero_copy {
                                    // Create anonymous buffer and write WAV data
                                    use super::fd_passing::AnonymousBuffer;

                                    match AnonymousBuffer::new("voicevox_audio", wav_data.len()) {
                                        Ok(mut buffer) => {
                                            if buffer.write_all(&wav_data).is_ok() {
                                                // Store the FD for later sending
                                                let fd = buffer.into_fd();
                                                self.pending_fd.lock().await.replace(fd);

                                                // Send metadata response
                                                OwnedResponse::SynthesizeResultFd {
                                                    size: wav_data.len(),
                                                    format: crate::ipc::AudioFormat::default(),
                                                }
                                            } else {
                                                // Fallback to regular response
                                                OwnedResponse::SynthesizeResult {
                                                    wav_data: Cow::Owned(wav_data),
                                                }
                                            }
                                        }
                                        Err(_e) => {
                                            // Fallback to regular response
                                            OwnedResponse::SynthesizeResult {
                                                wav_data: Cow::Owned(wav_data),
                                            }
                                        }
                                    }
                                } else {
                                    // Regular response
                                    OwnedResponse::SynthesizeResult {
                                        wav_data: Cow::Owned(wav_data),
                                    }
                                }

                                #[cfg(not(unix))]
                                OwnedResponse::SynthesizeResult {
                                    wav_data: Cow::Owned(wav_data),
                                }
                            }
                            Err(e) => {
                                eprintln!("Synthesis failed: {}", e);
                                OwnedResponse::Error {
                                    message: Cow::Owned(format!("Synthesis failed: {}", e)),
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to load model {}: {}", model_id, e);
                        OwnedResponse::Error {
                            message: Cow::Owned(format!(
                                "Failed to load model {} for synthesis: {}",
                                model_id, e
                            )),
                        }
                    }
                }
            }

            OwnedRequest::ListSpeakers => {
                // Use the pre-collected speakers list that includes all models
                let all_speakers = self.all_speakers.lock().await.clone();
                let style_to_model = self.style_to_model_map.lock().await.clone();

                // Send enhanced response with all speakers and model mapping
                OwnedResponse::SpeakersListWithModels {
                    speakers: Cow::Owned(all_speakers),
                    style_to_model,
                }
            }

            OwnedRequest::LoadModel { model_name } => {
                // Since we load and unload per request, this is now a no-op
                println!(
                    "LoadModel request received for: {} (no-op - models are loaded per request)",
                    model_name
                );
                OwnedResponse::Success
            }

            OwnedRequest::GetCapabilities => {
                // Return daemon capabilities
                OwnedResponse::Capabilities(crate::ipc::ProtocolCapabilities::default())
            }

            OwnedRequest::GetVoiceMapping => {
                println!("Getting voice mapping");
                // Build voice mapping from all speakers
                let mut mapping = HashMap::new();
                let all_speakers = self.all_speakers.lock().await;

                for speaker in all_speakers.iter() {
                    for style in &speaker.styles {
                        let voice_name = format!("{}-{}", speaker.name, style.name);
                        let description = format!("{} ({})", speaker.name, style.name);
                        mapping.insert(Cow::Owned(voice_name), (style.id, Cow::Owned(description)));
                    }
                }

                OwnedResponse::VoiceMapping { mapping }
            }

            OwnedRequest::ResolveVoiceName { voice_name } => {
                println!("Resolving voice name: {}", voice_name);

                // Try to parse as style ID first
                if let Ok(style_id) = voice_name.parse::<u32>() {
                    OwnedResponse::VoiceResolution {
                        style_id,
                        description: Cow::Owned(format!("Style ID {}", style_id)),
                    }
                } else {
                    // Search through all speakers for matching voice name
                    let all_speakers = self.all_speakers.lock().await;

                    for speaker in all_speakers.iter() {
                        for style in &speaker.styles {
                            let full_name = format!("{}-{}", speaker.name, style.name);
                            if full_name.to_lowercase() == voice_name.to_lowercase() {
                                return OwnedResponse::VoiceResolution {
                                    style_id: style.id,
                                    description: Cow::Owned(full_name),
                                };
                            }
                        }
                    }

                    OwnedResponse::Error {
                        message: Cow::Owned(format!("Voice name '{}' not found", voice_name)),
                    }
                }
            }
        }
    }
}

pub async fn handle_client(mut stream: UnixStream, state: Arc<Mutex<DaemonState>>) -> Result<()> {
    println!("New client connected (FD-enabled handler)");

    loop {
        // Read request using framed codec
        let request = {
            let (reader, _writer) = stream.split();
            let mut framed_reader = FramedRead::new(reader, LengthDelimitedCodec::new());

            match framed_reader.next().await {
                Some(Ok(data)) => match bincode::deserialize::<DaemonRequest>(&data) {
                    Ok(req) => req,
                    Err(e) => {
                        println!("Failed to deserialize request: {}", e);
                        break;
                    }
                },
                _ => break,
            }
        };

        // Handle request
        let response = {
            let state = state.lock().await;
            state.handle_request(request).await
        };

        // Check if we need FD passing
        #[cfg(unix)]
        let needs_fd = matches!(response, OwnedResponse::SynthesizeResultFd { .. });
        #[cfg(not(unix))]
        let needs_fd = false;

        // Send response
        {
            let (_reader, writer) = stream.split();
            let mut framed_writer = FramedWrite::new(writer, LengthDelimitedCodec::new());

            match bincode::serialize(&response) {
                Ok(response_data) => {
                    if let Err(e) = framed_writer.send(response_data.into()).await {
                        println!("Failed to send response: {}", e);
                        break;
                    }
                }
                Err(e) => {
                    println!("Failed to serialize response: {}", e);
                    break;
                }
            }
        }

        // Now handle FD passing if needed - stream is available again
        #[cfg(unix)]
        if needs_fd {
            if let Some(fd) = state.lock().await.take_pending_fd().await {
                // Small delay to ensure response is received
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

                // Use the stream directly for FD passing
                let result = stream.try_io(tokio::io::Interest::WRITABLE, || {
                    use crate::daemon::fd_passing::send_fd;
                    use std::os::unix::io::AsRawFd;
                    let socket_fd = stream.as_raw_fd();
                    match send_fd(socket_fd, fd, b"audio") {
                        Ok(_) => Ok(()),
                        Err(e) => {
                            eprintln!("FD send error: {}", e);
                            Err(std::io::Error::other(e.to_string()))
                        }
                    }
                });

                match result {
                    Ok(_) => println!("✅ Successfully sent audio FD"),
                    Err(e) => eprintln!("❌ Failed to send FD: {}", e),
                }

                // Close the FD
                unsafe {
                    libc::close(fd);
                }
            }
        }
    }

    println!("Client disconnected");
    Ok(())
}

pub async fn run_daemon(socket_path: PathBuf, foreground: bool) -> Result<()> {
    // Remove existing socket file if it exists
    if socket_path.exists() {
        std::fs::remove_file(&socket_path)?;
    }

    // Create Unix socket listener
    let listener = UnixListener::bind(&socket_path)?;
    println!("VOICEVOX daemon started successfully");
    println!("Listening on: {}", socket_path.display());

    // Initialize daemon state
    let state = Arc::new(Mutex::new(DaemonState::new().await?));

    if !foreground {
        println!("Running in background mode. Use Ctrl+C to stop gracefully.");
    }

    // Set up graceful shutdown
    let shutdown = async {
        signal::ctrl_c().await.expect("Failed to listen for ctrl-c");
        println!("\nShutting down daemon...");
    };

    // Accept connections
    let server = async {
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let state_clone = Arc::clone(&state);
                    tokio::spawn(async move {
                        if let Err(e) = handle_client(stream, state_clone).await {
                            println!("Client handler error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    println!("Failed to accept connection: {}", e);
                }
            }
        }
    };

    // Run server with shutdown handling
    tokio::select! {
        _ = server => {},
        _ = shutdown => {},
    }

    // Cleanup
    if socket_path.exists() {
        std::fs::remove_file(&socket_path)?;
    }

    println!("VOICEVOX daemon stopped");
    Ok(())
}
