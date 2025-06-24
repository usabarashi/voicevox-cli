use anyhow::{anyhow, Result};
use clap::{Arg, Command};
use std::path::PathBuf;
use std::time::Duration;

use voicevox_cli::client::*;
use voicevox_cli::core::VoicevoxCore;
use voicevox_cli::ipc::SynthesizeOptions;
use voicevox_cli::paths::get_socket_path;
use voicevox_cli::voice::{resolve_voice_dynamic, scan_available_models};

async fn try_daemon_with_retry(
    text: &str,
    style_id: u32,
    voice_description: &str,
    options: SynthesizeOptions,
    output_file: Option<&String>,
    quiet: bool,
    socket_path: &PathBuf,
) -> Result<()> {
    if daemon_mode(text, style_id, voice_description, options.clone(), output_file, quiet, socket_path).await.is_ok() {
        return Ok(());
    }
    
    start_daemon_if_needed().await?;
    tokio::time::sleep(Duration::from_secs(5)).await;
    daemon_mode(text, style_id, voice_description, options, output_file, quiet, socket_path).await
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
    // Silent operation like macOS say - no output unless error
    
    let core = VoicevoxCore::new()?;
    
    // Load models silently - no download attempt in client
    if minimal_models {
        if let Err(e) = core.load_minimal_models() {
            eprintln!("Error: Failed to load minimal models: {}", e);
            eprintln!("Please start voicevox-daemon to download models automatically");
            return Err(e);
        }
    } else {
        if let Err(e) = core.load_all_models_no_download() {
            eprintln!("Error: Failed to load models: {}", e);
            eprintln!("Please start voicevox-daemon to download models automatically");
            return Err(e);
        }
    }
    
    // Synthesize speech silently
    let wav_data = core.synthesize(text, style_id)?;
    
    // Handle output
    if let Some(output_file) = output_file {
        std::fs::write(output_file, &wav_data)?;
        // Silent for file output (like macOS say -o)
    }
    
    // Play audio if not quiet and no output file (like macOS say command)
    if !quiet && output_file.is_none() {
        if let Err(e) = play_audio_from_memory(&wav_data) {
            eprintln!("Error: Audio playback failed: {}", e);
            return Err(e);
        }
    }
    
    Ok(())
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
                .value_name("VOICE"),
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
                .conflicts_with_all(["voice", "model"]),
        )
        .arg(
            Arg::new("model")
                .help("Specify voice model by VVM file number (e.g., --model 3 for 3.vvm)")
                .long("model")
                .short('m')
                .value_name("MODEL_ID")
                .value_parser(clap::value_parser!(u32))
                .conflicts_with_all(["voice", "speaker-id"]),
        )
        .arg(
            Arg::new("list-models")
                .help("List all available VVM models and exit")
                .long("list-models")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("update-models")
                .help("Update voice models only (skip dictionary)")
                .long("update-models")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("update-dict")
                .help("Update dictionary only (skip voice models)")
                .long("update-dict")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("update-model")
                .help("Update specific voice model by ID")
                .long("update-model")
                .value_name("MODEL_ID")
                .value_parser(clap::value_parser!(u32)),
        )
        .arg(
            Arg::new("check-updates")
                .help("Check for available updates without downloading")
                .long("check-updates")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("version-info")
                .help("Show version information of installed models and dictionary")
                .long("version-info")
                .action(clap::ArgAction::SetTrue),
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
    
    if matches.get_flag("daemon-status") {
        let socket_path = if let Some(custom_path) = matches.get_one::<String>("socket-path") {
            PathBuf::from(custom_path)
        } else {
            get_socket_path()
        };
        return check_daemon_status(&socket_path).await;
    }
    
    if let Some(voice_name) = matches.get_one::<String>("voice") {
        if voice_name == "?" {
            resolve_voice_dynamic("?")?; // This exits internally
        }
    }
    
    if matches.get_flag("list-models") {
        println!("Scanning for available VVM models...");
        match scan_available_models() {
            Ok(models) => {
                if models.is_empty() {
                    println!("No VVM models found. Please download models first with voicevox-daemon.");
                } else {
                    println!("Available VVM models:");
                    for model in &models {
                        println!("  Model {} ({})", model.model_id, model.file_path.display());
                        println!("    Usage: --model {} or --speaker-id <STYLE_ID>", model.model_id);
                    }
                    println!();
                    println!("Tips:");
                    println!("  - Use --model N to load model N.vvm");
                    println!("  - Use --speaker-id for direct style ID specification");
                    println!("  - Use --list-speakers for detailed speaker information");
                }
            }
            Err(e) => {
                eprintln!("Error scanning models: {}", e);
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    
    if matches.get_flag("update-models") {
        println!("🔄 Updating voice models only...");
        println!("Note: This feature requires VOICEVOX downloader with --models-only support");
        println!("For now, falling back to full update...");
        return ensure_models_available().await;
    }
    
    if matches.get_flag("update-dict") {
        println!("🔄 Updating dictionary only...");
        println!("Note: This feature requires VOICEVOX downloader with --dict-only support");
        println!("For now, falling back to full update...");
        return ensure_models_available().await;
    }
    
    if let Some(model_id) = matches.get_one::<u32>("update-model") {
        println!("🔄 Updating model {} only...", model_id);
        println!("Note: This feature requires VOICEVOX downloader with --model support");
        println!("For now, falling back to full update...");
        return ensure_models_available().await;
    }
    
    if matches.get_flag("check-updates") {
        println!("🔍 Checking for available updates...");
        
        // Get current models
        match scan_available_models() {
            Ok(current_models) => {
                println!("📊 Current installation status:");
                println!("  Voice models: {} VVM files", current_models.len());
                for model in &current_models {
                    println!("    Model {} ({})", model.model_id, model.file_path.display());
                }
                
                // Check dictionary
                use voicevox_cli::paths::find_openjtalk_dict;
                match find_openjtalk_dict() {
                    Ok(dict_path) => {
                        println!("  Dictionary: {} ✅", dict_path);
                    }
                    Err(_) => {
                        println!("  Dictionary: Not found ❌");
                    }
                }
                
                println!();
                println!("💡 Update options:");
                println!("  --update-models     Update all voice models");
                println!("  --update-dict       Update dictionary only");
                println!("  --update-model N    Update specific model N");
            }
            Err(e) => {
                eprintln!("Error scanning models: {}", e);
            }
        }
        return Ok(());
    }
    
    if matches.get_flag("version-info") {
        println!("📋 VOICEVOX CLI Version Information");
        println!("=====================================");
        
        // Application version
        println!("Application: v{}", env!("CARGO_PKG_VERSION"));
        
        // Get current models with metadata
        match scan_available_models() {
            Ok(current_models) => {
                println!("Voice Models: {} installed", current_models.len());
                for model in &current_models {
                    if let Ok(metadata) = std::fs::metadata(&model.file_path) {
                        let size_kb = metadata.len() / 1024;
                        println!("  Model {}: {} ({} KB)", 
                                 model.model_id, 
                                 model.file_path.file_name().unwrap_or_default().to_string_lossy(),
                                 size_kb);
                    }
                }
            }
            Err(e) => {
                println!("Voice Models: Error scanning - {}", e);
            }
        }
        
        // Check dictionary
        use voicevox_cli::paths::find_openjtalk_dict;
        match find_openjtalk_dict() {
            Ok(dict_path) => {
                println!("Dictionary: {}", dict_path);
            }
            Err(_) => {
                println!("Dictionary: Not installed");
            }
        }
        
        return Ok(());
    }
    
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
                println!("Please start voicevox-daemon to download models automatically");
            }
        } else {
            if let Err(e) = core.load_all_models_no_download() {
                println!("Warning: Failed to load some models: {}", e);
                println!("Please start voicevox-daemon to download models automatically");
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
    } else if let Some(model_id) = matches.get_one::<u32>("model") {
        // For now, use the first style from the model (style_id = model_id * 10 as a heuristic)
        // In the future, this should load the model and get the actual first style ID
        (*model_id, format!("Model {} (Default Style)", model_id))
    } else if let Some(voice_name) = matches.get_one::<String>("voice") {
        resolve_voice_dynamic(voice_name)?
    } else {
        // No voice specified - default to speaker ID 3 (ずんだもん ノーマル)
        (3, "Default (ずんだもん ノーマル)".to_string())
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
    
    // Check for models and download if needed (client-side first-run setup)
    if !force_standalone {
        if let Err(_) = ensure_models_available().await {
            // User declined download or download failed, fall back to standalone
            if !quiet {
                println!("Falling back to standalone mode...");
            }
        }
    }
    
    // Try daemon mode first (unless forced standalone)
    if !force_standalone {
        let socket_path = if let Some(custom_path) = matches.get_one::<String>("socket-path") {
            PathBuf::from(custom_path)
        } else {
            get_socket_path()
        };
        
        // Try daemon mode with automatic retry
        if let Ok(_) = try_daemon_with_retry(&text, style_id, &voice_description, options.clone(), output_file, quiet, &socket_path).await {
            return Ok(());
        }
        
        // Daemon failed, log message if not quiet
        if !quiet {
            println!("🔄 Daemon unavailable, using standalone mode...");
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