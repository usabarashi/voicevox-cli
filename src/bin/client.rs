//! VOICEVOX CLI client binary - `voicevox-say`
//!
//! Lightweight CLI client that communicates with the daemon via Unix sockets.
//! Provides macOS `say` command-compatible interface for Japanese TTS with
//! various character voices. Handles first-run setup and model downloads.

use anyhow::{anyhow, Result};
use clap::{Arg, Command};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use voicevox_cli::client::daemon_client::start_daemon_with_confirmation;
use voicevox_cli::client::*;
use voicevox_cli::core::VoicevoxCore;
use voicevox_cli::ipc::OwnedSynthesizeOptions;
use voicevox_cli::paths::get_socket_path;
use voicevox_cli::voice::{resolve_voice_dynamic, scan_available_models};

// Resolve voice ID from command line arguments with fallback chain
fn resolve_voice_from_args(matches: &clap::ArgMatches) -> Result<(u32, String)> {
    matches
        .get_one::<u32>("speaker-id")
        .map(|&id| (id, format!("Style ID {}", id)))
        .or_else(|| {
            matches
                .get_one::<u32>("model")
                .map(|&id| (id, format!("Model {} (Default Style)", id)))
        })
        .map(Ok)
        .or_else(|| {
            matches
                .get_one::<String>("voice")
                .map(|voice_name| resolve_voice_dynamic(voice_name))
        })
        .unwrap_or_else(|| Ok((3, "Default (Zundamon Normal)".to_string())))
}

async fn try_daemon_with_retry(
    text: &str,
    style_id: u32,
    voice_description: &str,
    options: OwnedSynthesizeOptions,
    output_file: Option<&String>,
    quiet: bool,
    socket_path: &PathBuf,
) -> Result<()> {
    // First attempt to connect to daemon
    let initial_result = daemon_mode(
        text,
        style_id,
        voice_description,
        options.clone(),
        output_file,
        quiet,
        socket_path,
    )
    .await;

    match initial_result {
        Ok(_) => Ok(()),
        Err(e) => {
            // Check if error is connection-related (daemon not running)
            let is_connection_error = e.to_string().contains("Failed to connect to daemon")
                || e.to_string().contains("Daemon connection timeout");

            if is_connection_error {
                // Check if models exist before starting daemon
                if voicevox_cli::paths::find_models_dir_client().is_err() {
                    // Models not found, this is likely first run
                    if !quiet {
                        println!("🎭 Voice models not found. Setting up VOICEVOX...");
                    }
                    ensure_models_available().await?;
                }

                // Try to start daemon
                start_daemon_with_confirmation().await?;
                tokio::time::sleep(Duration::from_secs(5)).await;

                // Retry daemon mode after starting daemon
                return daemon_mode(
                    text,
                    style_id,
                    voice_description,
                    options,
                    output_file,
                    quiet,
                    socket_path,
                )
                .await;
            }

            // For other errors, just propagate them
            Err(e)
        }
    }
}

async fn standalone_mode(
    text: &str,
    style_id: u32,
    _voice_description: &str,
    output_file: Option<&String>,
    quiet: bool,
    _rate: f32,
    _streaming: bool,
) -> Result<()> {
    // Check for models before initializing core
    if voicevox_cli::paths::find_models_dir_client().is_err() {
        if !quiet {
            println!("🎭 Voice models not found. Setting up VOICEVOX...");
        }
        ensure_models_available().await?;
    }

    let core = VoicevoxCore::new()?;
    core.load_all_models_no_download().map_err(|e| {
        eprintln!("Error: Failed to load models: {}", e);
        e
    })?;

    let wav_data = core.synthesize(text, style_id)?;

    // Handle output
    match output_file {
        Some(file_path) => std::fs::write(file_path, &wav_data)?,
        None if !quiet => play_audio_from_memory(&wav_data).map_err(|e| {
            eprintln!("Error: Audio playback failed: {}", e);
            e
        })?,
        _ => {} // quiet mode with no output file
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let app = Command::new("voicevox-say")
        .version(env!("CARGO_PKG_VERSION"))
        .about("VOICEVOX Say - Convert text to audible speech using VOICEVOX")
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
                .help("Specify voice model by file number (e.g., --model 3 for 3.vvm)")
                .long("model")
                .short('m')
                .value_name("MODEL_ID")
                .value_parser(clap::value_parser!(u32))
                .conflicts_with_all(["voice", "speaker-id"]),
        )
        .arg(
            Arg::new("list-models")
                .help("List all available voice models and exit")
                .long("list-models")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("status")
                .help("Show installation status of voice models and dictionary")
                .long("status")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("standalone")
                .help("Force standalone mode (don't use daemon)")
                .long("standalone")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("socket-path")
                .help("Specify custom Unix socket path")
                .long("socket-path")
                .short('s')
                .value_name("PATH"),
        );

    let matches = app.get_matches();

    if let Some(voice_name) = matches.get_one::<String>("voice") {
        if voice_name == "?" {
            resolve_voice_dynamic("?")?; // This exits internally
        }
    }

    if matches.get_flag("list-models") {
        println!("Scanning for available voice models...");
        let models = scan_available_models().unwrap_or_else(|e| {
            eprintln!("Error scanning models: {}", e);
            std::process::exit(1);
        });

        if models.is_empty() {
            println!("No voice models found. Please start voicevox-daemon to download voice models automatically.");
            return Ok(());
        }

        println!("Available voice models:");
        for model in &models {
            println!("  Model {} ({})", model.model_id, model.file_path.display());
            println!(
                "    Usage: --model {} or --speaker-id <STYLE_ID>",
                model.model_id
            );
        }

        println!("\nTips:");
        println!("  - Use --model N to load model N.vvm");
        println!("  - Use --speaker-id for direct style ID specification");
        println!("  - Use --list-speakers for detailed speaker information");

        return Ok(());
    }

    if matches.get_flag("status") {
        println!("VOICEVOX CLI Installation Status");
        println!("=====================================");

        println!("Application: v{}", env!("CARGO_PKG_VERSION"));

        match scan_available_models() {
            Ok(current_models) => {
                println!("Voice Models: {} files installed", current_models.len());
                for model in &current_models {
                    let model_info = match std::fs::metadata(&model.file_path) {
                        Ok(metadata) => {
                            let size_kb = metadata.len() / 1024;
                            let filename = model
                                .file_path
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy();
                            format!("  Model {}: {} ({} KB)", model.model_id, filename, size_kb)
                        }
                        Err(_) => {
                            format!("  Model {} ({})", model.model_id, model.file_path.display())
                        }
                    };
                    println!("{}", model_info);
                }

                use voicevox_cli::paths::find_openjtalk_dict;
                match find_openjtalk_dict() {
                    Ok(dict_path) => {
                        println!("Dictionary: {} ✅", dict_path);
                    }
                    Err(_) => {
                        println!("Dictionary: Not found ❌");
                        println!("  Install with: voicevox-setup-models");
                    }
                }
            }
            Err(e) => {
                eprintln!("Error scanning models: {}", e);
            }
        }
        return Ok(());
    }

    if matches.get_flag("list-speakers") {
        let socket_path = matches
            .get_one::<String>("socket-path")
            .map(PathBuf::from)
            .unwrap_or_else(get_socket_path);

        if !matches.get_flag("standalone") && list_speakers_daemon(&socket_path).await.is_ok() {
            return Ok(());
        }

        println!("Initializing VOICEVOX Core...");
        let core = VoicevoxCore::new()?;

        if let Err(e) = core.load_all_models_no_download() {
            println!("Warning: Failed to load some models: {}", e);
            eprintln!("Please start voicevox-daemon to download voice models automatically");
        }

        println!("All available speakers and styles from loaded models:");
        let speakers = core.get_speakers()?;

        // Build dynamic style-to-model mapping by scanning loaded models
        println!("Building style-to-model mapping...");
        // For standalone mode, we can't easily determine exact mappings since all models are loaded
        // Just create a simple mapping where style_id maps to itself
        let style_to_model: HashMap<u32, u32> = speakers
            .iter()
            .flat_map(|s| s.styles.iter().map(|style| (style.id, style.id)))
            .collect();

        for speaker in &speakers {
            println!("  {}", speaker.name);
            for style in &speaker.styles {
                let model_id = style_to_model.get(&style.id).copied().unwrap_or(style.id);
                println!(
                    "    {} (Model: {}, Style ID: {})",
                    style.name, model_id, style.id
                );
                if let Some(style_type) = &style.style_type {
                    println!("        Type: {}", style_type);
                }
            }
            println!();
        }
        return Ok(());
    }

    let text = get_input_text(&matches)?;
    if text.trim().is_empty() {
        return Err(anyhow!(
            "No text provided. Use command line argument, -f file, or pipe text to stdin."
        ));
    }

    // Voice resolution: speaker-id → model → voice-name → default
    let (style_id, voice_description) = resolve_voice_from_args(&matches)?;

    let rate = *matches.get_one::<f32>("rate").unwrap_or(&1.0);
    let streaming = matches.get_flag("streaming");
    let quiet = matches.get_flag("quiet");
    let output_file = matches.get_one::<String>("output-file");
    let force_standalone = matches.get_flag("standalone");

    if !(0.5..=2.0).contains(&rate) {
        return Err(anyhow!("Rate must be between 0.5 and 2.0, got: {}", rate));
    }

    let options = OwnedSynthesizeOptions {
        rate,
        streaming,
        context: None,
        zero_copy: false,
    };

    // Try daemon mode first, regardless of model availability
    if !force_standalone {
        let socket_path = matches
            .get_one::<String>("socket-path")
            .map(PathBuf::from)
            .unwrap_or_else(get_socket_path);

        if try_daemon_with_retry(
            &text,
            style_id,
            &voice_description,
            options.clone(),
            output_file,
            quiet,
            &socket_path,
        )
        .await
        .is_ok()
        {
            return Ok(());
        }

        if !quiet {
            println!("🔄 Daemon unavailable, using standalone mode...");
        }
    }

    standalone_mode(
        &text,
        style_id,
        &voice_description,
        output_file,
        quiet,
        rate,
        streaming,
    )
    .await
}
