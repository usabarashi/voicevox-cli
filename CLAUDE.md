# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is VOICEVOX CLI (`voicevox-cli`) - a command-line text-to-speech synthesis tool using VOICEVOX Core 0.16.0. It provides a macOS `say` command-compatible interface for Japanese TTS with various character voices like ずんだもん (Zundamon), 四国めたん (Shikoku Metan), etc.

The tool uses a **daemon-client architecture** for performance, with pre-loaded voice models in a background daemon process for instant synthesis. It's specifically designed for macOS with CPU-only processing and maintains complete compatibility with macOS `say` command behavior (silent operation on success, errors to stderr only).

**Key Features:**
- **Dynamic Voice Detection**: Zero hardcoded voice mappings - adapts to available models
- **Rust Patterns**: GATs, const generics, zero-copy serialization, and type-level programming
- **Performance Architecture**: CompactString, SmallVec, rayon parallelization, and SIMD optimizations
- **Functional Programming Design**: Immutable data structures, monadic composition, and declarative processing
- **macOS Integration**: Complete compatibility with macOS `say` command interface

## Architecture

### System Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                    VOICEVOX CLI Architecture                    │
└─────────────────────────────────────────────────────────────────┘

┌───────────────────┐    Unix Socket    ┌─────────────────────────┐
│   voicevox-say    │◄─────────────────►│    voicevox-daemon      │
│   (CLI Client)    │     IPC/Tokio     │   (Background Service)  │
├───────────────────┤                   ├─────────────────────────┤
│ • User Interface  │                   │ • Model Loading         │
│ • Argument Parse  │                   │ • Voice Synthesis       │
│ • First-run Setup │                   │ • Audio Generation      │
│ • Model Download  │                   │ • Socket Server         │
└───────────────────┘                   └─────────────────────────┘
         │                                         │
         │                                         │
         ▼                                         ▼
┌───────────────────┐                   ┌─────────────────────────┐
│  Static Libraries │                   │   Voice Models (VVM)    │
│  (Build-time)     │                   │   (Runtime Download)    │
├───────────────────┤                   ├─────────────────────────┤
│ ✓ VOICEVOX Core   │                   │ • 26+ Characters        │
│ ✓ ONNX Runtime    │                   │ • Zundamon, Metan, etc. │
│ ✓ OpenJTalk Dict  │                   │ • ~/.local/share/...    │
│ ✓ Rust API        │                   │ • User-specific         │
└───────────────────┘                   └─────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│                        File Structure                           │
├─────────────────────────────────────────────────────────────────┤
│ src/                                                            │
│ ├── lib.rs              # Shared library & IPC protocols        │
│ ├── bin/                # Binary crates                         │
│ │   ├── daemon.rs       # Background daemon process             │
│ │   └── client.rs       # CLI client (primary interface)        │
│ │                                                               │
│ ├── core.rs             # VOICEVOX Core wrapper (single file)   │
│ ├── voice.rs            # Dynamic voice detection (single file) │
│ ├── paths.rs            # XDG-compliant path discovery          │
│ ├── setup.rs            # First-run setup utilities             │
│ ├── ipc.rs              # Inter-process communication           │
│ ├── config.rs           # Configuration file support            │
│ ├── memory_pool.rs      # Memory pool for buffer reuse          │
│ │                                                               │
│ ├── client/             # Client-side functionality (multi)     │
│ │   ├── mod.rs          # Module exports                        │
│ │   ├── download.rs     # Model download management             │
│ │   ├── daemon_client.rs# Daemon communication                  │
│ │   ├── audio.rs        # Audio playback                        │
│ │   ├── input.rs        # Input handling                        │
│ │   └── fd_receive.rs   # Zero-copy file descriptor reception   │
│ │                                                               │
│ └── daemon/             # Server-side functionality (multi)     │
│     ├── mod.rs          # Module exports                        │
│     ├── server.rs       # Background server implementation      │
│     ├── process.rs      # Process management                    │
│     ├── fd_passing.rs   # Zero-copy file descriptor passing     │
│     └── fd_server.rs    # FD-enabled server implementation      │
│                                                                 │
│ Static Resources (Build-time):                                  │
│ ├── voicevox_core/      # Statically linked libraries           │
│ └── flake.nix           # Nix build configuration               │
│                                                                 │
│ Runtime Resources (User directory):                             │
│ └── ~/.local/share/voicevox/models/  # Voice model files        │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│                    Process Flow Diagram                         │
└─────────────────────────────────────────────────────────────────┘

User Command: voicevox-say "Hello"
         │
         ▼
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   Client Start  │────►│  Check Daemon   │────►│  Send Request   │
└─────────────────┘     └─────────────────┘     └─────────────────┘
         │                       │                       │
         │                       ▼                       │
         │              ┌─────────────────┐              │
         │              │  Start Daemon   │              │
         │              │  (if needed)    │              │
         │              └─────────────────┘              │
         │                       │                       │
         │                       ▼                       │
         │              ┌─────────────────┐              │
         │              │  Load 3 Models  │              │
         │              │  (Lazy Loading) │              │
         │              └─────────────────┘              │
         │                       │                       │
         └───────────────────────┼───────────────────────┘
                                 ▼
                    ┌─────────────────┐
                    │ Voice Synthesis │
                    │ (VOICEVOX Core) │
                    └─────────────────┘
                                 │
                                 ▼
                    ┌─────────────────┐     ┌─────────────────┐
                    │ Audio Output    │────►│ Client Response │
                    │ (WAV/Speaker)   │     │ (Silent/Error)  │
                    └─────────────────┘     └─────────────────┘
```


### Core Components

**Library & Binaries**:
- **`src/lib.rs`**: Shared library with VoicevoxCore and IPC protocols
- **`src/bin/daemon.rs`**: Background daemon process with model management
- **`src/bin/client.rs`**: Lightweight CLI client (primary interface) with functional voice resolution

**Single-File Modules** (Rust 2018+ Pattern):
- **`src/core.rs`**: VOICEVOX Core wrapper with functional programming patterns
- **`src/voice.rs`**: Dynamic voice detection and resolution system
- **`src/paths.rs`**: Functional path discovery and XDG compliance
- **`src/setup.rs`**: First-run setup and model management utilities
- **`src/ipc.rs`**: Inter-process communication protocols and data structures

**Multi-File Modules**:
- **`src/client/`**: Client-side functionality (daemon client, download management)
  - `download.rs`: Model download with interactive license acceptance
  - `daemon_client.rs`: Unix socket communication with daemon
  - `audio.rs`: Audio playback and WAV file handling
  - `input.rs`: stdin and argument processing
- **`src/daemon/`**: Server-side functionality (model loading, synthesis)
  - `server.rs`: Background server implementation with async IPC
  - `process.rs`: Process management and duplicate prevention


### Static Linking Architecture

**Components**:
- **VOICEVOX Core**: Statically linked `libvoicevox_core.dylib`
- **ONNX Runtime**: Statically linked `libvoicevox_onnxruntime.dylib`
- **OpenJTalk Dictionary**: Build-time embedded via `env!()` macro
- **Package**: ~54MB total with 26+ voice models available for download

## Build Reproducibility

**IMPORTANT**: Local builds and GitHub Actions builds MUST produce identical release archives with matching SHA256 hashes. This ensures binary reproducibility across build environments.

**Implementation**: Both local and CI builds use the same `tar` command from Nix development shell to ensure consistent archive creation.

**Installation Default**: Users install pre-built binaries from GitHub Releases by default. Source builds are available but not required for typical usage.

**Hash Calculation for Release**:
```bash
# 1. Build reproducible archive
nix build .#voicevox-cli-archive

# 2. Calculate hash
nix hash file result  # → sha256-XXX...

# 3. Update outputHash in voicevox-cli-archive-verified in flake.nix
# 4. Verify reproducibility locally
nix build .#voicevox-cli-archive-verified  # Will fail if hash doesn't match

# 5. Commit and push
# 6. GitHub Actions creates identical archive
```

**Reproducibility Verification**:
The `voicevox-cli-archive-verified` derivation ensures build reproducibility:
- Uses fixed-output derivation with expected hash
- Build fails if produced archive doesn't match expected hash
- Guarantees identical archives between local and CI builds

## Build Commands

### Nix (Static Linking)
```bash
# Build the project (~54MB package)
nix build

# Run daemon directly
nix run .#voicevox-daemon

# Run client directly
nix run .#voicevox-say -- "テストメッセージ"

# Development shell
nix develop

# Check package size and contents
du -sh result/
ls -la result/bin/

# Test functionality after build
./result/bin/voicevox-say "静的リンクテストなのだ"
./result/bin/voicevox-say --list-speakers
```


### Cargo (Production Ready)
```bash
# Build all binaries (daemon + client)
export DYLD_LIBRARY_PATH=./voicevox_core/c_api/lib:./voicevox_core/onnxruntime/lib
cargo build --release

# Build specific binaries
cargo build --release --bin voicevox-daemon   # Background service
cargo build --release --bin voicevox-say      # Primary CLI (client)

# Development build
cargo build --bin voicevox-daemon --bin voicevox-say

# Performance features
cargo build --release --features performance  # All optimizations combined
cargo build --release --features "performance,parallel,zero_copy"  # Custom profile
```

## Production Usage

### Daemon Management
```bash
# Start daemon (with lazy loading - only 3 models initially)
voicevox-daemon --start

# Stop daemon
voicevox-daemon --stop

# Check daemon status
voicevox-daemon --status

# Restart daemon (stop then start)
voicevox-daemon --restart

# Development mode (foreground with output)
voicevox-daemon --foreground

# Run as detached background process
voicevox-daemon --detach

# Custom socket path
voicevox-daemon --socket-path /custom/path/daemon.sock --start
```


### Client Usage (macOS say Compatible)
```bash
# Basic usage (completely silent like macOS say)
./target/release/voicevox-say "こんにちはなのだ"

# Save to file (silent)
./target/release/voicevox-say "テスト" -o output.wav

# Different voices
./target/release/voicevox-say --speaker-id 3 "ずんだもんの声なのだ"
./target/release/voicevox-say --speaker-id 2 "四国めたんの声です"

# Voice selection by model
./target/release/voicevox-say --model 3 "モデル3の音声なのだ"

# Status and information (only commands that produce output)
./target/release/voicevox-say --list-speakers
./target/release/voicevox-say --list-models
./target/release/voicevox-say --status

# Force standalone mode
./target/release/voicevox-say --standalone "テストメッセージ"

# Read from stdin
echo "標準入力からのテキスト" | ./target/release/voicevox-say
```

## Current Implementation

### Memory Management
- **Lazy Loading**: Starts with only 3 models (Metan 0, Zundamon 1, Tsumugi 8)
- **LRU Cache**: Maximum 5 models in memory, automatically unloads least-used models
- **Favorites Protection**: Models 0, 1, 8 are never unloaded
- **Real Memory Release**: Uses VOICEVOX Core's `unload_voice_model` API

### First-Run Experience
- **Automatic Setup**: Downloads models on first use if not found
- **Seamless Integration**: Model download happens within normal command flow
- **No Extra Commands**: Users don't need to run separate setup commands

### Configuration Support
- **Config File**: `~/.config/voicevox/config.toml` for persistent settings
- **CLI Override**: Command-line options override config file
- **Customizable**: Memory limits, preload models, favorites list

### Zero-Copy Memory Transfer
- **File Descriptor Passing**: Uses Unix domain socket SCM_RIGHTS for zero-copy audio transfer
- **Memory-Mapped Files**: Audio data shared via anonymous memory files (memfd_create/tempfile)
- **Protocol Negotiation**: Automatic fallback to regular transfer if zero-copy unavailable
- **Stream Reuse Pattern**: Works around Tokio's ownership constraints for FD passing

### Configuration Example
```toml
# Memory Management
[memory]
max_loaded_models = 5          # Maximum models in memory
enable_lru_cache = true        # Enable automatic unloading
memory_limit_mb = 1024         # Informational only

# Model Preferences
[models]
preload = [0, 1, 8]            # Models to load on startup
favorites = [0, 1, 8]          # Never unload these models
predictive_preload = false     # Experimental feature

# Daemon Settings
[daemon]
socket_path = "/custom/path"   # Optional custom socket
startup_timeout = 10           # Seconds to wait
debug = false                  # Enable debug logging
```

### CLI Configuration Options
```bash
# Create example configuration
voicevox-daemon --create-config

# Use custom config file
voicevox-daemon --config /path/to/config.toml

# Override specific settings
voicevox-daemon --max-models 10 --no-lru
```

## Testing & Development

### Quick Test Procedure (Recommended)

```bash
# Use Nix build for reliable testing (statically linked)
nix build

# 1. Kill any existing daemon
pkill -f voicevox-daemon || true

# 2. Start daemon and check memory
./result/bin/voicevox-daemon --start --detach
ps aux | grep voicevox-daemon | grep -v grep | awk '{print "Memory (MB): " $6/1024}'

# 3. Test synthesis
./result/bin/voicevox-say "テストなのだ"

# 4. Check daemon status
./result/bin/voicevox-daemon --status

# 5. Stop daemon
./result/bin/voicevox-daemon --stop
```

### Development Testing (Cargo)

```bash
# For Cargo builds, library path issues are common on macOS
# Recommendation: Use Nix build for testing to avoid dylib issues

# If you must use Cargo:
cargo build --release
# Copy libraries to target directory (macOS workaround)
cp target/release/deps/*.dylib target/release/ 2>/dev/null || true

# Then test from release directory
cd target/release
./voicevox-daemon --foreground &
./voicevox-say "テスト"
pkill -f voicevox-daemon
```


### CI Task Runner (Local)

Run the complete CI pipeline locally using Nix:

```bash
# Run all CI checks (matches GitHub Actions)
nix run .#ci

# Individual development commands
nix develop --command cargo fmt        # Format code
nix develop --command cargo clippy     # Static analysis
nix develop --command cargo audit      # Security audit
nix build                              # Build project
```

### GitHub Actions CI

**Pipeline**: Static analysis, Nix build, package verification, security audit
**Features**: SHA-pinned actions, matrix strategy (Nix primary, Cargo fallback), efficient caching
