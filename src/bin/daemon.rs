use anyhow::Result;
use clap::{Arg, Command};
use std::path::PathBuf;
use std::sync::Arc;
// use tokio::io::{AsyncReadExt, AsyncWriteExt}; // Not needed for this implementation
use tokio::net::{UnixListener, UnixStream};
use tokio::signal;
use tokio::sync::Mutex;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};
use futures_util::{SinkExt, StreamExt};

use voicevox_cli::{
    get_socket_path, get_voice_mapping, resolve_voice_name, DaemonRequest, DaemonResponse,
    VoicevoxCore,
};

struct DaemonState {
    core: VoicevoxCore,
}

impl DaemonState {
    async fn new(minimal_models: bool) -> Result<Self> {
        println!("🚀 Initializing VOICEVOX Core for daemon...");
        
        let core = VoicevoxCore::new()?;
        
        if minimal_models {
            println!("📦 Loading minimal models for faster startup...");
            if let Err(e) = core.load_minimal_models() {
                println!("⚠️  Warning: Failed to load some minimal models: {}", e);
            }
        } else {
            println!("📦 Loading all available models for best user experience...");
            if let Err(e) = core.load_all_models() {
                println!("⚠️  Warning: Failed to load some models: {}", e);
            }
        }
        
        println!("✅ VOICEVOX Core daemon initialized successfully");
        
        Ok(DaemonState { core })
    }
    
    async fn handle_request(&self, request: DaemonRequest) -> DaemonResponse {
        match request {
            DaemonRequest::Ping => {
                println!("🏓 Ping received");
                DaemonResponse::Pong
            }
            
            DaemonRequest::Synthesize { text, style_id, options } => {
                println!("🎤 Synthesizing: \"{}\" with style ID {} (rate: {})", 
                    if text.chars().count() > 50 { 
                        format!("{}...", text.chars().take(50).collect::<String>()) 
                    } else { 
                        text.clone() 
                    }, 
                    style_id, 
                    options.rate
                );
                
                match self.core.synthesize(&text, style_id) {
                    Ok(wav_data) => {
                        println!("✅ Synthesis completed ({} bytes)", wav_data.len());
                        DaemonResponse::SynthesizeResult { wav_data }
                    }
                    Err(e) => {
                        println!("❌ Synthesis failed: {}", e);
                        DaemonResponse::Error {
                            message: format!("Synthesis failed: {}", e),
                        }
                    }
                }
            }
            
            DaemonRequest::ListSpeakers => {
                println!("📋 Listing speakers");
                match self.core.get_speakers() {
                    Ok(speakers) => {
                        println!("✅ Found {} speakers", speakers.len());
                        DaemonResponse::SpeakersList { speakers }
                    }
                    Err(e) => {
                        println!("❌ Failed to get speakers: {}", e);
                        DaemonResponse::Error {
                            message: format!("Failed to get speakers: {}", e),
                        }
                    }
                }
            }
            
            DaemonRequest::LoadModel { model_name } => {
                println!("📦 Loading model: {}", model_name);
                match self.core.load_specific_model(&model_name) {
                    Ok(_) => {
                        println!("✅ Model loaded successfully: {}", model_name);
                        DaemonResponse::Success
                    }
                    Err(e) => {
                        println!("❌ Failed to load model {}: {}", model_name, e);
                        DaemonResponse::Error {
                            message: format!("Failed to load model {}: {}", model_name, e),
                        }
                    }
                }
            }
            
            DaemonRequest::GetVoiceMapping => {
                println!("🎭 Getting voice mapping");
                let mapping = get_voice_mapping();
                let mapping_strings: std::collections::HashMap<String, (u32, String)> = mapping
                    .into_iter()
                    .map(|(k, (id, desc))| (k.to_string(), (id, desc.to_string())))
                    .collect();
                DaemonResponse::VoiceMapping {
                    mapping: mapping_strings,
                }
            }
            
            DaemonRequest::ResolveVoiceName { voice_name } => {
                println!("🔍 Resolving voice name: {}", voice_name);
                match resolve_voice_name(&voice_name) {
                    Ok((style_id, description)) => {
                        println!("✅ Resolved to style ID {} ({})", style_id, description);
                        DaemonResponse::VoiceResolution {
                            style_id,
                            description,
                        }
                    }
                    Err(e) => {
                        println!("❌ Failed to resolve voice name {}: {}", voice_name, e);
                        DaemonResponse::Error {
                            message: format!("Failed to resolve voice name {}: {}", voice_name, e),
                        }
                    }
                }
            }
        }
    }
}

async fn handle_client(stream: UnixStream, state: Arc<Mutex<DaemonState>>) -> Result<()> {
    let (reader, writer) = stream.into_split();
    let mut framed_reader = FramedRead::new(reader, LengthDelimitedCodec::new());
    let mut framed_writer = FramedWrite::new(writer, LengthDelimitedCodec::new());
    
    println!("🔗 New client connected");
    
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
                                    println!("❌ Failed to send response: {}", e);
                                    break;
                                }
                            }
                            Err(e) => {
                                println!("❌ Failed to serialize response: {}", e);
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        println!("❌ Failed to deserialize request: {}", e);
                        let error_response = DaemonResponse::Error {
                            message: format!("Failed to deserialize request: {}", e),
                        };
                        
                        if let Ok(error_data) = bincode::serialize(&error_response) {
                            let _ = framed_writer.send(error_data.into()).await;
                        }
                        break;
                    }
                }
            }
            Err(e) => {
                println!("❌ Frame error: {}", e);
                break;
            }
        }
    }
    
    println!("🔌 Client disconnected");
    Ok(())
}

async fn run_daemon(socket_path: PathBuf, minimal_models: bool, foreground: bool) -> Result<()> {
    // Remove existing socket file if it exists
    if socket_path.exists() {
        std::fs::remove_file(&socket_path)?;
    }
    
    // Create Unix socket listener
    let listener = UnixListener::bind(&socket_path)?;
    println!("🎧 VOICEVOX daemon listening on: {}", socket_path.display());
    
    // Initialize daemon state
    let state = Arc::new(Mutex::new(DaemonState::new(minimal_models).await?));
    
    if !foreground {
        println!("🌙 Running in background mode. Use Ctrl+C to stop gracefully.");
    }
    
    // Set up graceful shutdown
    let shutdown = async {
        signal::ctrl_c().await.expect("Failed to listen for ctrl-c");
        println!("\n🛑 Received shutdown signal, cleaning up...");
    };
    
    // Accept connections
    let server = async {
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let state_clone = Arc::clone(&state);
                    tokio::spawn(async move {
                        if let Err(e) = handle_client(stream, state_clone).await {
                            println!("❌ Client handler error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    println!("❌ Failed to accept connection: {}", e);
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
        println!("🧹 Cleaned up socket file");
    }
    
    println!("👋 VOICEVOX daemon stopped");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let app = Command::new("voicevox-daemon")
        .version(env!("CARGO_PKG_VERSION"))
        .about("🫛 VOICEVOX Daemon - Background TTS service with pre-loaded models")
        .arg(
            Arg::new("socket-path")
                .help("Specify custom Unix socket path")
                .long("socket-path")
                .short('s')
                .value_name("PATH"),
        )
        .arg(
            Arg::new("minimal-models")
                .help("Load only minimal models for faster startup")
                .long("minimal-models")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("foreground")
                .help("Run in foreground (don't daemonize)")
                .long("foreground")
                .short('f')
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("models-dir")
                .help("Specify custom models directory")
                .long("models-dir")
                .value_name("PATH"),
        )
        .arg(
            Arg::new("dict-dir")
                .help("Specify custom OpenJTalk dictionary directory")
                .long("dict-dir")
                .value_name("PATH"),
        );
    
    let matches = app.get_matches();
    
    // Override environment variables if provided via CLI
    if let Some(models_dir) = matches.get_one::<String>("models-dir") {
        std::env::set_var("VOICEVOX_MODELS_DIR", models_dir);
    }
    if let Some(dict_dir) = matches.get_one::<String>("dict-dir") {
        std::env::set_var("VOICEVOX_DICT_DIR", dict_dir);
    }
    
    // Determine socket path
    let socket_path = if let Some(custom_path) = matches.get_one::<String>("socket-path") {
        PathBuf::from(custom_path)
    } else {
        get_socket_path()
    };
    
    let minimal_models = matches.get_flag("minimal-models");
    let foreground = matches.get_flag("foreground");
    
    // Display startup banner
    println!("🫛 VOICEVOX Daemon v{}", env!("CARGO_PKG_VERSION"));
    println!("Socket: {}", socket_path.display());
    if minimal_models {
        println!("Mode: Minimal models (faster startup)");
    } else {
        println!("Mode: All models (best compatibility)");
    }
    println!();
    
    run_daemon(socket_path, minimal_models, foreground).await
}