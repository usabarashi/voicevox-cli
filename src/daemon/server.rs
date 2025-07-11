use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::{UnixListener, UnixStream};
use tokio::signal;
use tokio::sync::Mutex;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

use crate::core::VoicevoxCore;
use crate::ipc::{DaemonRequest, OwnedRequest, OwnedResponse};
use std::borrow::Cow;
use std::time::Instant;

// Model cache with LRU-style tracking and usage statistics
#[derive(Debug, Clone)]
pub struct ModelCache {
    // Model ID -> Last used time
    pub loaded_models: HashMap<u32, Instant>,
    // Model ID -> Usage count
    pub usage_stats: HashMap<u32, usize>,
    // Maximum number of models to keep loaded
    pub max_models: usize,
    // Models that should never be unloaded (favorites)
    pub favorites: HashSet<u32>,
}

impl ModelCache {
    pub fn new(max_models: usize) -> Self {
        Self::with_favorites(max_models, HashSet::from([3, 2, 8]))
    }

    pub fn with_favorites(max_models: usize, favorites: HashSet<u32>) -> Self {
        Self {
            loaded_models: HashMap::new(),
            usage_stats: HashMap::new(),
            max_models,
            favorites,
        }
    }

    pub fn is_loaded(&self, model_id: u32) -> bool {
        self.loaded_models.contains_key(&model_id)
    }

    pub fn mark_used(&mut self, model_id: u32) {
        self.loaded_models.insert(model_id, Instant::now());
        *self.usage_stats.entry(model_id).or_insert(0) += 1;
    }

    pub fn add_model(&mut self, model_id: u32) {
        self.loaded_models.insert(model_id, Instant::now());
        self.usage_stats.entry(model_id).or_insert(0);
    }

    // Get the least recently used model that's not a favorite
    pub fn get_lru_model(&self) -> Option<u32> {
        self.loaded_models
            .iter()
            .filter(|(id, _)| !self.favorites.contains(id))
            .min_by_key(|(_, time)| *time)
            .map(|(id, _)| *id)
    }

    pub fn should_evict(&self) -> bool {
        self.loaded_models.len() >= self.max_models
    }

    pub fn remove_model(&mut self, model_id: u32) {
        self.loaded_models.remove(&model_id);
    }

    pub fn get_stats(&self) -> String {
        format!(
            "Loaded: {} models, Max: {}, Stats: {:?}",
            self.loaded_models.len(),
            self.max_models,
            self.usage_stats
        )
    }
}

pub struct DaemonState {
    core: VoicevoxCore,
    loaded_models: Arc<Mutex<HashSet<u32>>>,
    model_cache: Arc<Mutex<ModelCache>>,
    style_to_model_map: Arc<Mutex<HashMap<u32, u32>>>,
    all_speakers: Arc<Mutex<Vec<crate::voice::Speaker>>>,
    #[cfg(unix)]
    pending_fd: Arc<Mutex<Option<std::os::unix::io::RawFd>>>,
}

impl DaemonState {
    pub async fn new() -> Result<Self> {
        // Load configuration
        let config = crate::config::Config::load().unwrap_or_else(|e| {
            eprintln!("Failed to load config, using defaults: {}", e);
            crate::config::Config::default()
        });

        Self::with_config(config).await
    }

    pub async fn with_config(config: crate::config::Config) -> Result<Self> {
        let core = VoicevoxCore::new()?;
        let loaded_models = Arc::new(Mutex::new(HashSet::new()));
        let style_to_model_map = Arc::new(Mutex::new(HashMap::new()));

        // Create model cache with configuration
        let model_cache = if config.memory.enable_lru_cache {
            Arc::new(Mutex::new(ModelCache::with_favorites(
                config.memory.max_loaded_models,
                config.models.favorites.clone(),
            )))
        } else {
            // Disable LRU by setting very high limit
            Arc::new(Mutex::new(ModelCache::with_favorites(
                999,
                config.models.favorites.clone(),
            )))
        };

        // Build dynamic style-to-model mapping
        println!("Building dynamic style-to-model mapping...");
        let (mapping, speakers) = crate::voice::build_style_to_model_map_async(&core).await?;
        *style_to_model_map.lock().await = mapping;
        let all_speakers = Arc::new(Mutex::new(speakers));
        println!(
            "  ✓ Discovered {} style mappings",
            style_to_model_map.lock().await.len()
        );

        // Load models specified in config
        let preload_models = config.models.preload;

        println!(
            "Loading {} models from configuration...",
            preload_models.len()
        );
        for model_id in preload_models {
            match core.load_specific_model(&model_id.to_string()) {
                Ok(_) => {
                    loaded_models.lock().await.insert(model_id);
                    model_cache.lock().await.add_model(model_id);
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

        Ok(DaemonState {
            core,
            loaded_models,
            model_cache,
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

    // Ensure a model is loaded before use with memory management
    async fn ensure_model_loaded(&self, model_id: u32) -> Result<()> {
        let mut loaded = self.loaded_models.lock().await;
        let mut cache = self.model_cache.lock().await;

        if !loaded.contains(&model_id) {
            // Check if we need to evict a model
            if cache.should_evict() {
                if let Some(lru_model) = cache.get_lru_model() {
                    println!("Memory limit reached. Evicting model {} (LRU)", lru_model);

                    // Actually unload the model using the new method
                    let models_dir = crate::paths::find_models_dir_client().unwrap_or_else(|_| {
                        std::path::PathBuf::from("~/.local/share/voicevox/models/vvms")
                    });
                    let model_path = models_dir.join(format!("{}.vvm", lru_model));
                    match self
                        .core
                        .unload_voice_model_by_path(model_path.to_str().unwrap_or(""))
                    {
                        Ok(_) => {
                            loaded.remove(&lru_model);
                            cache.remove_model(lru_model);
                            println!("  ✓ Model {} unloaded successfully", lru_model);
                        }
                        Err(e) => {
                            eprintln!("  ✗ Failed to unload model {}: {}", lru_model, e);
                            // Continue anyway - remove from tracking
                            loaded.remove(&lru_model);
                            cache.remove_model(lru_model);
                        }
                    }
                }
            }

            println!("Dynamically loading model {} on demand...", model_id);
            self.core.load_specific_model(&model_id.to_string())?;
            loaded.insert(model_id);
            cache.add_model(model_id);
            println!("  ✓ Model {} loaded successfully", model_id);
        }

        // Mark model as used
        cache.mark_used(model_id);

        Ok(())
    }

    pub async fn handle_request(&self, request: OwnedRequest) -> OwnedResponse {
        match request {
            OwnedRequest::Ping => OwnedResponse::Pong,

            OwnedRequest::Synthesize {
                text,
                style_id,
                options,
            } => {
                // Ensure the required model is loaded
                let model_id = self.get_model_id_from_style(style_id).await;

                match self.ensure_model_loaded(model_id).await {
                    Ok(_) => {
                        // Model is loaded, proceed with synthesis
                        match self.core.synthesize(&text, style_id) {
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
                println!("Loading model: {}", model_name);

                // Try to parse model_id from model_name
                if let Ok(model_id) = model_name.parse::<u32>() {
                    // Use the same logic as ensure_model_loaded
                    match self.ensure_model_loaded(model_id).await {
                        Ok(_) => {
                            println!("Model {} ready for use", model_name);
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
    let config = crate::config::Config::default();
    run_daemon_with_config(socket_path, foreground, config).await
}

pub async fn run_daemon_with_config(
    socket_path: PathBuf,
    foreground: bool,
    config: crate::config::Config,
) -> Result<()> {
    // Remove existing socket file if it exists
    if socket_path.exists() {
        std::fs::remove_file(&socket_path)?;
    }

    // Create Unix socket listener
    let listener = UnixListener::bind(&socket_path)?;
    println!("VOICEVOX daemon started successfully");
    println!("Listening on: {}", socket_path.display());

    // Initialize daemon state with config
    let state = Arc::new(Mutex::new(DaemonState::with_config(config).await?));

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
