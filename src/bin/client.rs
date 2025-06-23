use anyhow::{anyhow, Result};
use clap::{Arg, Command};
use std::fs;
use std::io::{self, Read, Cursor};
use std::path::PathBuf;
use std::time::Duration;
use std::process::{Command as ProcessCommand, Stdio};
use rodio::{Decoder, OutputStream, Sink};
// use tokio::io::{AsyncReadExt, AsyncWriteExt}; // Not needed for this implementation
use tokio::net::UnixStream;
use tokio::time::timeout;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};
use futures_util::{SinkExt, StreamExt};

use voicevox_tts::{
    get_socket_path, resolve_voice_name, DaemonRequest, DaemonResponse, SynthesizeOptions,
    VoicevoxCore,
};

// Direct audio playback from memory (like macOS say command)
fn play_audio_from_memory(wav_data: &[u8]) -> Result<()> {
    // Try rodio first for cross-platform compatibility
    if let Ok((_stream, stream_handle)) = OutputStream::try_default() {
        let sink = Sink::try_new(&stream_handle)?;
        // Create owned data for Decoder to avoid lifetime issues
        let wav_data_owned = wav_data.to_vec();
        let cursor = Cursor::new(wav_data_owned);
        
        match Decoder::new(cursor) {
            Ok(source) => {
                sink.append(source);
                sink.sleep_until_end();
                return Ok(());
            }
            Err(_) => {
                // Rodio failed, fall back to system command
            }
        }
    }
    
    // Fallback to system audio players (like original say command behavior)
    play_audio_via_system(wav_data)
}

// System audio playback fallback
fn play_audio_via_system(wav_data: &[u8]) -> Result<()> {
    let temp_file = "/tmp/voicevox_say_temp.wav";
    fs::write(temp_file, wav_data)?;
    
    // macOS standard afplay for playback (silent like say command)
    if let Ok(_) = std::process::Command::new("afplay").arg(temp_file).output() {
        let _ = fs::remove_file(temp_file); // Clean up
        return Ok(());
    }
    
    // sox fallback
    if let Ok(_) = std::process::Command::new("play").arg(temp_file).output() {
        let _ = fs::remove_file(temp_file); // Clean up
        return Ok(());
    }
    
    // Clean up temp file even if playback failed
    let _ = fs::remove_file(temp_file);
    Err(anyhow!("No audio player found. Install sox or use -o to save file"))
}

// Fallback to standalone mode when daemon is not available
async fn standalone_mode(
    text: &str,
    style_id: u32,
    _voice_description: &str,
    output_file: Option<&String>,
    quiet: bool,
    _rate: f32,
    _streaming: bool,
    minimal_models: bool,
) -> Result<()> {
    println!("Initializing VOICEVOX Core...");
    
    let core = VoicevoxCore::new()?;
    
    // Load models
    if minimal_models {
        println!("Loading minimal models for faster startup...");
        if let Err(e) = core.load_minimal_models() {
            println!("Warning: Failed to load some minimal models: {}", e);
        }
    } else {
        println!("Loading all available models...");
        if let Err(e) = core.load_all_models() {
            println!("Warning: Failed to load some models: {}", e);
        }
    }
    
    println!("VOICEVOX Core initialized successfully");
    
    // Synthesize speech
    println!("Synthesizing speech...");
    let wav_data = core.synthesize(text, style_id)?;
    println!("Speech synthesis completed ({} bytes)", wav_data.len());
    
    // Handle output
    if let Some(output_file) = output_file {
        fs::write(output_file, &wav_data)?;
        println!("Audio saved to: {}", output_file);
    }
    
    // Play audio if not quiet and no output file (like macOS say command)
    if !quiet && output_file.is_none() {
        if let Err(e) = play_audio_from_memory(&wav_data) {
            eprintln!("Warning: Audio playback failed: {}", e);
        }
    }
    
    Ok(())
}

// Communicate with daemon
async fn daemon_mode(
    text: &str,
    style_id: u32,
    _voice_description: &str,
    options: SynthesizeOptions,
    output_file: Option<&String>,
    quiet: bool,
    socket_path: &PathBuf,
) -> Result<()> {
    // Connect to daemon with timeout
    let stream = timeout(Duration::from_secs(5), UnixStream::connect(socket_path))
        .await
        .map_err(|_| {
            println!("Daemon connection timeout after 5 seconds");
            anyhow!("Daemon connection timeout")
        })?
        .map_err(|e| {
            println!("Daemon connection failed: {}", e);
            anyhow!("Failed to connect to daemon: {}", e)
        })?;
    
    let (reader, writer) = stream.into_split();
    let mut framed_reader = FramedRead::new(reader, LengthDelimitedCodec::new());
    let mut framed_writer = FramedWrite::new(writer, LengthDelimitedCodec::new());
    
    // Send synthesis request
    let request = DaemonRequest::Synthesize {
        text: text.to_string(),
        style_id,
        options,
    };
    
    let request_data = bincode::serialize(&request)
        .map_err(|e| {
            println!("Failed to serialize request: {}", e);
            anyhow!("Failed to serialize request: {}", e)
        })?;
    
    framed_writer
        .send(request_data.into())
        .await
        .map_err(|e| {
            println!("Failed to send request: {}", e);
            anyhow!("Failed to send request: {}", e)
        })?;
    
    // Receive response
    let response_frame = timeout(Duration::from_secs(30), framed_reader.next())
        .await
        .map_err(|_| {
            println!("Daemon response timeout after 30 seconds");
            anyhow!("Daemon response timeout")
        })?
        .ok_or_else(|| {
            println!("Connection closed by daemon");
            anyhow!("Connection closed by daemon")
        })?
        .map_err(|e| {
            println!("Failed to receive response: {}", e);
            anyhow!("Failed to receive response: {}", e)
        })?;
    
    let response: DaemonResponse = bincode::deserialize(&response_frame)
        .map_err(|e| {
            println!("Failed to deserialize response: {}", e);
            anyhow!("Failed to deserialize response: {}", e)
        })?;
    
    match response {
        DaemonResponse::SynthesizeResult { wav_data } => {
            
            // Handle output
            if let Some(output_file) = output_file {
                fs::write(output_file, &wav_data)?;
            }
            
            // Play audio if not quiet and no output file (like macOS say command)
            if !quiet && output_file.is_none() {
                if let Err(e) = play_audio_from_memory(&wav_data) {
                    println!("Audio playback failed: {}", e);
                }
            }
            
            
            Ok(())
        }
        DaemonResponse::Error { message } => Err(anyhow!("Daemon error: {}", message)),
        _ => Err(anyhow!("Unexpected response from daemon")),
    }
}

// Check daemon status
async fn check_daemon_status(socket_path: &PathBuf) -> Result<()> {
    match UnixStream::connect(socket_path).await {
        Ok(stream) => {
            let (reader, writer) = stream.into_split();
            let mut framed_reader = FramedRead::new(reader, LengthDelimitedCodec::new());
            let mut framed_writer = FramedWrite::new(writer, LengthDelimitedCodec::new());
            
            // Send ping
            let request = DaemonRequest::Ping;
            let request_data = bincode::serialize(&request)?;
            framed_writer.send(request_data.into()).await?;
            
            // Receive response
            if let Some(response_frame) = framed_reader.next().await {
                let response_frame = response_frame?;
                let response: DaemonResponse = bincode::deserialize(&response_frame)?;
                
                match response {
                    DaemonResponse::Pong => {
                        println!("VOICEVOX daemon is running and responsive");
                        println!("Socket: {}", socket_path.display());
                        return Ok(());
                    }
                    _ => {
                        eprintln!("Error: Daemon responded with unexpected message");
                    }
                }
            }
        }
        Err(_) => {
            println!("VOICEVOX daemon is not running");
            println!("Expected socket: {}", socket_path.display());
            println!("Start daemon with: voicevox-daemon");
        }
    }
    Err(anyhow!("Daemon not available"))
}

// List speakers via daemon
async fn list_speakers_daemon(socket_path: &PathBuf) -> Result<()> {
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
        let response: DaemonResponse = bincode::deserialize(&response_frame)?;
        
        match response {
            DaemonResponse::SpeakersList { speakers } => {
                println!("All available speakers and styles from daemon:");
                for speaker in &speakers {
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
            DaemonResponse::Error { message } => {
                return Err(anyhow!("Daemon error: {}", message));
            }
            _ => {
                return Err(anyhow!("Unexpected response from daemon"));
            }
        }
    }
    
    Err(anyhow!("Failed to get speakers from daemon"))
}

// Get text input from various sources
fn get_input_text(matches: &clap::ArgMatches) -> Result<String> {
    // Command line argument
    if let Some(text) = matches.get_one::<String>("text") {
        return Ok(text.clone());
    }
    
    // File input
    if let Some(file_path) = matches.get_one::<String>("input-file") {
        if file_path == "-" {
            // Read from stdin
            let mut buffer = String::new();
            io::stdin().read_to_string(&mut buffer)?;
            return Ok(buffer.trim().to_string());
        } else {
            // Read from file
            return Ok(fs::read_to_string(file_path)?);
        }
    }
    
    // Default to stdin if no text specified
    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer)?;
    Ok(buffer.trim().to_string())
}

// Start daemon process if not already running
async fn start_daemon_if_needed() -> Result<()> {
    // Find daemon binary
    let daemon_path = if let Ok(current_exe) = std::env::current_exe() {
        let mut daemon_path = current_exe.clone();
        daemon_path.set_file_name("voicevox-daemon");
        if daemon_path.exists() {
            daemon_path
        } else {
            PathBuf::from("./target/debug/voicevox-daemon")
        }
    } else {
        PathBuf::from("voicevox-daemon")
    };
    
    // Start daemon process
    let mut cmd = ProcessCommand::new(&daemon_path);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    
    // Set environment variables for dynamic linking
    if let Ok(current_dir) = std::env::current_dir() {
        let lib_path = format!(
            "{}:{}",
            current_dir.join("voicevox_core/c_api/lib").display(),
            current_dir.join("voicevox_core/onnxruntime/lib").display()
        );
        cmd.env("DYLD_LIBRARY_PATH", &lib_path);
    }
    
    match cmd.spawn() {
        Ok(_) => {
            Ok(())
        }
        Err(e) => {
            println!("Failed to start daemon: {}", e);
            Err(anyhow!("Failed to start daemon: {}", e))
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let app = Command::new("voicevox-say")
        .version(env!("CARGO_PKG_VERSION"))
        .about("🫛 VOICEVOX Say - Convert text to audible speech using VOICEVOX")
        .arg(
            Arg::new("text")
                .help("Specify the text to speak on the command line")
                .index(1)
                .required(false),
        )
        .arg(
            Arg::new("voice")
                .help("Specify the voice to be used. Use '?' to list all available voices")
                .long("voice")
                .short('v')
                .value_name("VOICE")
                .default_value("zundamon"),
        )
        .arg(
            Arg::new("rate")
                .help("Speech rate multiplier (0.5-2.0, default: 1.0)")
                .long("rate")
                .short('r')
                .value_name("RATE")
                .value_parser(clap::value_parser!(f32))
                .default_value("1.0"),
        )
        .arg(
            Arg::new("output-file")
                .help("Specify the path for an audio file to be written")
                .long("output-file")
                .short('o')
                .value_name("FILE"),
        )
        .arg(
            Arg::new("input-file")
                .help("Specify a file to be spoken. Use '-' for stdin")
                .long("input-file")
                .short('f')
                .value_name("FILE"),
        )
        .arg(
            Arg::new("streaming")
                .help("Enable streaming synthesis (sentence-by-sentence)")
                .long("streaming")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("quiet")
                .help("Don't play audio, only save to file")
                .long("quiet")
                .short('q')
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("list-speakers")
                .help("List all available speakers and styles")
                .long("list-speakers")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("speaker-id")
                .help("Directly specify speaker style ID (advanced users)")
                .long("speaker-id")
                .short('s')
                .value_name("ID")
                .value_parser(clap::value_parser!(u32))
                .conflicts_with("voice"),
        )
        .arg(
            Arg::new("minimal-models")
                .help("Load only minimal models for faster startup (standalone mode)")
                .long("minimal-models")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("standalone")
                .help("Force standalone mode (don't use daemon)")
                .long("standalone")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("daemon-status")
                .help("Check daemon status and exit")
                .long("daemon-status")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("socket-path")
                .help("Specify custom daemon socket path")
                .long("socket-path")
                .value_name("PATH"),
        )
        .arg(
            Arg::new("models-dir")
                .help("Specify custom models directory (standalone mode)")
                .long("models-dir")
                .value_name("PATH"),
        )
        .arg(
            Arg::new("dict-dir")
                .help("Specify custom OpenJTalk dictionary directory (standalone mode)")
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
    
    // Handle special modes first
    if matches.get_flag("daemon-status") {
        let socket_path = if let Some(custom_path) = matches.get_one::<String>("socket-path") {
            PathBuf::from(custom_path)
        } else {
            get_socket_path()
        };
        return check_daemon_status(&socket_path).await;
    }
    
    // Handle voice list display
    if let Some(voice_name) = matches.get_one::<String>("voice") {
        if voice_name == "?" {
            resolve_voice_name("?")?; // This exits internally
        }
    }
    
    // Handle list speakers
    if matches.get_flag("list-speakers") {
        let socket_path = if let Some(custom_path) = matches.get_one::<String>("socket-path") {
            PathBuf::from(custom_path)
        } else {
            get_socket_path()
        };
        
        if !matches.get_flag("standalone") {
            if let Ok(_) = list_speakers_daemon(&socket_path).await {
                return Ok(());
            }
                }
        
        // Fallback to standalone
        println!("Initializing VOICEVOX Core...");
        let core = VoicevoxCore::new()?;
        
        if matches.get_flag("minimal-models") {
            if let Err(e) = core.load_minimal_models() {
                println!("Warning: Failed to load some minimal models: {}", e);
            }
        } else {
            if let Err(e) = core.load_all_models() {
                println!("Warning: Failed to load some models: {}", e);
            }
        }
        
        println!("All available speakers and styles from loaded models:");
        let speakers = core.get_speakers()?;
        for speaker in &speakers {
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
    
    // Get text input
    let text = get_input_text(&matches)?;
    if text.trim().is_empty() {
        return Err(anyhow!(
            "No text provided. Use command line argument, -f file, or pipe text to stdin."
        ));
    }
    
    // Resolve voice settings
    let (style_id, voice_description) = if let Some(speaker_id) = matches.get_one::<u32>("speaker-id") {
        (*speaker_id, format!("Style ID {}", speaker_id))
    } else {
        let voice_name = matches.get_one::<String>("voice").unwrap();
        resolve_voice_name(voice_name)?
    };
    
    // Get other options
    let rate = *matches.get_one::<f32>("rate").unwrap_or(&1.0);
    let streaming = matches.get_flag("streaming");
    let quiet = matches.get_flag("quiet");
    let output_file = matches.get_one::<String>("output-file");
    let minimal_models = matches.get_flag("minimal-models");
    let force_standalone = matches.get_flag("standalone");
    
    // Validate rate
    if rate < 0.5 || rate > 2.0 {
        return Err(anyhow!("Rate must be between 0.5 and 2.0, got: {}", rate));
    }
    
    let options = SynthesizeOptions { rate, streaming };
    
    
    // Try daemon mode first (unless forced standalone)
    if !force_standalone {
        let socket_path = if let Some(custom_path) = matches.get_one::<String>("socket-path") {
            PathBuf::from(custom_path)
        } else {
            get_socket_path()
        };
        
        match daemon_mode(
            &text,
            style_id,
            &voice_description,
            options.clone(),
            output_file,
            quiet,
            &socket_path,
        )
        .await
        {
            Ok(_) => return Ok(()),
            Err(_) => {
                // Try to start daemon automatically
                        if start_daemon_if_needed().await.is_ok() {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    
                    // Try daemon mode again
                    if let Ok(_) = daemon_mode(
                        &text,
                        style_id,
                        &voice_description,
                        options,
                        output_file,
                        quiet,
                        &socket_path,
                    )
                    .await
                    {
                        return Ok(());
                    }
                }
            }
        }
    }
    
    // Fallback to standalone mode
    standalone_mode(
        &text,
        style_id,
        &voice_description,
        output_file,
        quiet,
        rate,
        streaming,
        minimal_models,
    )
    .await
}