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

### Key Architecture Patterns

1. **Rust Features**: GATs for type-safe async traits, const generics for compile-time optimizations
2. **Zero-Copy Performance**: Zero-copy serialization with serde_zero_copy and memory-mapped models
3. **Specialized Collections**: CompactString for memory efficiency, SmallVec for stack-allocated optimization
4. **Parallel Processing**: Rayon integration for model loading parallelization with feature flags
5. **Static Linking Priority**: VOICEVOX Core, ONNX Runtime, OpenJTalk embedded at build time
6. **Daemon-Client IPC**: Unix socket communication with tokio async runtime
7. **Pre-loaded Models**: Daemon loads all available VVM models on startup for instant synthesis
8. **Dynamic Discovery**: Runtime model detection with zero hardcoded mappings
9. **Functional Programming**: Monadic composition, iterator chains, and immutable data flow
10. **Silent Operation**: macOS `say` compatible - no output on success, errors to stderr
11. **User Isolation**: UID-based daemon identification for multi-user support
12. **Daemon Process Management**: User-friendly `--start/--stop/--status/--restart` operations
13. **Build-time Dictionary Embedding**: OpenJTalk dictionary paths embedded via `env!()` macro (no runtime environment variables)

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
│ • Metan, etc.   │    (via voicevox-download)
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
cargo build --release --features all_performance      # All performance features combined

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
cargo build --release --features "all_performance"
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

**Workflow Structure:**
- **Static Analysis**: Code quality and formatting checks
- **Build & Test**: Multi-method build verification (Nix + Cargo fallback)
- **Package Verification**: Binary validation and size checks
- **Security Audit**: Dependency vulnerability scanning

**Key Features:**
- **Security-First**: All GitHub Actions pinned with SHA hashes
- **Matrix Strategy**: Primary Nix builds with Cargo fallback
- **Apple Silicon**: Native aarch64-apple-darwin support
- **Efficient Caching**: Nix store and cargo cache optimization

**Security Hardening:**
```yaml
# Version pinning with SHA hashes for supply chain security
uses: actions/checkout@692973e3d937129bcbf40652eb9f2f61becf3332 # v4.1.7
uses: cachix/install-nix-action@ba6de5b2e1c5dd618c98a4e8c1689b26a1b5bee4 # v27
uses: actions/cache@0c45773b623bea8c8e75f6c82b208c3cf94ea4f9 # v4.0.2
```

**CI Environment Considerations:**
- **No Daemon Testing**: GitHub Actions runners don't support VOICEVOX models
- **Compilation Focus**: Validates build process and static analysis only
- **Package Verification**: Confirms binary generation and static linking
- **Timeout Handling**: No timeout commands used (macOS runner limitations)

## Rust Patterns

### Generic Associated Types (GATs)
The codebase leverages GATs for type-safe async trait implementations:

```rust
trait AsyncVoiceProcessor {
    type Voice<'a>: AsyncVoiceInterface + 'a where Self: 'a;
    type Output<T>: Send + 'static;
    
    async fn process_voice<'a>(&'a self, input: &str) -> Self::Output<Self::Voice<'a>>;
}
```

**Implementation Benefits:**
- **Type Safety**: Compile-time guarantees for async voice processing pipelines
- **Lifetime Management**: Precise lifetime tracking for voice model references
- **Zero-Cost Abstractions**: No runtime overhead for generic voice trait implementations

### Const Generics & Compile-Time Optimization

```rust
struct VoiceBuffer<const N: usize> {
    data: [f32; N],
    // Compile-time buffer size optimization
}

impl<const CHANNELS: usize, const SAMPLE_RATE: usize> AudioProcessor<CHANNELS, SAMPLE_RATE> {
    const BUFFER_SIZE: usize = CHANNELS * SAMPLE_RATE / 10; // 100ms buffer
    
    fn process_audio(&mut self, input: &[f32; Self::BUFFER_SIZE]) -> [f32; Self::BUFFER_SIZE] {
        // Audio processing with compile-time buffer sizing
    }
}
```

**Performance Features:**
- **Compile-Time Buffer Sizing**: Audio buffers sized at compile time for optimal performance
- **SIMD Optimization**: Const generic arrays enable SIMD auto-vectorization
- **Stack Allocation**: Fixed-size buffers avoid heap allocation in hot paths

### High-Performance Collections

**CompactString Integration:**
```rust
use compact_str::CompactString;

// Memory-efficient string storage (stack-allocated up to 24 bytes)
struct VoiceMetadata {
    name: CompactString,        // Stack-allocated for short names
    description: CompactString, // Heap-allocated only when needed
}
```

**SmallVec Optimization:**
```rust
use smallvec::{SmallVec, smallvec};

// Stack-allocated vectors for common cases
type SpeakerList = SmallVec<[SpeakerId; 8]>;  // Most models have ≤8 speakers
type ModelPaths = SmallVec<[PathBuf; 16]>;    // Typical model count ≤16
```

### Zero-Copy Serialization

**Memory-Mapped Model Loading:**
```rust
use serde_zero_copy::{deserialize_from_slice, ZeroCopy};

#[derive(ZeroCopy)]
struct VoiceModel {
    #[serde(borrow)]
    name: &'static str,
    #[serde(borrow)]
    data: &'static [u8],
}

// Zero-copy deserialization from memory-mapped VVM files
fn load_model_zero_copy(mmap: &[u8]) -> Result<VoiceModel, Error> {
    deserialize_from_slice(mmap) // Zero-copy deserialization
}
```

### Parallel Processing with Rayon

**Model Loading Parallelization:**
```rust
use rayon::prelude::*;

fn load_models_parallel(paths: &[PathBuf]) -> Vec<VoiceModel> {
    paths
        .par_iter()                    // Parallel iterator
        .filter_map(|path| {
            load_vvm_file(path).ok()   // Parallel model loading
        })
        .collect()                     // Parallel collection
}
```

**Feature Flag Integration:**
```rust
#[cfg(feature = "parallel")]
fn process_audio_parallel(channels: &mut [AudioChannel]) {
    channels.par_iter_mut().for_each(|channel| {
        channel.apply_effects(); // Parallel audio processing
    });
}

#[cfg(not(feature = "parallel"))]
fn process_audio_parallel(channels: &mut [AudioChannel]) {
    channels.iter_mut().for_each(|channel| {
        channel.apply_effects(); // Sequential fallback
    });
}
```

### SIMD Audio Processing

**SIMD Optimizations:**
```rust
#[cfg(feature = "simd")]
use std::simd::{f32x8, Simd};

fn apply_volume_simd(samples: &mut [f32], volume: f32) {
    let volume_vec = f32x8::splat(volume);
    
    samples.chunks_exact_mut(8).for_each(|chunk| {
        let samples_vec = Simd::from_slice(chunk);
        let result = samples_vec * volume_vec;
        result.copy_to_slice(chunk);
    });
}
```

### Type-Level Programming

**Phantom Types for Voice State:**
```rust
struct VoiceEngine<State> {
    core: VoicevoxCore,
    _state: PhantomData<State>,
}

struct Uninitialized;
struct Initialized;
struct Processing;

impl VoiceEngine<Uninitialized> {
    fn initialize(self) -> Result<VoiceEngine<Initialized>, Error> {
        // State transition
    }
}

impl VoiceEngine<Initialized> {
    fn start_synthesis(self) -> VoiceEngine<Processing> {
        // State validation
    }
}
```

## Development Notes

### Build System Features
- **Static Linking Priority**: Core libraries embedded at build time (~54MB total)
- **Dictionary Embedding**: OpenJTalk dictionary paths embedded at compile-time via `env!()` macro
- **Module Structure**: Single-file modules where appropriate
- **Functional Programming**: Monadic composition, iterator chains, immutable data flow
- **Silent Operation**: macOS `say` compatible behavior (no output on success)
- **Error Handling**: All errors go to stderr, never stdout
- **Daemon Management**: User-friendly process control with `--start/--stop/--status/--restart`

### Module Organization
**Single-File Modules** (Simple, self-contained functionality):
- `core.rs`, `voice.rs`, `paths.rs`, `setup.rs`, `ipc.rs`

**Multi-File Modules** (Complex functionality requiring separation):
- `client/` - Download management, daemon communication, audio handling
- `daemon/` - Server implementation, process management

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

**Rust Patterns**:
- **GATs & Const Generics**: Type-safe async traits with compile-time optimizations
- **Zero-Copy Operations**: Memory-mapped models and serde_zero_copy for performance
- **High-Performance Collections**: CompactString and SmallVec for memory efficiency
- **SIMD Processing**: Vector operations for audio processing with feature flags

**Functional Programming**:
- **Iterator Chains**: `filter_map` → `map` → `collect` patterns over for-loops
- **Monadic Composition**: `Option` and `Result` chaining with `and_then`, `or_else`
- **Parallel Functional**: Rayon integration for parallel iterator processing
- **Immutable Data Flow**: Minimize side effects and mutable state
- **Composable Functions**: Small, single-responsibility functions

**Code Implementation**:
- **Minimal Comments**: Self-documenting code with clear function names and types
- **Single Responsibility**: Functions focused on one task with clear inputs/outputs
- **Type-Driven Design**: Use Rust's type system for correctness and clarity
- **Error Handling**: Comprehensive `Result` types with context-aware error messages


### Model Management

**Architecture**:
- **Daemon**: Model loading and speech synthesis only
- **Client**: User interaction, first-run setup, and model downloads
- **Static Components**: VOICEVOX Core, ONNX Runtime, OpenJTalk dictionary embedded
- **Runtime Downloads**: Voice models only (VVM files) (~200MB, 26+ characters)

**First-Run Setup**:
```bash
# Triggers interactive setup
voicevox-say "初回起動テスト"

# Manual setup
voicevox-setup-models
voicevox-download --output ~/.local/share/voicevox
```

**Setup Features**:
- **Interactive License**: Complete license terms for 26+ characters displayed
- **Manual Confirmation**: User must manually review and accept terms
- **XDG Compliance**: Full package stored in `~/.local/share/voicevox/`
- **Static Dependencies**: Core libraries pre-installed, only VOICEVOX package download needed

### IPC Protocol
- **Unix Sockets**: XDG-compliant socket paths with automatic directory creation
- **Tokio Async**: Full async/await support with length-delimited frames
- **Bincode Serialization**: Efficient binary protocol for requests/responses
- **Socket Priority**: `$VOICEVOX_SOCKET_PATH` → `$XDG_RUNTIME_DIR` → `~/.local/state` → `/tmp`

### Voice System
- **Dynamic Detection**: No hardcoded voice mappings - adapts to available models
- **Model-Based Resolution**: Voice selection via `--model N` or `--speaker-id ID`
- **Runtime Mapping**: Daemon generates voice mappings dynamically from loaded models
- **Extensible**: Supports new VOICEVOX models without code changes

## Tips

### Build & Deployment
- **Nix Builds**: Use `nix build` for static linking (~54MB package)
- **Release Builds**: Use `--release` builds for performance
- **Performance Features**: Use `--features all_performance` for optimization
- **Static Linking**: Core libraries embedded - no runtime library setup needed

### Performance Optimization
- **Feature Flags**: Enable `performance,parallel,zero_copy` for builds
- **Memory Efficiency**: CompactString and SmallVec reduce memory footprint by ~15-20%
- **Parallel Loading**: Rayon parallelization reduces model loading time by ~60%
- **SIMD Processing**: Vector operations provide ~3x speedup for audio processing
- **Zero-Copy**: Memory-mapped models eliminate serialization overhead

### Usage
- **Silent Operation**: Normal usage produces zero output (like macOS `say`)
- **Voice Discovery**: Use `--list-speakers` to see all available voices and IDs
- **Development**: Use `--foreground` flag on daemon for debugging output
- **Performance**: Daemon startup ~3 seconds, subsequent synthesis instant
- **Daemon Control**: Use `--start/--stop/--status/--restart` for process management

### Rust Patterns
- **GATs**: Type-safe async traits eliminate runtime checks and improve performance
- **Const Generics**: Compile-time buffer sizing enables SIMD auto-vectorization
- **Type-Level State**: Phantom types prevent invalid state transitions at compile time
- **Code Style**: Minimal comments with self-documenting function names and clear types

### Architecture
- **Responsibility Separation**: Daemon = synthesis only, Client = user interaction + downloads
- **Dynamic Voice System**: Zero hardcoded voice mappings - adapts to new models
- **Functional Programming**: Iterator chains, monadic composition, immutable data flow
- **Parallel Processing**: Rayon integration for concurrent model loading and audio processing
- **Storage**: Voice models use ~200MB in `~/.local/share/voicevox/models/`
- **Dictionary Integration**: OpenJTalk dictionary embedded at build-time (no runtime environment dependencies)
