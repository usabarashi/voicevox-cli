//! VOICEVOX CLI daemon binary - `voicevox-daemon`
//!
//! Background service that pre-loads voice models and handles synthesis requests
//! via Unix socket IPC. Provides instant response times after initial model loading.
//! Supports graceful shutdown and duplicate process prevention.

use anyhow::Result;
use clap::{Arg, Command};
use std::path::PathBuf;

use tokio::net::UnixStream;
use voicevox_cli::daemon::{check_and_prevent_duplicate, run_daemon_with_config};
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
            Arg::new("start")
                .help("Start the daemon (default behavior)")
                .long("start")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("stop")
                .help("Stop the running daemon")
                .long("stop")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("status")
                .help("Check daemon status")
                .long("status")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("restart")
                .help("Restart the daemon (stop then start)")
                .long("restart")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("config")
                .help("Path to configuration file")
                .long("config")
                .short('c')
                .value_name("FILE"),
        )
        .arg(
            Arg::new("max-models")
                .help("Override maximum number of loaded models")
                .long("max-models")
                .value_name("NUM"),
        )
        .arg(
            Arg::new("no-lru")
                .help("Disable LRU cache management")
                .long("no-lru")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("create-config")
                .help("Create example configuration file")
                .long("create-config")
                .action(clap::ArgAction::SetTrue),
        );

    let matches = app.get_matches();

    // Determine socket path
    let socket_path = if let Some(custom_path) = matches.get_one::<String>("socket-path") {
        PathBuf::from(custom_path)
    } else {
        get_socket_path()
    };

    let foreground = matches.get_flag("foreground");
    let detach = matches.get_flag("detach");
    let start = matches.get_flag("start");
    let stop = matches.get_flag("stop");
    let status = matches.get_flag("status");
    let restart = matches.get_flag("restart");
    let create_config = matches.get_flag("create-config");
    
    // Handle create-config
    if create_config {
        return voicevox_cli::config::Config::create_example()
            .map_err(|e| anyhow::anyhow!("Failed to create example config: {}", e));
    }

    // Handle daemon operations
    if stop {
        return handle_stop_daemon(&socket_path).await;
    }

    if status {
        return handle_status_daemon(&socket_path).await;
    }

    if restart {
        println!("🔄 Restarting daemon...");
        let _ = handle_stop_daemon(&socket_path).await; // Ignore errors if not running
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
        // Continue to start logic below
    }

    // Default behavior is start (if no operation specified or explicit --start)
    if !start && !restart {
        // If no operation flags are specified, show help for daemon operations
        if !foreground && !detach {
            println!("VOICEVOX Daemon v{}", env!("CARGO_PKG_VERSION"));
            println!("\nDaemon Operations:");
            println!("  --start     Start the daemon (default)");
            println!("  --stop      Stop the running daemon");
            println!("  --status    Check daemon status");
            println!("  --restart   Restart the daemon");
            println!("\nExecution Modes:");
            println!("  --foreground Run in foreground (for development)");
            println!("  --detach     Run as background process");
            println!("\nUse --help for all options");
            return Ok(());
        }
    }

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

    // Load configuration with CLI overrides
    let mut config = if let Some(config_path) = matches.get_one::<String>("config") {
        match std::fs::read_to_string(config_path) {
            Ok(content) => toml::from_str(&content)
                .unwrap_or_else(|e| {
                    eprintln!("Failed to parse config file: {}", e);
                    voicevox_cli::config::Config::default()
                }),
            Err(e) => {
                eprintln!("Failed to read config file: {}", e);
                voicevox_cli::config::Config::default()
            }
        }
    } else {
        voicevox_cli::config::Config::load()
            .unwrap_or_else(|e| {
                eprintln!("Failed to load config, using defaults: {}", e);
                voicevox_cli::config::Config::default()
            })
    };
    
    // Apply CLI overrides
    if let Some(max_models) = matches.get_one::<String>("max-models") {
        if let Ok(num) = max_models.parse::<usize>() {
            config.memory.max_loaded_models = num;
        }
    }
    
    if matches.get_flag("no-lru") {
        config.memory.enable_lru_cache = false;
    }

    // Display startup banner
    println!("VOICEVOX Daemon v{}", env!("CARGO_PKG_VERSION"));
    println!("Starting user daemon...");
    println!("Socket: {} (user-specific)", socket_path.display());
    println!("Mode: {} models max, LRU: {}", 
             config.memory.max_loaded_models,
             if config.memory.enable_lru_cache { "enabled" } else { "disabled" });

    run_daemon_with_config(socket_path, foreground, config).await
}

/// Handle daemon stop operation
async fn handle_stop_daemon(socket_path: &PathBuf) -> Result<()> {
    println!("🛑 Stopping VOICEVOX daemon...");

    // Check if daemon is running
    match UnixStream::connect(socket_path).await {
        Ok(_) => {
            // Daemon is running, find and stop it
            let output = std::process::Command::new("pgrep")
                .args([
                    "-f",
                    "-u",
                    &unsafe { libc::getuid() }.to_string(),
                    "voicevox-daemon",
                ])
                .output();

            match output {
                Ok(output) if output.status.success() => {
                    let pids = String::from_utf8_lossy(&output.stdout);
                    let pids: Vec<&str> = pids.trim().lines().collect();

                    if pids.is_empty() {
                        println!("❌ No daemon process found");
                        return Ok(());
                    }

                    for pid in pids {
                        if let Ok(pid_num) = pid.parse::<u32>() {
                            let kill_result = std::process::Command::new("kill")
                                .arg("-TERM")
                                .arg(pid)
                                .status();

                            match kill_result {
                                Ok(status) if status.success() => {
                                    println!("✅ Daemon stopped (PID: {})", pid_num);

                                    // Wait a moment then verify
                                    tokio::time::sleep(tokio::time::Duration::from_millis(1000))
                                        .await;

                                    match UnixStream::connect(socket_path).await {
                                        Err(_) => println!("✅ Socket cleanup confirmed"),
                                        Ok(_) => println!("⚠️  Daemon may still be running"),
                                    }
                                }
                                _ => {
                                    println!("❌ Failed to stop daemon (PID: {})", pid_num);
                                    println!("   Try: kill -9 {}", pid_num);
                                }
                            }
                        }
                    }
                }
                _ => {
                    println!("❌ Failed to find daemon process");
                    println!("   Try manual: pkill -f -u $(id -u) voicevox-daemon");
                }
            }
        }
        Err(_) => {
            println!("❌ Daemon is not running");
            println!("   Socket: {}", socket_path.display());
        }
    }

    Ok(())
}

/// Handle daemon status check
async fn handle_status_daemon(socket_path: &PathBuf) -> Result<()> {
    println!("📊 VOICEVOX Daemon Status");
    println!("========================");

    // Check socket connectivity
    match UnixStream::connect(socket_path).await {
        Ok(_) => {
            println!("Status: ✅ Running and responsive");
            println!("Socket: {}", socket_path.display());

            // Additional process information
            let output = std::process::Command::new("pgrep")
                .args([
                    "-f",
                    "-u",
                    &unsafe { libc::getuid() }.to_string(),
                    "voicevox-daemon",
                ])
                .output();

            if let Ok(output) = output {
                if output.status.success() {
                    let pids = String::from_utf8_lossy(&output.stdout);
                    let pids: Vec<&str> = pids.trim().lines().collect();

                    for pid in pids {
                        if let Ok(pid_num) = pid.parse::<u32>() {
                            println!("Process ID: {}", pid_num);

                            // Get memory usage if possible
                            let ps_output = std::process::Command::new("ps")
                                .args(["-p", pid, "-o", "rss,pmem,time"])
                                .output();

                            if let Ok(ps_output) = ps_output {
                                if ps_output.status.success() {
                                    let info = String::from_utf8_lossy(&ps_output.stdout);
                                    let lines: Vec<&str> = info.lines().collect();
                                    if lines.len() > 1 {
                                        println!("Memory Info: {}", lines[1].trim());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Err(_) => {
            println!("Status: ❌ Not running");
            println!("Socket: {}", socket_path.display());
        }
    }

    Ok(())
}
