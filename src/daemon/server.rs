use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::net::{UnixListener, UnixStream};
use tokio::signal;
use tokio::sync::Mutex;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

use crate::core::VoicevoxCore;
use crate::ipc::{DaemonRequest, OwnedRequest, OwnedResponse};

pub struct DaemonState {
    core: Mutex<VoicevoxCore>,
    style_to_model_map: HashMap<u32, u32>,
    all_speakers: Vec<crate::voice::Speaker>,
    available_models: Vec<crate::voice::AvailableModel>,
}

struct SocketFileGuard {
    path: PathBuf,
}

impl SocketFileGuard {
    const fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl Drop for SocketFileGuard {
    fn drop(&mut self) {
        let _ = remove_socket_if_exists(&self.path);
    }
}

fn remove_socket_if_exists(socket_path: &Path) -> Result<()> {
    match std::fs::remove_file(socket_path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
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
            style_to_model_map: mapping,
            all_speakers: speakers,
            available_models: models,
        })
    }

    fn get_model_id_from_style(&self, style_id: u32) -> u32 {
        let model_id = self.style_to_model_map.get(&style_id).copied();
        model_id.map_or_else(
            || {
                eprintln!(
                    "Warning: Style {style_id} not found in dynamic mapping, using style ID as model ID"
                );
                style_id
            },
            std::convert::identity,
        )
    }

    fn get_model_path(&self, model_id: u32) -> Option<&Path> {
        self.available_models
            .iter()
            .find(|model| model.model_id == model_id)
            .map(|model| model.file_path.as_path())
    }

    fn speakers_list_response(&self) -> OwnedResponse {
        let speakers = self.all_speakers.clone();
        let style_to_model = self.style_to_model_map.clone();
        OwnedResponse::SpeakersListWithModels {
            speakers,
            style_to_model,
        }
    }

    fn models_list_response(&self) -> OwnedResponse {
        let models = self.available_models.clone();
        OwnedResponse::ModelsList { models }
    }

    fn unload_model_if_known(core: &VoicevoxCore, model_id: u32, model_path: Option<&Path>) {
        let Some(model_path) = model_path else {
            eprintln!("Model {model_id} not found in available models");
            return;
        };

        if let Err(e) = core.unload_voice_model_by_path(model_path) {
            eprintln!("Failed to unload model {model_id}: {e}");
        }
    }

    async fn synthesize_response(&self, text: String, style_id: u32, rate: f32) -> OwnedResponse {
        let model_id = self.get_model_id_from_style(style_id);
        let model_path = self.get_model_path(model_id);

        let synthesis_result = {
            let core = self.core.lock().await;
            if let Err(e) = core.load_specific_model(&model_id.to_string()) {
                eprintln!("Failed to load model {model_id}: {e}");
                return OwnedResponse::Error {
                    message: format!("Failed to load model {model_id} for synthesis: {e}"),
                };
            }

            let synthesis_result = core.synthesize_with_rate(&text, style_id, rate);
            Self::unload_model_if_known(&core, model_id, model_path);
            drop(core);
            synthesis_result
        };

        match synthesis_result {
            Ok(wav_data) => OwnedResponse::SynthesizeResult { wav_data },
            Err(e) => OwnedResponse::Error {
                message: format!("Synthesis failed: {e}"),
            },
        }
    }

    pub async fn handle_request(&self, request: OwnedRequest) -> OwnedResponse {
        match request {
            OwnedRequest::Ping => OwnedResponse::Pong,
            OwnedRequest::Synthesize {
                text,
                style_id,
                options,
            } => self.synthesize_response(text, style_id, options.rate).await,
            OwnedRequest::ListSpeakers => self.speakers_list_response(),
            OwnedRequest::ListModels => self.models_list_response(),
        }
    }
}

fn decode_request_frame(data: &[u8]) -> Result<DaemonRequest> {
    bincode::serde::decode_from_slice::<DaemonRequest, _>(data, bincode::config::standard())
        .map(|(request, _)| request)
        .map_err(Into::into)
}

fn encode_response_frame(response: &OwnedResponse) -> Result<Vec<u8>> {
    bincode::serde::encode_to_vec(response, bincode::config::standard()).map_err(Into::into)
}

/// Handles a single connected daemon client until the stream closes or decoding fails.
///
/// # Errors
///
/// Returns an error if reading from or writing to the framed Unix stream fails.
pub async fn handle_client(stream: UnixStream, state: Arc<DaemonState>) -> Result<()> {
    let mut framed = Framed::new(stream, LengthDelimitedCodec::new());

    while let Some(frame) = framed.next().await {
        let data = match frame {
            Ok(data) => data,
            Err(error) => {
                eprintln!("Client stream read error: {error}");
                break;
            }
        };

        let request = match decode_request_frame(&data) {
            Ok(request) => request,
            Err(error) => {
                eprintln!("Failed to decode client request: {error}");
                break;
            }
        };

        let response = state.handle_request(request).await;
        let response_data = match encode_response_frame(&response) {
            Ok(response_data) => response_data,
            Err(error) => {
                eprintln!("Failed to encode daemon response: {error}");
                break;
            }
        };

        if let Err(error) = framed.send(response_data.into()).await {
            eprintln!("Client stream write error: {error}");
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
    remove_socket_if_exists(&socket_path)?;

    let _socket_guard = SocketFileGuard::new(socket_path.clone());
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
                if let Err(error) = handle_client(stream, state_clone).await {
                    eprintln!("Client handler error: {error}");
                }
            });
        }
        #[allow(unreachable_code)]
        Ok::<(), anyhow::Error>(())
    };

    tokio::select! {
        result = server => result?,
        result = wait_for_shutdown_signal() => result?,
    }

    remove_socket_if_exists(&socket_path)?;

    println!("VOICEVOX daemon stopped");
    Ok(())
}
