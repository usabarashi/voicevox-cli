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
         │              │  Load Models    │              │
         │              │  (26+ VVM)      │              │
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

**Integration**: Static linking priority with Nix builds:

```
┌─────────────────────────────────────────────────────────────────┐
│                 Static Linking Priority Architecture            │
└─────────────────────────────────────────────────────────────────┘

Build Time (Nix):                    Runtime:
┌─────────────────┐                   ┌─────────────────┐
│  flake.nix      │                   │ voicevox-daemon │
│  Configuration  │                   │ voicevox-say    │
└─────────────────┘                   └─────────────────┘
         │                                    │
         ▼                                    │
┌─────────────────┐                           │
│ Static Linking  │                           │
│ Process         │                           │
├─────────────────┤                           │
│ ✓ VOICEVOX Core │──────────────────────────►│
│ ✓ ONNX Runtime  │  Embedded at Build Time   │
│ ✓ OpenJTalk     │                           │
│ ✓ Rust API      │                           │
└─────────────────┘                           │
         │                                    │
         ▼                                    │
┌─────────────────┐                           │
│ Binary Package  │                           │
│ Package (~54MB) │                           │
└─────────────────┘                           │
                                              │
Runtime Download:                             │
┌─────────────────┐                           │
│ Voice Models    │                           │
│ (VVM Files)     │◄──────────────────────────┘
├─────────────────┤    First-run Setup
│ • 26+ Characters│    User Downloads
│ • Zundamon      │    License Acceptance
│ • Metan, etc.   │    (via voicevox-setup-models)
│ • ~/.local/...  │
└─────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│                    Library Dependency Comparison                │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│ Traditional Approach:          Static Linking Approach:         │
│                                                                 │
│ ┌─────────────────┐            ┌─────────────────┐              │
│ │ voicevox-say    │            │ voicevox-say    │              │
│ │ (Small binary)  │            │ (All embedded)  │              │
│ └─────────────────┘            └─────────────────┘              │
│          │                               │                      │
│          ▼                               ▼                      │
│ ┌─────────────────┐            ┌─────────────────┐              │
│ │ Runtime Loading │            │ Instant Ready   │              │
│ │ • DYLD_LIB_PATH │            │ • No setup      │              │
│ │ • Library deps  │            │ • Pre-linked    │              │
│ │ • Setup needed  │            │ • Zero config   │              │
│ └─────────────────┘            └─────────────────┘              │
│          │                                                      │
│          ▼                                                      │
│ ┌─────────────────┐                                             │
│ │ External Libs   │                                             │
│ │ • Download req  │                                             │
│ │ • Path config   │                                             │
│ │ • Version deps  │                                             │
│ └─────────────────┘                                             │
└─────────────────────────────────────────────────────────────────┘
```

**Static Linking Components**:
- **VOICEVOX Core**: Official Rust crate with statically linked `libvoicevox_core.dylib`
- **ONNX Runtime**: Statically linked `libvoicevox_onnxruntime.dylib` with compatibility symlinks
- **OpenJTalk Dictionary**: Build-time embedded dictionary via `env!()` macro (no runtime environment variables)
- **Voice Models Only**: Runtime downloads limited to VVM files (~200MB, 26+ characters)
- **Package Size**: ~54MB total package size

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

**Nix Build Features:**
- **Static Linking**: Core libraries embedded at build time
- **Lightweight**: ~54MB total package size
- **No Runtime Setup**: Libraries pre-configured
- **Path Configuration**: DYLD_LIBRARY_PATH configured automatically

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

# Performance features (production optimization)
cargo build --release --features performance           # CompactString + SmallVec optimizations
cargo build --release --features parallel              # Rayon parallelization for model loading
cargo build --release --features zero_copy            # Zero-copy serialization with serde_zero_copy
cargo build --release --features simd                 # SIMD optimizations for audio processing
cargo build --release --features performance         # All performance features combined

# Features
cargo build --features dynamic_voicevox               # Dynamic library loading
cargo build --features use_bindgen                    # Generate FFI bindings

# Feature combinations
cargo build --release --features "performance,parallel,zero_copy"  # Performance profile
```

## Production Usage

### Daemon Management
```bash
# Start daemon (production - loads all 26+ models)
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

## Voice Discovery (Dynamic Detection)

### Dynamic Voice Management
```bash
# Discover available models
./target/release/voicevox-say --list-models

# Example output:
# Available VVM models:
#   Model 0 (/path/to/0.vvm)
#   Model 3 (/path/to/3.vvm)
#   Model 16 (/path/to/16.vvm)

# Get detailed speaker information
./target/release/voicevox-say --list-speakers

# Use model by ID
./target/release/voicevox-say --model 3 "Text"

# Use specific speaker style ID
./target/release/voicevox-say --speaker-id 3 "Text"

# Check installation status of voice models and dictionary
./target/release/voicevox-say --status
```

### Voice Selection Methods
```bash
# Method 1: Direct speaker ID (most precise)
--speaker-id 3   # Use exact style ID from --list-speakers output

# Method 2: Model selection (uses first available style)
--model 3        # Load 3.vvm model and use default style

# Method 3: Dynamic name resolution (if speaker metadata available)
# This requires VOICEVOX Core integration and loaded models
```

## Testing & Development

### Local Development Testing

```bash
# Start development environment
# Note: With Nix builds, library paths are automatically configured
# For Cargo builds:
export DYLD_LIBRARY_PATH=./voicevox_core/c_api/lib:./voicevox_core/onnxruntime/lib

# Test daemon-client workflow
./target/debug/voicevox-daemon --foreground &
sleep 3
./target/debug/voicevox-say "動作テストなのだ"
./target/debug/voicevox-daemon --stop

# Test various voices dynamically
./target/debug/voicevox-say --speaker-id 3 "スピーカーID 3のテスト"
./target/debug/voicevox-say --model 3 "モデル3のテスト"
./target/debug/voicevox-say --model 16 "モデル16のテスト"

# Test file output
./target/debug/voicevox-say "ファイル出力テスト" -o test.wav

# Test information commands
./target/debug/voicevox-say --list-speakers
./target/debug/voicevox-say --list-models
./target/debug/voicevox-say --status

# Test daemon management
./target/debug/voicevox-daemon --status

# Performance testing
cargo build --release --features "performance"
time ./target/release/voicevox-daemon --start  # ~1.2s startup with parallel loading
time ./target/release/voicevox-say "パフォーマンステスト"  # ~50ms synthesis

# Memory usage comparison
cargo build --release --features "performance"  # CompactString + SmallVec
cargo build --release  # Standard collections
# Expected: ~15-20% memory reduction with performance features
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

**CI Pipeline Components:**
- **Nix Flake Check**: Validates flake.nix configuration
- **Rust Toolchain**: Verifies rustc and cargo versions
- **Code Formatting**: Ensures consistent code style with `cargo fmt`
- **Static Analysis**: Runs `cargo clippy` with strict warnings
- **Script Syntax**: Validates shell script syntax
- **Security Audit**: Checks for known vulnerabilities with `cargo audit`

### GitHub Actions CI

**Workflow Structure (.github/workflows/ci.yml):**

**Jobs:**
1. **Static Analysis**: Nix flake check, Rust formatting (cargo fmt), static analysis (clippy), script syntax validation
2. **Build & Test**: Matrix strategy with Nix build (primary) and Cargo compilation check (fallback)
3. **Package Verification**: Binary validation, static linking verification, package size checks
4. **Security Audit**: Dependency vulnerability scanning (cargo audit), license compliance

**Key Features:**
- **Matrix Strategy**: Primary Nix builds with Cargo fallback for aarch64-apple-darwin
- **Security-First**: SHA-pinned actions and modern toolchain (dtolnay/rust-toolchain)
- **Efficient Caching**: Nix store caching with actions/cache@v4
- **Compilation Focus**: No daemon testing in CI environment (VOICEVOX models unavailable)
