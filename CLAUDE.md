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
- **Static Linking Priority**: VOICEVOX Core, ONNX Runtime, and OpenJTalk statically linked for minimal dependencies

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
│ │                                                               │
│ ├── client/             # Client-side functionality (multi)     │
│ │   ├── mod.rs          # Module exports                        │
│ │   ├── download.rs     # Model download management             │
│ │   ├── daemon_client.rs# Daemon communication                  │
│ │   ├── audio.rs        # Audio playback                        │
│ │   └── input.rs        # Input handling                        │
│ │                                                               │
│ └── daemon/             # Server-side functionality (multi)     │
│     ├── mod.rs          # Module exports                        │
│     ├── server.rs       # Background server implementation      │
│     └── process.rs      # Process management                    │
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

### Daemon-Client Architecture

**Production Architecture**: The system now uses a high-performance daemon-client model instead of standalone execution:

- **`voicevox-daemon`**: Background process with pre-loaded VOICEVOX models
- **`voicevox-say`**: Lightweight client that communicates with daemon via Unix sockets

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

**External Resources**:
- **`voicevox_core/`**: VOICEVOX Core runtime libraries (`libvoicevox_core.dylib`) and headers
- **`models/*.vvm`**: VOICEVOX voice model files (26+ models supported)
- **`dict/`**: OpenJTalk dictionary for Japanese text processing

### Static Linking Architecture

**Components**:
- **VOICEVOX Core**: Statically linked `libvoicevox_core.dylib`
- **ONNX Runtime**: Statically linked `libvoicevox_onnxruntime.dylib`
- **OpenJTalk Dictionary**: Build-time embedded via `env!()` macro
- **Package**: ~54MB total with 26+ voice models available for download

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

**Note**: With lazy loading implementation, the daemon now:
- Starts with only 3 popular models (Zundamon, Shikoku Metan, Kasukabe Tsumugi)
- Loads other models on-demand when first used (~500ms delay)
- Reduces memory from ~1.1GB to ~300MB at startup

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

### Memory & Performance Testing

```bash
# After daemon starts, check memory usage
DAEMON_PID=$(pgrep -f voicevox-daemon)
ps -p $DAEMON_PID -o pid,vsz,rss,comm | awk 'NR>1 {print "VSZ(MB):", $2/1024, "RSS(MB):", $3/1024}'

# Expected results with lazy loading:
# - Initial: ~300MB (3 models loaded)
# - After using 10 models: ~600MB
# - All models loaded: ~1.1GB
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
