# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

VOICEVOX CLI (`voicevox-cli`) - a command-line text-to-speech tool using VOICEVOX Core 0.16.0. Provides macOS `say` command-compatible interface for Japanese TTS with character voices like ずんだもん (Zundamon), 四国めたん (Shikoku Metan), etc.

**Key Features:**
- **Daemon-Client Architecture**: Background daemon with pre-loaded models for instant synthesis
- **Dynamic Voice Detection**: No hardcoded mappings - discovers voices from available VVM files
- **macOS Integration**: Silent operation on success, errors to stderr only
- **CPU-Only Processing**: Optimized for macOS without GPU dependencies

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

**Binaries**:
- `voicevox-daemon`: Background service with model management
- `voicevox-say`: CLI client (macOS `say` compatible)

**Key Modules**:
- `core.rs`: VOICEVOX Core wrapper
- `voice.rs`: Dynamic voice detection and style-to-model mapping
- `ipc.rs`: Unix socket communication protocol
- `server.rs`: Daemon implementation with LRU cache
- `daemon_client.rs`: Client-side daemon communication


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


### Cargo
```bash
# Build all binaries
export DYLD_LIBRARY_PATH=./voicevox_core/c_api/lib:./voicevox_core/onnxruntime/lib
cargo build --release

# Development build
cargo build
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


### Client Usage
```bash
# Basic usage (silent like macOS say)
voicevox-say "こんにちはなのだ"

# Save to file
voicevox-say "テスト" -o output.wav

# Different voices
voicevox-say --speaker-id 3 "ずんだもんの声なのだ"
voicevox-say --model 1 "モデル1の音声"

# List available speakers
voicevox-say --list-speakers

# Read from stdin
echo "テキスト" | voicevox-say
```

## Current Implementation

### Voice Model Management
- **Dynamic Voice Detection**: No hardcoded voice mappings - automatically discovers style-to-model relationships at startup
- **Lazy Loading**: Starts with only 3 models (Metan 0, Zundamon 1, Tsumugi 8) for fast startup
- **On-Demand Loading**: Automatically loads required models when specific voices are requested
- **LRU Cache**: Maximum 5 models in memory, automatically unloads least-used models
- **Favorites Protection**: Models 0, 1, 8 are never unloaded (configurable)
- **Real Memory Release**: Uses VOICEVOX Core's `unload_voice_model` API for actual memory recovery

### Voice Discovery System
- **Complete Speaker List**: Daemon shows all 99 available styles in `--list-speakers`, regardless of loaded models
- **Style-to-Model Mapping**: Built dynamically at startup by scanning all VVM files
- **Zero Hardcoding**: Voice IDs are discovered from model files, not hardcoded in source

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



### CI Commands

```bash
# Run all CI checks
nix run .#ci

# Individual checks
nix develop --command cargo fmt        # Format code
nix develop --command cargo clippy     # Static analysis
nix develop --command cargo audit      # Security audit
```
