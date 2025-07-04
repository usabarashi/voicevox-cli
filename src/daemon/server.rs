use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::{UnixListener, UnixStream};
use tokio::signal;
use tokio::sync::Mutex;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

fn get_dynamic_voice_mapping(
) -> std::collections::HashMap<std::borrow::Cow<'static, str>, (u32, std::borrow::Cow<'static, str>)>
{
    use std::borrow::Cow;

    let mut mapping = std::collections::HashMap::new();

    let available_models = scan_available_models().unwrap_or_default();

    available_models
        .iter()
        .enumerate()
        .for_each(|(index, model)| {
            let model_name = format!("model{}", model.model_id);
            let description = format!("Model {} (Default Style)", model.model_id);
            mapping.insert(
                Cow::Owned(model_name),
                (model.model_id, Cow::Owned(description)),
            );

            mapping.insert(
                Cow::Owned(model.model_id.to_string()),
                (
                    model.model_id,
                    Cow::Owned(format!("Model {}", model.model_id)),
                ),
            );

            if index == 0 {
                mapping.insert(
                    Cow::Borrowed("default"),
                    (
                        model.model_id,
                        Cow::Owned(format!("Default Model {}", model.model_id)),
                    ),
                );
            }
        });

    mapping
}

use crate::core::VoicevoxCore;
use crate::ipc::{DaemonRequest, OwnedRequest, OwnedResponse};
use crate::voice::{resolve_voice_dynamic, scan_available_models};
use std::borrow::Cow;
use std::collections::HashSet;

pub struct DaemonState {
    core: VoicevoxCore,
    loaded_models: Arc<Mutex<HashSet<u32>>>,
}

impl DaemonState {
    pub async fn new() -> Result<Self> {
        let core = VoicevoxCore::new()?;
        let loaded_models = Arc::new(Mutex::new(HashSet::new()));

        // Load only popular models on startup for faster initialization
        let popular_models = vec![3, 2, 8]; // Zundamon, Shikoku Metan, Kasukabe Tsumugi
        
        println!("Loading popular models for quick startup...");
        for model_id in popular_models {
            match core.load_specific_model(&model_id.to_string()) {
                Ok(_) => {
                    loaded_models.lock().await.insert(model_id);
                    println!("  ✓ Loaded model {}", model_id);
                }
                Err(e) => {
                    eprintln!("  ✗ Failed to load model {}: {}", model_id, e);
                }
            }
        }

        if loaded_models.lock().await.is_empty() {
            eprintln!("Warning: No models could be loaded. Please run 'voicevox-say' first to download models.");
        }

        Ok(DaemonState { core, loaded_models })
    }

    // Helper function to extract model_id from style_id
    fn get_model_id_from_style(style_id: u32) -> u32 {
        // Most models follow the pattern where style_id = model_id * 10 + variant
        // But some early models (0-30) have direct mapping
        if style_id <= 30 {
            style_id
        } else {
            style_id / 10
        }
    }

    // Ensure a model is loaded before use
    async fn ensure_model_loaded(&self, model_id: u32) -> Result<()> {
        let mut loaded = self.loaded_models.lock().await;
        
        if !loaded.contains(&model_id) {
            println!("Dynamically loading model {} on demand...", model_id);
            self.core.load_specific_model(&model_id.to_string())?;
            loaded.insert(model_id);
            println!("  ✓ Model {} loaded successfully", model_id);
        }
        
        Ok(())
    }

    pub async fn handle_request(&self, request: OwnedRequest) -> OwnedResponse {
        match request {
            OwnedRequest::Ping => OwnedResponse::Pong,

            OwnedRequest::Synthesize {
                text,
                style_id,
                options: _,
            } => {
                // Ensure the required model is loaded
                let model_id = Self::get_model_id_from_style(style_id);
                
                match self.ensure_model_loaded(model_id).await {
                    Ok(_) => {
                        // Model is loaded, proceed with synthesis
                        match self.core.synthesize(&text, style_id) {
                            Ok(wav_data) => OwnedResponse::SynthesizeResult {
                                wav_data: Cow::Owned(wav_data),
                            },
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
            },

            OwnedRequest::ListSpeakers => match self.core.get_speakers() {
                Ok(speakers) => OwnedResponse::SpeakersList {
                    speakers: Cow::Owned(speakers),
                },
                Err(e) => {
                    eprintln!("Failed to get speakers: {}", e);
                    OwnedResponse::Error {
                        message: Cow::Owned(format!("Failed to get speakers: {}", e)),
                    }
                }
            },

            OwnedRequest::LoadModel { model_name } => {
                println!("Loading model: {}", model_name);
                
                // Try to parse model_id from model_name
                if let Ok(model_id) = model_name.parse::<u32>() {
                    // Check if already loaded
                    let mut loaded = self.loaded_models.lock().await;
                    if loaded.contains(&model_id) {
                        println!("Model {} already loaded", model_name);
                        return OwnedResponse::Success;
                    }
                    
                    // Load the model
                    match self.core.load_specific_model(&model_name) {
                        Ok(_) => {
                            loaded.insert(model_id);
                            println!("Model loaded successfully: {}", model_name);
                            OwnedResponse::Success
                        }
                        Err(e) => {
                            println!("Failed to load model {}: {}", model_name, e);
                            OwnedResponse::Error {
                                message: Cow::Owned(format!(
                                    "Failed to load model {}: {}",
                                    model_name, e
                                )),
                            }
                        }
                    }
                } else {
                    // Non-numeric model name, just try to load it
                    match self.core.load_specific_model(&model_name) {
                        Ok(_) => {
                            println!("Model loaded successfully: {}", model_name);
                            OwnedResponse::Success
                        }
                        Err(e) => {
                            println!("Failed to load model {}: {}", model_name, e);
                            OwnedResponse::Error {
                                message: Cow::Owned(format!(
                                    "Failed to load model {}: {}",
                                    model_name, e
                                )),
                            }
                        }
                    }
                }
            }

            OwnedRequest::GetVoiceMapping => {
                println!("Getting voice mapping");
                // Return dynamic voice mapping from available models
                let dynamic_mapping = get_dynamic_voice_mapping();

                OwnedResponse::VoiceMapping {
                    mapping: dynamic_mapping,
                }
            }

            OwnedRequest::ResolveVoiceName { voice_name } => {
                println!("Resolving voice name: {}", voice_name);
                match resolve_voice_dynamic(&voice_name) {
                    Ok((style_id, description)) => {
                        println!("Resolved to style ID {} ({})", style_id, description);
                        OwnedResponse::VoiceResolution {
                            style_id,
                            description: Cow::Owned(description),
                        }
                    }
                    Err(e) => {
                        println!("Failed to resolve voice name {}: {}", voice_name, e);
                        OwnedResponse::Error {
                            message: Cow::Owned(format!(
                                "Failed to resolve voice name {}: {}",
                                voice_name, e
                            )),
                        }
                    }
                }
            }
        }
    }
}

pub async fn handle_client(stream: UnixStream, state: Arc<Mutex<DaemonState>>) -> Result<()> {
    let (reader, writer) = stream.into_split();
    let mut framed_reader = FramedRead::new(reader, LengthDelimitedCodec::new());
    let mut framed_writer = FramedWrite::new(writer, LengthDelimitedCodec::new());

    println!("New client connected");

    while let Some(frame) = framed_reader.next().await {
        match frame {
            Ok(data) => {
                // Deserialize request
                match bincode::deserialize::<DaemonRequest>(&data) {
                    Ok(request) => {
                        // Handle request
                        let response = {
                            let state = state.lock().await;
                            state.handle_request(request).await
                        };

                        // Serialize and send response
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
                    Err(e) => {
                        println!("Failed to deserialize request: {}", e);
                        let error_response = OwnedResponse::Error {
                            message: Cow::Owned(format!("Failed to deserialize request: {}", e)),
                        };

                        if let Ok(error_data) = bincode::serialize(&error_response) {
                            let _ = framed_writer.send(error_data.into()).await;
                        }
                        break;
                    }
                }
            }
            Err(e) => {
                println!("Frame error: {}", e);
                break;
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
