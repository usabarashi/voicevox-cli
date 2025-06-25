# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is VOICEVOX TTS (`voicevox-tts`) - a production-ready command-line text-to-speech synthesis tool using VOICEVOX Core 0.16.0. It provides a macOS `say` command-compatible interface for Japanese TTS with various character voices like ずんだもん (Zundamon), 四国めたん (Shikoku Metan), etc.

The tool uses a **daemon-client architecture** for optimal performance, with pre-loaded voice models in a background daemon process for instant synthesis. It's specifically optimized for macOS with CPU-only processing and maintains complete compatibility with macOS `say` command behavior (silent operation on success, errors to stderr only).

**Key Features:**
- **Dynamic Voice Detection**: Zero hardcoded voice mappings - automatically adapts to available models
- **Functional Programming Design**: Immutable data structures, monadic composition, and declarative processing
- **High-Performance Architecture**: Optimized for minimal latency with pre-loaded models in daemon
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
│ ✓ FFI Bindings    │                   │ • User-specific         │
└───────────────────┘                   └─────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│                        File Structure                           │
├─────────────────────────────────────────────────────────────────┤
│ src/                                                            │
│ ├── lib.rs              # Shared library & IPC protocols       │
│ ├── bin/                                                        │
│ │   ├── daemon.rs        # Background daemon process            │
│ │   └── client.rs        # CLI client (primary interface)      │
│ ├── core/               # VOICEVOX Core wrapper                 │
│ ├── voice/              # Dynamic voice detection               │
│ ├── paths/              # XDG-compliant path discovery          │
│ ├── client/             # Client-side functionality             │
│ ├── daemon/             # Server-side functionality             │
│ └── ipc/                # Inter-process communication           │
│                                                                 │
│ Static Resources (Build-time):                                  │
│ ├── voicevox_core/      # Statically linked libraries          │
│ └── flake.nix           # Optimized Nix build configuration     │
│                                                                 │
│ Runtime Resources (User directory):                             │
│ └── ~/.local/share/voicevox/models/  # Voice model files       │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│                    Process Flow Diagram                         │
└─────────────────────────────────────────────────────────────────┘

User Command: voicevox-say "Hello"
         │
         ▼
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   Client Start │────►│  Check Daemon   │────►│  Send Request   │
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

- **`src/lib.rs`**: Shared library with VoicevoxCore and IPC protocols
- **`src/bin/daemon.rs`**: Background daemon process with model management  
- **`src/bin/client.rs`**: Lightweight CLI client (primary interface) with functional voice resolution
- **`src/core/mod.rs`**: VOICEVOX Core wrapper with functional programming patterns
- **`src/voice/mod.rs`**: Dynamic voice detection and resolution system
- **`src/paths/mod.rs`**: Functional path discovery and XDG compliance
- **`src/client/`**: Client-side functionality (daemon client, download management)
- **`src/daemon/`**: Server-side functionality (model loading, synthesis)
- **`src/ipc/`**: Inter-process communication protocols and data structures
- **`voicevox_core/`**: VOICEVOX Core runtime libraries (`libvoicevox_core.dylib`) and headers
- **`models/*.vvm`**: VOICEVOX voice model files (26+ models supported)
- **`dict/`**: OpenJTalk dictionary for Japanese text processing

### Key Architecture Patterns

1. **Static Linking Priority**: VOICEVOX Core, ONNX Runtime, OpenJTalk embedded at build time
2. **Daemon-Client IPC**: Unix socket communication with tokio async runtime
3. **Pre-loaded Models**: Daemon loads all available VVM models on startup for instant synthesis
4. **Dynamic Discovery**: Runtime model detection with zero hardcoded mappings
5. **Functional Programming**: Monadic composition, iterator chains, and immutable data flow
6. **Silent Operation**: macOS `say` compatible - no output on success, errors to stderr
7. **User Isolation**: UID-based daemon identification for multi-user support

### Static Linking Architecture

**Production Integration**: Static linking priority with optimized Nix builds:

```
┌─────────────────────────────────────────────────────────────────┐
│                 Static Linking Priority Architecture            │
└─────────────────────────────────────────────────────────────────┘

Build Time (Nix):                    Runtime:
┌─────────────────┐                   ┌─────────────────┐
│  flake.nix      │                   │ voicevox-daemon │
│  Configuration  │                   │ voicevox-say    │
└─────────────────┘                   └─────────────────┘
         │                                     │
         ▼                                     │
┌─────────────────┐                           │
│ Static Linking  │                           │
│ Process         │                           │
├─────────────────┤                           │
│ ✓ VOICEVOX Core │──────────────────────────►│
│ ✓ ONNX Runtime  │  Embedded at Build Time  │
│ ✓ OpenJTalk     │                           │
│ ✓ FFI Bindings  │                           │
└─────────────────┘                           │
         │                                     │
         ▼                                     │
┌─────────────────┐                           │
│ Optimized       │                           │
│ Package (~54MB) │                           │
└─────────────────┘                           │
                                               │
Runtime Download:                              │
┌─────────────────┐                           │
│ Voice Models    │                           │
│ (VVM Files)     │◄──────────────────────────┘
├─────────────────┤    First-run Setup
│ • 26+ Chars     │    User Downloads
│ • ~/.local/...  │    License Acceptance
│ • ~200MB        │
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
- **VOICEVOX Core**: Statically linked `libvoicevox_core.dylib` 
- **ONNX Runtime**: Statically linked `libvoicevox_onnxruntime.dylib` with compatibility symlinks
- **OpenJTalk Dictionary**: Embedded dictionary via static linking
- **Voice Models Only**: Runtime downloads limited to VVM files (~200MB, 26+ characters)
- **Package Size**: ~54MB total with optimized configuration

## Build Commands

### Nix (Recommended - Optimized Static Linking)
```bash
# Build the project (optimized ~54MB package)
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
- **Automatic Paths**: DYLD_LIBRARY_PATH configured automatically

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

# Features (production uses default: link_voicevox)
cargo build --features dynamic_voicevox       # Dynamic library loading
cargo build --features use_bindgen           # Generate FFI bindings
```

## Production Usage

### Daemon Management
```bash
# Start daemon (production - loads all 26+ models)
# Note: With Nix builds, DYLD_LIBRARY_PATH is automatically configured
./target/release/voicevox-daemon

# Development mode (foreground with output)
./target/debug/voicevox-daemon --foreground

# Custom socket path
voicevox-daemon --socket-path /custom/path/daemon.sock

# Stop daemon (user-specific)
pkill -f -u $(id -u) voicevox-daemon
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
./target/release/voicevox-say --daemon-status
./target/release/voicevox-say --list-speakers
./target/release/voicevox-say --list-models

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

# Check what models are available for a specific voice ID
./target/release/voicevox-say --check-updates
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

```bash
# Start development environment
# Note: With Nix builds, library paths are automatically configured
# For Cargo builds:
export DYLD_LIBRARY_PATH=./voicevox_core/c_api/lib:./voicevox_core/onnxruntime/lib

# Test daemon-client workflow
./target/debug/voicevox-daemon --foreground &
sleep 3
./target/debug/voicevox-say "動作テストなのだ"
pkill -f -u $(id -u) voicevox-daemon

# Test various voices dynamically
./target/debug/voicevox-say --speaker-id 3 "スピーカーID 3のテスト"
./target/debug/voicevox-say --model 3 "モデル3のテスト"
./target/debug/voicevox-say --model 16 "モデル16のテスト"

# Test file output
./target/debug/voicevox-say "ファイル出力テスト" -o test.wav

# Test information commands
./target/debug/voicevox-say --list-speakers
./target/debug/voicevox-say --list-models
./target/debug/voicevox-say --daemon-status
./target/debug/voicevox-say --check-updates
```

## Development Notes

### Build System Features
- **Static Linking Priority**: Core libraries embedded at build time (~54MB total)
- **Functional Programming**: Monadic composition, iterator chains, immutable data flow
- **Silent Operation**: macOS `say` compatible behavior (no output on success)
- **Error Handling**: All errors go to stderr, never stdout

### Build Validation
```bash
# Verify build size and functionality
du -sh result/  # Should show ~54MB
./result/bin/voicevox-say "テストなのだ"
./result/bin/voicevox-say --list-speakers

# Check linked libraries
otool -L result/bin/voicevox-say
```

### Code Quality Patterns

**Functional Programming**:
- **Iterator Chains**: `filter_map` → `map` → `collect` patterns over for-loops
- **Monadic Composition**: `Option` and `Result` chaining with `and_then`, `or_else`
- **Immutable Data Flow**: Minimize side effects and mutable state
- **Composable Functions**: Small, single-responsibility functions


### Model Management

**Architecture**:
- **Daemon**: Model loading and speech synthesis only
- **Client**: User interaction, first-run setup, and model downloads  
- **Static Components**: VOICEVOX Core, ONNX Runtime, OpenJTalk dictionary embedded
- **Runtime Downloads**: Voice models (VVM files) only (~200MB, 26+ characters)

**First-Run Setup**:
```bash
# Triggers interactive setup
voicevox-say "初回起動テスト"

# Manual setup
voicevox-setup-models
voicevox-download --output ~/.local/share/voicevox/models
```

**Setup Features**:
- **Interactive License**: Complete license terms for 26+ characters displayed
- **Manual Confirmation**: User must manually review and accept terms
- **XDG Compliance**: Models stored in `~/.local/share/voicevox/models/`
- **Static Dependencies**: Core libraries pre-installed, only VVM downloads needed

### IPC Protocol
- **Unix Sockets**: XDG-compliant socket paths with automatic directory creation
- **Tokio Async**: Full async/await support with length-delimited frames
- **Bincode Serialization**: Efficient binary protocol for requests/responses
- **Socket Priority**: `$VOICEVOX_SOCKET_PATH` → `$XDG_RUNTIME_DIR` → `~/.local/state` → `/tmp`

### Voice System
- **Dynamic Detection**: No hardcoded voice mappings - automatically adapts to available models
- **Model-Based Resolution**: Voice selection via `--model N` or `--speaker-id ID`
- **Runtime Mapping**: Daemon generates voice mappings dynamically from loaded models
- **Future-Proof**: Automatically supports new VOICEVOX models without code changes

## Tips

### Build & Deployment
- **Nix Builds Recommended**: Use `nix build` for optimal static linking (~54MB package)
- **Production**: Always use `--release` builds for performance
- **Static Linking**: Core libraries embedded - no runtime library setup needed

### Usage
- **Silent Operation**: Normal usage produces zero output (like macOS `say`)
- **Voice Discovery**: Use `--list-speakers` to see all available voices and IDs
- **Development**: Use `--foreground` flag on daemon for debugging output
- **Performance**: Daemon startup ~3 seconds, subsequent synthesis instant

### Architecture
- **Responsibility Separation**: Daemon = synthesis only, Client = user interaction + downloads
- **Dynamic Voice System**: Zero hardcoded voice mappings - automatically adapts to new models
- **Functional Programming**: Iterator chains, monadic composition, immutable data flow
- **Storage**: Voice models use ~200MB in `~/.local/share/voicevox/models/`