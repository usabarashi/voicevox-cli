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

## Architecture

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

1. **Daemon-Client IPC**: Unix socket communication with tokio async runtime
2. **Pre-loaded Models**: Daemon loads all available VVM models on startup for instant synthesis  
3. **CPU-Only Processing**: Hardcoded CPU mode on macOS (no GPU dependencies)
4. **Silent Operation**: macOS `say` compatible - no output on success, errors to stderr
5. **Automatic Fallback**: Client → Daemon → Standalone → Error progression
6. **Process Management**: Duplicate daemon prevention and graceful shutdown
7. **Responsibility Separation**: Client-side setup, daemon-side synthesis
8. **User Isolation**: UID-based daemon identification for multi-user support
9. **Functional Programming**: Monadic composition, iterator chains, and immutable data flow
10. **Dynamic Discovery**: Runtime model detection with zero hardcoded mappings

### FFI Integration

**Production Integration**: No dummy implementations - real VOICEVOX Core integration only:
- **Static Linking**: Direct FFI calls to `libvoicevox_core.dylib` 
- **Dynamic Loading**: Optional `dynamic_voicevox` feature for runtime loading
- **Manual Bindings**: Production-ready manual FFI bindings (no bindgen dependency)

## Build Commands

### Nix (Recommended)
```bash
# Build the project
nix build

# Run daemon directly
nix run .#voicevox-daemon

# Run client directly
nix run .#voicevox-say -- "テストメッセージ"

# Development shell
nix develop
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

# Features (production uses default: link_voicevox)
cargo build --features dynamic_voicevox       # Dynamic library loading
cargo build --features use_bindgen           # Generate FFI bindings
```

## Production Usage

### Daemon Management
```bash
# Start daemon (production - loads all 19 models)
export DYLD_LIBRARY_PATH=./voicevox_core/c_api/lib:./voicevox_core/onnxruntime/lib
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

### Production Architecture  
- **No Dummy Code**: Completely removed all dummy implementations
- **Real VOICEVOX Only**: Production builds require actual VOICEVOX Core libraries
- **Silent Operation**: macOS `say` compatible behavior (no output on success)
- **Error Handling**: All errors go to stderr, never stdout
- **Functional Programming**: Deep refactoring from imperative to functional patterns throughout codebase

### Code Quality & Patterns

**Functional Programming Implementation**:
- **Iterator Chains**: Replace for-loops with `filter_map` → `map` → `collect` patterns
- **Monadic Composition**: `Option` and `Result` chaining with `and_then`, `or_else`, `unwrap_or`
- **Early Return Elimination**: Functional alternatives to nested if-else and early returns
- **Responsibility Separation**: Small, composable functions with single responsibilities
- **Immutable Data Flow**: Minimize side effects and mutable state

**Performance Optimizations**:
- **Functional Fast Paths**: Optimized short-circuit evaluation for simple cases
- **Memory Efficiency**: String pre-allocation and minimal copying in pipelines
- **Recursive Efficiency**: Tail-call optimization patterns for file tree traversal
- **Pipeline Composition**: Lazy evaluation patterns for large data processing

**Architectural Refactoring Examples**:
```rust
// Before: Deep nested imperative style
for entry in entries.filter_map(|e| e.ok()) {
    let entry_path = entry.path();
    if entry_path.is_file() {
        loaded_count += self.try_load_vvm_file(&entry_path);
    } else if entry_path.is_dir() {
        let _ = self.load_vvm_files_recursive(&entry_path);
    }
}

// After: Functional composition with separated concerns
let loaded_count = entries
    .filter_map(Result::ok)
    .map(|entry| entry.path())
    .map(|path| self.process_entry_path(&path))
    .sum::<u32>();
```

### Model Management

**Responsibility Separation Architecture**:
- **Daemon**: Model loading and speech synthesis only (no download capability)  
- **Client**: User interaction, first-run setup, and model downloads
- **All Models Default**: Daemon loads all available VVM models on startup (~26+ characters)
- **Environment Independent**: Automatic path discovery for models and dictionaries
- **Duplicate Prevention**: Multiple daemon startup protection via UID-based isolation
- **Recursive VVM Search**: Deep directory scanning for VVM files in nested structures

#### Voice Model Setup Process

**Client-Side First-Run Setup (Current Implementation)**:
```bash
# First time usage triggers client-side interactive setup
voicevox-say "初回起動テスト"

# Client-side workflow:
# 1. Checks for existing models with find_models_dir_client()
# 2. If not found, prompts user for download consent
# 3. Launches VOICEVOX downloader with direct user interaction
# 4. User manually accepts license terms for 26+ voice characters
# 5. Models downloaded to ~/.local/share/voicevox/models/
```

**Interactive License Acceptance**:
```bash
# User sees complete license display with pager:
🎭 VOICEVOX CLI - First Run Setup
Voice models are required for text-to-speech synthesis.

Would you like to download voice models now? [Y/n]: y
🔄 Starting voice model download...
Note: This will require accepting VOICEVOX license terms.

📦 Target directory: ~/.local/share/voicevox/models
🔄 Launching VOICEVOX downloader...
   Please follow the on-screen instructions to accept license terms.
   Press Enter when ready to continue...

# Complete license terms displayed for:
# - VOICEVOX Audio Model License  
# - Individual voice library terms (26+ characters)
# - VOICEVOX ONNX Runtime License
# User presses 'q' to exit pager, then 'y' to accept
```

**Manual Setup**:
```bash
# Use dedicated setup command
voicevox-setup-models

# Or direct downloader  
voicevox-download --output ~/.local/share/voicevox/models
```

**Setup Features**:
- **Client-Side Responsibility**: Model downloads handled by voicevox-say client
- **Official VOICEVOX Downloader**: Direct integration with `voicevox-download` from VOICEVOX Core
- **Complete License Display**: Interactive pager shows all 26+ character license terms
- **Manual User Confirmation**: No automated acceptance - user must manually review and accept
- **XDG Compliance**: Models stored in `~/.local/share/voicevox/models/`
- **Size Information**: ~1.1GB download (26+ voice models)
- **Automatic Detection**: Recursive VVM file discovery after download
- **Graceful Fallback**: If download fails/declined, falls back to standalone mode

### IPC Protocol
- **Unix Sockets**: XDG-compliant socket paths with automatic directory creation
- **Tokio Async**: Full async/await support with length-delimited frames
- **Bincode Serialization**: Efficient binary protocol for requests/responses
- **Automatic Fallback**: Client automatically starts daemon if needed

### Socket Path Priority (XDG Base Directory Specification)
1. `$VOICEVOX_SOCKET_PATH` (environment override)
2. `$XDG_RUNTIME_DIR/voicevox-daemon.sock` (runtime files)
3. `$XDG_STATE_HOME/voicevox-daemon.sock` (persistent state)
4. `~/.local/state/voicevox-daemon.sock` (XDG fallback)
5. `/tmp/voicevox-daemon-{uid}.sock` (temporary, user-specific by UID)

### Voice System (Dynamic Detection Architecture)
- **Fully Dynamic Voice Detection**: No hardcoded voice mappings - automatically adapts to available VVM models
- **VOICEVOX Core Integration**: Direct speaker information from `libvoicevox_core.dylib` for accurate voice metadata
- **Model-Based Resolution**: Voice selection via `--model N` for N.vvm files or `--speaker-id ID` for specific styles
- **Automatic Model Scanning**: Recursive VVM file discovery with `scan_available_models()`
- **Runtime Voice Mapping**: Daemon generates voice mappings dynamically from loaded models
- **Zero Hardcoding**: Removed all hardcoded voice names (zundamon, metan, tsumugi, etc.)
- **Future-Proof**: Automatically supports new VOICEVOX models without code changes

## Tips

- **Production Deployment**: Always use `--release` builds for performance
- **Silent Operation**: Normal usage produces zero output (like macOS `say`)
- **Voice Discovery**: Use `--list-speakers` to see all available voices and IDs  
- **Development**: Use `--foreground` flag on daemon for debugging output
- **Performance**: Daemon startup takes ~3 seconds but subsequent synthesis is instant
- **First-Run Setup**: Client handles initial setup - users can accept (Y) or decline (n) model download
- **License Compliance**: Complete VOICEVOX license terms displayed interactively during setup
- **Manual License Review**: No automated acceptance - users must manually review all terms
- **Storage Management**: Voice models use ~1.1GB in `~/.local/share/voicevox/models/`
- **User Isolation**: Daemon processes isolated by UID for multi-user systems (improved duplicate checking)
- **Responsibility Separation**: Daemon = synthesis only, Client = user interaction + downloads
- **Recursive Model Search**: VVM files discovered in nested directory structures automatically
- **Cleanup Automation**: Unnecessary files (zip, tgz, tar.gz) removed after download
- **Dynamic Voice System**: Zero hardcoded voice mappings - automatically adapts to new models
- **Individual Updates**: Selective model updates with `--update-models`, `--update-dict`, `--update-model N`
- **Version Management**: Complete version tracking with `--version-info` and `--check-updates`
- **Functional Programming**: Codebase uses functional patterns - prefer iterator chains over for-loops
- **Code Style**: Monadic composition over nested conditionals, small composable functions over large implementations
- **Error Handling**: `Result` and `Option` chaining patterns for clean error propagation
- **Performance**: Functional fast paths with early bailout for optimal performance in common cases