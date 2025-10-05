use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
#[cfg(unix)]
use libc::{getegid, geteuid};
use std::collections::HashMap;
#[cfg(unix)]
use std::env;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::net::{UnixListener, UnixStream};
use tokio::signal;
use tokio::sync::Mutex;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

use crate::core::{CoreSynthesis, VoicevoxCore};
use crate::ipc::{DaemonRequest, OwnedRequest, OwnedResponse};

#[cfg(unix)]
fn secure_socket_dir_hierarchy(dir: &Path) -> Result<()> {
    let boundary_candidates = [
        env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from),
        env::var_os("XDG_STATE_HOME").map(PathBuf::from),
        env::var_os("HOME").map(PathBuf::from),
    ];

    let mut boundary: Option<PathBuf> = None;
    let mut current_dir = dir;
    let mut new_dirs = Vec::new();

    while let Some(parent) = current_dir.parent() {
        if parent.as_os_str().is_empty() {
            break;
        }

        if boundary_candidates
            .iter()
            .flatten()
            .any(|candidate| !candidate.as_os_str().is_empty() && parent == candidate.as_path())
        {
            boundary = Some(parent.to_path_buf());
            break;
        }

        if !parent.exists() {
            new_dirs.push(parent.to_path_buf());
        }

        current_dir = parent;
    }

    let boundary = boundary.ok_or_else(|| {
        anyhow::anyhow!(
            "Socket directory resides outside of approved bases: {}",
            dir.display()
        )
    })?;

    let boundary_meta = fs::symlink_metadata(&boundary).with_context(|| {
        format!(
            "Failed to inspect socket directory permissions: {}",
            boundary.display()
        )
    })?;

    if boundary_meta.file_type().is_symlink() {
        return Err(anyhow::anyhow!(
            "Socket path traverses a symlink: {}",
            boundary.display()
        ));
    }

    let uid = unsafe { geteuid() } as u32;
    let gid = unsafe { getegid() } as u32;
    if boundary_meta.uid() != uid || boundary_meta.gid() != gid {
        return Err(anyhow::anyhow!(
            "Socket directory must be owned by the current user: {}",
            boundary.display()
        ));
    }

    for component in new_dirs.into_iter().rev() {
        fs::create_dir(&component).with_context(|| {
            format!(
                "Failed to create socket directory component: {}",
                component.display()
            )
        })?;

        let mut permissions = fs::metadata(&component)
            .with_context(|| {
                format!(
                    "Failed to inspect socket directory permissions: {}",
                    component.display()
                )
            })?
            .permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&component, permissions).with_context(|| {
            format!(
                "Failed to tighten socket directory permissions: {}",
                component.display()
            )
        })?;
    }

    if !dir.exists() {
        fs::create_dir(dir).with_context(|| {
            format!(
                "Failed to create socket directory component: {}",
                dir.display()
            )
        })?;
    }

    let metadata = fs::symlink_metadata(dir).with_context(|| {
        format!(
            "Failed to inspect socket directory permissions: {}",
            dir.display()
        )
    })?;

    if metadata.file_type().is_symlink() {
        return Err(anyhow::anyhow!(
            "Socket path traverses a symlink: {}",
            dir.display()
        ));
    }

    if metadata.uid() != uid || metadata.gid() != gid {
        return Err(anyhow::anyhow!(
            "Socket directory must be owned by the current user: {}",
            dir.display()
        ));
    }

    let mut permissions = metadata.permissions();
    let mode = permissions.mode();
    if mode & 0o077 != 0 {
        permissions.set_mode(0o700);
        fs::set_permissions(dir, permissions).with_context(|| {
            format!(
                "Failed to tighten socket directory permissions: {}",
                dir.display()
            )
        })?;
    }

    Ok(())
}

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

        let (mapping, speakers, models) =
            crate::voice::build_style_to_model_map_async_with_progress(&core, |_, _, _| {}).await?;
        *style_to_model_map.lock().await = mapping;
        let all_speakers = Arc::new(Mutex::new(speakers));
        let available_models = Arc::new(Mutex::new(models));

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

                let synthesis_result = self.core.synthesize(&text, style_id);
                let available_models = self.available_models.lock().await;
                if let Some(model) = available_models.iter().find(|m| m.model_id == model_id) {
                    let path_str = match model.file_path.to_str() {
                        Some(s) => s,
                        None => {
                            eprintln!("Model path contains invalid UTF-8: {:?}", model.file_path);
                            return OwnedResponse::Error {
                                message: format!(
                                    "Model path contains invalid UTF-8: {:?}",
                                    model.file_path
                                ),
                            };
                        }
                    };
                    match self.core.unload_voice_model_by_path(path_str) {
                        Ok(_) => {}
                        Err(e) => eprintln!("Failed to unload model {model_id}: {e}"),
                    }
                } else {
                    eprintln!("Model {model_id} not found in available models");
                }

                match synthesis_result {
                    Ok(wav_data) => OwnedResponse::SynthesizeResult { wav_data },
                    Err(e) => OwnedResponse::Error {
                        message: format!("Synthesis failed: {e}"),
                    },
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
                    Err(_) => {
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
                    if framed_writer.send(response_data.into()).await.is_err() {
                        break;
                    }
                }
                Err(_) => {
                    break;
                }
            }
        }
    }

    Ok(())
}

pub async fn run_daemon(socket_path: PathBuf, foreground: bool) -> Result<()> {
    if let Some(parent) = socket_path.parent() {
        if !parent.as_os_str().is_empty() {
            let mut builder = fs::DirBuilder::new();
            builder.recursive(true);
            #[cfg(unix)]
            {
                builder.mode(0o700);
            }

            builder.create(parent).with_context(|| {
                format!(
                    "Failed to create socket parent directory: {}",
                    parent.display()
                )
            })?;

            #[cfg(unix)]
            {
                secure_socket_dir_hierarchy(parent)?;
            }
        }
    }

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
            if let Ok((stream, _)) = listener.accept().await {
                let state_clone = Arc::clone(&state);
                tokio::spawn(async move {
                    let _ = handle_client(stream, state_clone).await;
                });
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
