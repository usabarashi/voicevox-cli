use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::{UnixListener, UnixStream};
use tokio::signal;
use tokio::sync::Mutex;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

use crate::core::{CoreSynthesis, VoicevoxCore};
use crate::ipc::{DaemonRequest, OwnedRequest, OwnedResponse};

pub struct DaemonState {
    core: VoicevoxCore,
    style_to_model_map: Arc<Mutex<HashMap<u32, u32>>>,
    all_speakers: Arc<Mutex<Vec<crate::voice::Speaker>>>,
    available_models: Arc<Mutex<Vec<crate::voice::AvailableModel>>>,
}

impl DaemonState {
    pub async fn new() -> Result<Self> {
        let core = VoicevoxCore::new()?;
        let style_to_model_map = Arc::new(Mutex::new(HashMap::new()));

        println!("Building dynamic style-to-model mapping...");
        let (mapping, speakers, models) =
            crate::voice::build_style_to_model_map_async_with_progress(
                &core,
                |current, total, filename| {
                    println!("  Loading model {} ({}/{})", filename, current, total);
                },
            )
            .await?;
        *style_to_model_map.lock().await = mapping;
        let all_speakers = Arc::new(Mutex::new(speakers));
        let available_models = Arc::new(Mutex::new(models));
        println!(
            "Discovered {} style mappings",
            style_to_model_map.lock().await.len()
        );

        println!("Models will be loaded and unloaded per synthesis request.");

        Ok(DaemonState {
            core,
            style_to_model_map,
            all_speakers,
            available_models,
        })
    }

    async fn get_model_id_from_style(&self, style_id: u32) -> u32 {
        let map = self.style_to_model_map.lock().await;

        if let Some(&model_id) = map.get(&style_id) {
            return model_id;
        }
        eprintln!(
            "Warning: Style {style_id} not found in dynamic mapping, using style ID as model ID"
        );
        style_id
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

                if let Err(e) = self.core.load_specific_model(&model_id.to_string()) {
                    eprintln!("Failed to load model {model_id}: {e}");
                    return OwnedResponse::Error {
                        message: format!("Failed to load model {model_id} for synthesis: {e}"),
                    };
                }

                println!("Loaded model {model_id} for synthesis");

                let synthesis_result = self.core.synthesize(&text, style_id);
                let available_models = self.available_models.lock().await;
                if let Some(model) = available_models.iter().find(|m| m.model_id == model_id) {
                    let path_str = match model.file_path.to_str() {
                        Some(s) => s,
                        None => {
                            eprintln!(
                                "Model path contains invalid UTF-8: {:?}",
                                model.file_path
                            );
                            return OwnedResponse::Error {
                                message: format!(
                                    "Model path contains invalid UTF-8: {:?}",
                                    model.file_path
                                ),
                            };
                        }
                    };
                    match self.core.unload_voice_model_by_path(path_str) {
                        Ok(_) => println!("Unloaded model {model_id} after synthesis"),
                        Err(e) => eprintln!("Failed to unload model {model_id}: {e}"),
                    }
                } else {
                    eprintln!("Model {model_id} not found in available models");
                }

                match synthesis_result {
                    Ok(wav_data) => OwnedResponse::SynthesizeResult { wav_data },
                    Err(e) => {
                        eprintln!("Synthesis failed: {e}");
                        OwnedResponse::Error {
                            message: format!("Synthesis failed: {e}"),
                        }
                    }
                }
            }

            OwnedRequest::ListSpeakers => {
                let all_speakers = self.all_speakers.lock().await.clone();
                let style_to_model = self.style_to_model_map.lock().await.clone();
                OwnedResponse::SpeakersListWithModels {
                    speakers: all_speakers,
                    style_to_model,
                }
            }

            OwnedRequest::ListModels => {
                let models = self.available_models.lock().await.clone();
                OwnedResponse::ModelsList { models }
            }
        }
    }
}

pub async fn handle_client(mut stream: UnixStream, state: Arc<Mutex<DaemonState>>) -> Result<()> {
    loop {
        let request = {
            let (reader, _writer) = stream.split();
            let mut framed_reader = FramedRead::new(reader, LengthDelimitedCodec::new());

            match framed_reader.next().await {
                Some(Ok(data)) => match bincode::serde::decode_from_slice::<DaemonRequest, _>(
                    &data,
                    bincode::config::standard(),
                ) {
                    Ok((req, _)) => req,
                    Err(e) => {
                        println!("Failed to deserialize request: {e}");
                        break;
                    }
                },
                _ => break,
            }
        };

        let response = {
            let state = state.lock().await;
            state.handle_request(request).await
        };

        {
            let (_reader, writer) = stream.split();
            let mut framed_writer = FramedWrite::new(writer, LengthDelimitedCodec::new());

            match bincode::serde::encode_to_vec(&response, bincode::config::standard()) {
                Ok(response_data) => {
                    if let Err(e) = framed_writer.send(response_data.into()).await {
                        println!("Failed to send response: {e}");
                        break;
                    }
                }
                Err(e) => {
                    println!("Failed to serialize response: {e}");
                    break;
                }
            }
        }
    }

    Ok(())
}

pub async fn run_daemon(socket_path: PathBuf, foreground: bool) -> Result<()> {
    if socket_path.exists() {
        std::fs::remove_file(&socket_path)?;
    }

    let listener = UnixListener::bind(&socket_path)?;
    println!("VOICEVOX daemon started successfully");
    println!("Listening on: {}", socket_path.display());

    let state = Arc::new(Mutex::new(DaemonState::new().await?));

    if !foreground {
        println!("Running in background mode. Use Ctrl+C to stop gracefully.");
    }

    let shutdown = async {
        signal::ctrl_c().await.expect("Failed to listen for ctrl-c");
        println!("\nShutting down daemon...");
    };

    let server = async {
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let state_clone = Arc::clone(&state);
                    tokio::spawn(async move {
                        if let Err(e) = handle_client(stream, state_clone).await {
                            println!("Client handler error: {e}");
                        }
                    });
                }
                Err(e) => {
                    println!("Failed to accept connection: {e}");
                }
            }
        }
    };

    tokio::select! {
        _ = server => {},
        _ = shutdown => {},
    }

    if socket_path.exists() {
        std::fs::remove_file(&socket_path)?;
    }

    println!("VOICEVOX daemon stopped");
    Ok(())
}
