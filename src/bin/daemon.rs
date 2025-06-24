use anyhow::Result;
use clap::{Arg, Command};
use std::path::PathBuf;

use voicevox_cli::daemon::{check_and_prevent_duplicate, run_daemon};
use voicevox_cli::paths::get_socket_path;

#[tokio::main]
async fn main() -> Result<()> {
    let app = Command::new("voicevox-daemon")
        .version(env!("CARGO_PKG_VERSION"))
        .about("VOICEVOX Daemon - Background TTS service with pre-loaded models")
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
            Arg::new("detach")
                .help("Run as detached background process")
                .long("detach")
                .short('d')
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
        )
        .arg(
            Arg::new("system-mode")
                .help("Run as system-wide daemon (multi-user mode)")
                .long("system-mode")
                .action(clap::ArgAction::SetTrue),
        );
    
    let matches = app.get_matches();
    
    // Override environment variables if provided via CLI
    if let Some(models_dir) = matches.get_one::<String>("models-dir") {
        std::env::set_var("VOICEVOX_MODELS_DIR", models_dir);
    }
    if let Some(dict_dir) = matches.get_one::<String>("dict-dir") {
        std::env::set_var("VOICEVOX_DICT_DIR", dict_dir);
    }
    
    let system_mode = matches.get_flag("system-mode");
    
    // Determine socket path
    let socket_path = if let Some(custom_path) = matches.get_one::<String>("socket-path") {
        PathBuf::from(custom_path)
    } else if system_mode {
        // System-wide socket path for multi-user access
        PathBuf::from("/var/run/voicevox/daemon.sock")
    } else {
        get_socket_path()
    };
    
    let foreground = matches.get_flag("foreground");
    let detach = matches.get_flag("detach");
    
    // Handle detach mode - fork process and exit parent
    if detach && !foreground {
        use std::os::unix::process::CommandExt;
        use std::process::{Command, Stdio};
        
        println!("Starting daemon in detached mode...");
        
        // Prepare args for child process (without --detach)
        let mut args: Vec<String> = std::env::args().collect();
        args.retain(|arg| arg != "--detach" && arg != "-d");
        args.push("--foreground".to_string()); // Child runs in foreground
        
        // Spawn detached child process
        let child = Command::new(&args[0])
            .args(&args[1..])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0) // Create new process group
            .spawn();
            
        match child {
            Ok(mut child) => {
                // Give child time to start
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                
                // Check if child is still running
                match child.try_wait() {
                    Ok(None) => {
                        println!("✅ VOICEVOX daemon started successfully in background");
                        println!("   Socket: {}", socket_path.display());
                        std::process::exit(0);
                    }
                    Ok(Some(status)) => {
                        eprintln!("❌ Daemon failed to start: exit code {}", status);
                        std::process::exit(1);
                    }
                    Err(e) => {
                        eprintln!("❌ Failed to check daemon status: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            Err(e) => {
                eprintln!("❌ Failed to spawn daemon process: {}", e);
                std::process::exit(1);
            }
        }
    }
    
    // Check for existing daemon process
    if let Err(e) = check_and_prevent_duplicate(&socket_path).await {
        eprintln!("{}", e);
        std::process::exit(1);
    }
    
    // Display startup banner
    println!("VOICEVOX Daemon v{}", env!("CARGO_PKG_VERSION"));
    if system_mode {
        println!("Starting system-wide daemon (multi-user mode)...");
        println!("Socket: {} (system-wide)", socket_path.display());
        println!("Mode: All models (shared across users)");
    } else {
        println!("Starting user daemon...");
        println!("Socket: {} (user-specific)", socket_path.display());
        println!("Mode: All models (user-specific)");
    }
    
    run_daemon(socket_path, foreground).await
}