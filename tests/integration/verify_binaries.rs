use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

/// Get MD5 hash of a file (platform-independent)
fn get_md5_hash(path: &Path) -> Result<String> {
    let contents = fs::read(path)
        .with_context(|| format!("Failed to read file: {:?}", path))?;

    let digest = md5::compute(&contents);

    Ok(format!("{:x}", digest))
}

/// Check if daemon is running and return its path
fn get_running_daemon_path() -> Option<String> {
    let output = Command::new("pgrep")
        .arg("-fl")
        .arg("voicevox-daemon")
        .output()
        .ok()?;

    if output.status.success() && !output.stdout.is_empty() {
        let stdout = String::from_utf8(output.stdout).ok()?;
        let path = stdout
            .lines()
            .next()?
            .split_whitespace()
            .nth(1)?
            .to_string();
        Some(path)
    } else {
        None
    }
}

/// Find system binary path using which
fn find_system_binary(name: &str) -> Option<String> {
    let output = Command::new("which").arg(name).output().ok()?;

    if output.status.success() && !output.stdout.is_empty() {
        Some(String::from_utf8(output.stdout).ok()?.trim().to_string())
    } else {
        None
    }
}

/// Check if string exists in binary using strings command
fn binary_contains_string(binary_path: &Path, search: &str) -> Result<bool> {
    let output = Command::new("strings")
        .arg(binary_path)
        .output()
        .context("Failed to run strings command")?;

    if !output.status.success() {
        anyhow::bail!("strings command failed");
    }

    let stdout = String::from_utf8(output.stdout)?;
    Ok(stdout.lines().any(|line| line.contains(search)))
}

#[test]
fn test_check_running_daemon() -> Result<()> {
    println!("\n=== Checking Running Daemon ===");

    match get_running_daemon_path() {
        Some(path) => {
            println!("✓ Daemon running at: {}", path);

            if path.contains("target/debug") {
                println!("✓ Running development build (OK for tests)");
            } else {
                println!("⚠️  WARNING: System daemon running (not development build)");
                println!("  Consider: kill $(pgrep voicevox-daemon)");
                println!("  Then: ./target/debug/voicevox-daemon --start --detach");
            }
        }
        None => {
            println!("⚠️  No daemon running (ignored tests will be skipped)");
            println!("  Start with: ./target/debug/voicevox-daemon --start --detach");
        }
    }

    Ok(())
}

#[test]
fn test_binaries_exist() -> Result<()> {
    println!("\n=== Checking Built Binaries ===");

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let binaries = ["voicevox-daemon", "voicevox-say", "voicevox-mcp-server"];

    for binary in binaries {
        let path = Path::new(manifest_dir)
            .join("target/debug")
            .join(binary);

        if path.exists() {
            let metadata = fs::metadata(&path)?;
            let modified = metadata.modified()?;
            println!("✓ {} exists (modified: {:?})", binary, modified);
        } else {
            anyhow::bail!("❌ {} not found. Run: nix develop -c cargo build", binary);
        }
    }

    Ok(())
}

#[test]
fn test_compare_with_system_binaries() -> Result<()> {
    println!("\n=== Comparing with System Binaries ===");

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let binaries = ["voicevox-daemon", "voicevox-mcp-server"];

    for binary in binaries {
        let local_path = Path::new(manifest_dir)
            .join("target/debug")
            .join(binary);

        if !local_path.exists() {
            println!("⚠️  {} not built yet", binary);
            continue;
        }

        match find_system_binary(binary) {
            Some(system_path) => {
                let local_hash = get_md5_hash(&local_path)?;
                let system_hash = get_md5_hash(Path::new(&system_path))?;

                if local_hash == system_hash {
                    println!(
                        "⚠️  {}: SAME as system version (may need rebuild)",
                        binary
                    );
                    println!("    Local:  {}", local_path.display());
                    println!("    System: {}", system_path);
                } else {
                    println!("✓ {}: Different from system version", binary);
                }
            }
            None => {
                println!("✓ {}: No system version found", binary);
            }
        }
    }

    Ok(())
}

#[test]
fn test_mcp_protocol_version() -> Result<()> {
    println!("\n=== Checking MCP Protocol Version ===");

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let mcp_server_path = Path::new(manifest_dir)
        .join("target/debug")
        .join("voicevox-mcp-server");

    if !mcp_server_path.exists() {
        anyhow::bail!("❌ MCP server not built. Run: nix develop -c cargo build");
    }

    if binary_contains_string(&mcp_server_path, "2024-11-05")? {
        println!("✓ MCP server uses protocol 2024-11-05 (rmcp implementation)");
    } else {
        anyhow::bail!("❌ MCP server protocol version not found or incorrect");
    }

    Ok(())
}

#[test]
fn test_print_summary() {
    println!("\n=== Binary Verification Summary ===");
    println!("✅ Run integration tests with:");
    println!("   cargo test --test mcp_protocol");
    println!("   cargo test --test synthesis_modes --ignored  # requires daemon");
}
