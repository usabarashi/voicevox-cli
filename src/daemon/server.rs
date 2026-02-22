use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::{UnixListener, UnixStream};
use tokio::signal;
use tokio::sync::{Mutex, RwLock};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

use crate::core::{CoreSynthesis, VoicevoxCore};
use crate::ipc::{DaemonRequest, OwnedRequest, OwnedResponse};

pub struct DaemonState {
    core: Mutex<VoicevoxCore>,
    style_to_model_map: RwLock<HashMap<u32, u32>>,
    all_speakers: RwLock<Vec<crate::voice::Speaker>>,
    available_models: RwLock<Vec<crate::voice::AvailableModel>>,
}

impl DaemonState {
    /// Builds daemon state and precomputes model/style metadata used by requests.
    ///
    /// # Errors
    ///
    /// Returns an error if VOICEVOX core initialization fails, model discovery fails,
    /// or the style-to-model mapping cannot be constructed.
    pub fn new() -> Result<Self> {
        let core = VoicevoxCore::new()?;
        let (mapping, speakers, models) =
            crate::voice::build_style_to_model_map_async_with_progress(&core, |_, _, _| {})?;

        Ok(Self {
            core: Mutex::new(core),
            style_to_model_map: RwLock::new(mapping),
            all_speakers: RwLock::new(speakers),
            available_models: RwLock::new(models),
        })
    }

    async fn get_model_id_from_style(&self, style_id: u32) -> u32 {
        self.style_to_model_map
            .read()
            .await
            .get(&style_id)
            .copied()
            .unwrap_or_else(|| {
                eprintln!(
                    "Warning: Style {style_id} not found in dynamic mapping, using style ID as model ID"
                );
                style_id
            })
    }

    async fn get_model_path(&self, model_id: u32) -> Option<PathBuf> {
        self.available_models
            .read()
            .await
            .iter()
            .find(|model| model.model_id == model_id)
            .map(|model| model.file_path.clone())
    }

    async fn speakers_list_response(&self) -> OwnedResponse {
        let speakers = self.all_speakers.read().await.clone();
        let style_to_model = self.style_to_model_map.read().await.clone();
        OwnedResponse::SpeakersListWithModels {
            speakers,
            style_to_model,
        }
    }

    async fn models_list_response(&self) -> OwnedResponse {
        let models = self.available_models.read().await.clone();
        OwnedResponse::ModelsList { models }
    }

    pub async fn handle_request(&self, request: OwnedRequest) -> OwnedResponse {
        match request {
            OwnedRequest::Ping => OwnedResponse::Pong,

            OwnedRequest::Synthesize {
                text,
                style_id,
                options: _,
            } => {
                let model_id = self.get_model_id_from_style(style_id).await;
                let model_path = self.get_model_path(model_id).await;

                let synthesis_result = {
                    let core = self.core.lock().await;

                    if let Err(e) = core.load_specific_model(&model_id.to_string()) {
                        eprintln!("Failed to load model {model_id}: {e}");
                        return OwnedResponse::Error {
                            message: format!("Failed to load model {model_id} for synthesis: {e}"),
                        };
                    }

                    let synthesis_result = core.synthesize(&text, style_id);
                    if let Some(model_path) = model_path.as_deref() {
                        if let Err(e) = core.unload_voice_model_by_path(model_path) {
                            eprintln!("Failed to unload model {model_id}: {e}");
                        }
                    }
                    synthesis_result
                };

                if model_path.is_none() {
                    eprintln!("Model {model_id} not found in available models");
                }

                match synthesis_result {
                    Ok(wav_data) => OwnedResponse::SynthesizeResult { wav_data },
                    Err(e) => OwnedResponse::Error {
                        message: format!("Synthesis failed: {e}"),
                    },
                }
            }

            OwnedRequest::ListSpeakers => self.speakers_list_response().await,

            OwnedRequest::ListModels => self.models_list_response().await,
        }
    }
}

/// Handles a single connected daemon client until the stream closes or decoding fails.
///
/// # Errors
///
/// Returns an error if reading from or writing to the framed Unix stream fails.
pub async fn handle_client(stream: UnixStream, state: Arc<DaemonState>) -> Result<()> {
    let mut framed = Framed::new(stream, LengthDelimitedCodec::new());

    while let Some(frame) = framed.next().await {
        let Ok(data) = frame else { break };

        let Ok((request, _)) = bincode::serde::decode_from_slice::<DaemonRequest, _>(
            &data,
            bincode::config::standard(),
        ) else {
            break;
        };

        let response = state.handle_request(request).await;
        let Ok(response_data) =
            bincode::serde::encode_to_vec(&response, bincode::config::standard())
        else {
            break;
        };

        if framed.send(response_data.into()).await.is_err() {
            break;
        }
    }

    Ok(())
}

async fn wait_for_shutdown_signal() -> Result<()> {
    signal::ctrl_c().await?;
    println!("\nShutting down daemon...");
    Ok(())
}

/// Runs the daemon accept loop and serves requests over a Unix domain socket.
///
/// # Errors
///
/// Returns an error if socket cleanup/bind fails, daemon state initialization fails,
/// socket accept fails, or final socket cleanup fails during shutdown.
pub async fn run_daemon(socket_path: PathBuf, foreground: bool) -> Result<()> {
    if socket_path.exists() {
        std::fs::remove_file(&socket_path)?;
    }

    let listener = UnixListener::bind(&socket_path)?;
    println!("VOICEVOX daemon started successfully");
    println!("Listening on: {}", socket_path.display());

    let state = Arc::new(DaemonState::new()?);

    if !foreground {
        println!("Running in background mode. Use Ctrl+C to stop gracefully.");
    }

    let server = async {
        loop {
            let (stream, _) = listener.accept().await?;
            let state_clone = Arc::clone(&state);
            tokio::spawn(async move {
                let _ = handle_client(stream, state_clone).await;
            });
        }
        #[allow(unreachable_code)]
        Ok::<(), anyhow::Error>(())
    };

    tokio::select! {
        result = server => result?,
        result = wait_for_shutdown_signal() => result?,
    }

    if socket_path.exists() {
        std::fs::remove_file(&socket_path)?;
    }

    println!("VOICEVOX daemon stopped");
    Ok(())
}
