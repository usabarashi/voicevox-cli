use anyhow::{anyhow, Result};
use clap::{Arg, Command};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::process;
// use tokio::io::{AsyncReadExt, AsyncWriteExt}; // Not needed for this implementation
use tokio::net::{UnixListener, UnixStream};
use tokio::signal;
use tokio::sync::Mutex;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};
use futures_util::{SinkExt, StreamExt};

use voicevox_tts::{
    get_socket_path, get_voice_mapping, resolve_voice_name, DaemonRequest, DaemonResponse,
    VoicevoxCore,
};

struct DaemonState {
    core: VoicevoxCore,
}

impl DaemonState {
    async fn new() -> Result<Self> {
        
        let core = VoicevoxCore::new()?;
        
        // Always load all models for daemon
        if let Err(e) = core.load_all_models() {
            eprintln!("Warning: Failed to load some models: {}", e);
        }
        
        Ok(DaemonState { core })
    }
    
    async fn handle_request(&self, request: DaemonRequest) -> DaemonResponse {
        match request {
            DaemonRequest::Ping => {
                DaemonResponse::Pong
            }
            
            DaemonRequest::Synthesize { text, style_id, options: _ } => {
                match self.core.synthesize(&text, style_id) {
                    Ok(wav_data) => {
                        DaemonResponse::SynthesizeResult { wav_data }
                    }
                    Err(e) => {
                        eprintln!("Synthesis failed: {}", e);
                        DaemonResponse::Error {
                            message: format!("Synthesis failed: {}", e),
                        }
                    }
                }
            }
            
            DaemonRequest::ListSpeakers => {
                match self.core.get_speakers() {
                    Ok(speakers) => {
                        DaemonResponse::SpeakersList { speakers }
                    }
                    Err(e) => {
                        eprintln!("Failed to get speakers: {}", e);
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

async fn run_daemon(socket_path: PathBuf, foreground: bool) -> Result<()> {
    // Remove existing socket file if it exists
    if socket_path.exists() {
        std::fs::remove_file(&socket_path)?;
    }
    
    // Create Unix socket listener
    let listener = UnixListener::bind(&socket_path)?;
    println!("🎧 VOICEVOX daemon listening on: {}", socket_path.display());
    
    // Initialize daemon state
    let state = Arc::new(Mutex::new(DaemonState::new().await?));
    
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
    
    let foreground = matches.get_flag("foreground");
    
    // Check for existing daemon process
    if let Err(e) = check_and_prevent_duplicate(&socket_path).await {
        eprintln!("❌ {}", e);
        std::process::exit(1);
    }
    
    // Display startup banner
    println!("🫛 VOICEVOX Daemon v{}", env!("CARGO_PKG_VERSION"));
    println!("Socket: {}", socket_path.display());
    println!("Mode: All models (best compatibility)");
    println!();
    
    run_daemon(socket_path, foreground).await
}

// Check for existing daemon and prevent duplicate processes
async fn check_and_prevent_duplicate(socket_path: &PathBuf) -> Result<()> {
    // Check if socket file exists
    if socket_path.exists() {
        // Try to connect to existing daemon
        match tokio::net::UnixStream::connect(socket_path).await {
            Ok(_) => {
                return Err(anyhow!(
                    "VOICEVOX daemon is already running at {}. Use 'pkill -f voicevox-daemon' to stop it.",
                    socket_path.display()
                ));
            }
            Err(_) => {
                // Socket exists but no daemon responding, remove stale socket
                println!("🧹 Removing stale socket file: {}", socket_path.display());
                if let Err(e) = fs::remove_file(socket_path) {
                    return Err(anyhow!("Failed to remove stale socket: {}", e));
                }
            }
        }
    }
    
    // Check for running daemon processes
    match process::Command::new("pgrep")
        .arg("-f")
        .arg("voicevox-daemon")
        .output()
    {
        Ok(output) => {
            if output.status.success() && !output.stdout.is_empty() {
                let pids = String::from_utf8_lossy(&output.stdout);
                let current_pid = process::id();
                let other_pids: Vec<&str> = pids
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .filter(|pid| pid.trim().parse::<u32>().unwrap_or(0) != current_pid)
                    .collect();
                
                if !other_pids.is_empty() {
                    return Err(anyhow!(
                        "VOICEVOX daemon process(es) already running (PIDs: {}). Stop them first with 'pkill -f voicevox-daemon'",
                        other_pids.join(", ")
                    ));
                }
            }
        }
        Err(_) => {
            // pgrep not available, continue anyway
            println!("⚠️  Could not check for existing processes (pgrep not available)");
        }
    }
    
    Ok(())
}