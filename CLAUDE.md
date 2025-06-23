# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is VOICEVOX TTS (`voicevox-tts`) - a production-ready command-line text-to-speech synthesis tool using VOICEVOX Core 0.16.0. It provides a macOS `say` command-compatible interface for Japanese TTS with various character voices like ずんだもん (Zundamon), 四国めたん (Shikoku Metan), etc.

The tool uses a **daemon-client architecture** for optimal performance, with pre-loaded voice models in a background daemon process for instant synthesis. It's specifically optimized for macOS with CPU-only processing and maintains complete compatibility with macOS `say` command behavior (silent operation on success, errors to stderr only).

## Architecture

### Daemon-Client Architecture

**Production Architecture**: The system now uses a high-performance daemon-client model instead of standalone execution:

- **`voicevox-daemon`**: Background process with pre-loaded VOICEVOX models
- **`voicevox-say`**: Lightweight client that communicates with daemon via Unix sockets
- **`voicevox-tts`**: Legacy standalone binary (kept for compatibility)

### Core Components

- **`src/lib.rs`**: Shared library with VoicevoxCore and IPC protocols
- **`src/bin/daemon.rs`**: Background daemon process with model management
- **`src/bin/client.rs`**: Lightweight CLI client (primary interface)
- **`src/main.rs`**: Legacy standalone implementation
- **`voicevox_core/`**: VOICEVOX Core runtime libraries (`libvoicevox_core.dylib`) and headers
- **`models/*.vvm`**: VOICEVOX voice model files (19 models supported)
- **`dict/`**: OpenJTalk dictionary for Japanese text processing

### Key Architecture Patterns

1. **Daemon-Client IPC**: Unix socket communication with tokio async runtime
2. **Pre-loaded Models**: Daemon loads all 19 VVM models on startup for instant synthesis  
3. **CPU-Only Processing**: Hardcoded CPU mode on macOS (no GPU dependencies)
4. **Silent Operation**: macOS `say` compatible - no output on success, errors to stderr
5. **Automatic Fallback**: Client → Daemon → Standalone → Error progression
6. **Process Management**: Duplicate daemon prevention and graceful shutdown

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
# Build all binaries (daemon + client + legacy)
export DYLD_LIBRARY_PATH=./voicevox_core/c_api/lib:./voicevox_core/onnxruntime/lib
cargo build --release

# Build specific binaries
cargo build --release --bin voicevox-daemon   # Background service
cargo build --release --bin voicevox-say      # Primary CLI (client)
cargo build --release --bin voicevox-tts      # Legacy standalone

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

# Stop daemon
pkill -f voicevox-daemon
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

# Voice selection by name
./target/release/voicevox-say --voice zundamon "名前指定なのだ"

# Status and information (only commands that produce output)
./target/release/voicevox-say --daemon-status
./target/release/voicevox-say --list-speakers
./target/release/voicevox-say --voice "?"

# Force standalone mode
./target/release/voicevox-say --standalone "テストメッセージ"

# Read from stdin
echo "標準入力からのテキスト" | ./target/release/voicevox-say
```

## Available Voices

### Main Characters
```bash
# Zundamon (ずんだもん) - 8 variations
--speaker-id 3   # ノーマル (Normal)
--speaker-id 1   # あまあま (Sweet)  
--speaker-id 7   # ツンツン (Tsundere)
--speaker-id 5   # セクシー (Sexy)
--speaker-id 22  # ささやき (Whisper)
--speaker-id 38  # ヒソヒソ (Hushed)
--speaker-id 75  # ヘロヘロ (Exhausted)
--speaker-id 76  # なみだめ (Crying)

# Shikoku Metan (四国めたん) - 6 variations  
--speaker-id 2   # ノーマル (Normal)
--speaker-id 0   # あまあま (Sweet)
--speaker-id 6   # ツンツン (Tsundere) 
--speaker-id 4   # セクシー (Sexy)
--speaker-id 36  # ささやき (Whisper)
--speaker-id 37  # ヒソヒソ (Hushed)

# Other Popular Characters
--speaker-id 8   # 春日部つむぎ (Kasukabe Tsumugi)
--speaker-id 9   # 波音リツ (Namine Ritsu)
--speaker-id 10  # 雨晴はう (Amahare Hau)
--speaker-id 16  # 九州そら (Kyushu Sora)
--speaker-id 20  # もち子さん (Mochiko-san)
```

## Testing & Development

```bash
# Start development environment
export DYLD_LIBRARY_PATH=./voicevox_core/c_api/lib:./voicevox_core/onnxruntime/lib

# Test daemon-client workflow
./target/debug/voicevox-daemon --foreground &
sleep 3
./target/debug/voicevox-say "動作テストなのだ"
pkill -f voicevox-daemon

# Test various voices
./target/debug/voicevox-say --speaker-id 3 "ずんだもんノーマル"
./target/debug/voicevox-say --speaker-id 1 "ずんだもんあまあま"
./target/debug/voicevox-say --speaker-id 7 "ずんだもんツンツン"

# Test file output
./target/debug/voicevox-say "ファイル出力テスト" -o test.wav

# Test information commands
./target/debug/voicevox-say --list-speakers
./target/debug/voicevox-say --daemon-status
```

## Development Notes

### Production Architecture  
- **No Dummy Code**: Completely removed all dummy implementations
- **Real VOICEVOX Only**: Production builds require actual VOICEVOX Core libraries
- **Silent Operation**: macOS `say` compatible behavior (no output on success)
- **Error Handling**: All errors go to stderr, never stdout

### Model Management
- **All Models Default**: Daemon loads all 19 VVM models on startup
- **Environment Independent**: Automatic path discovery for models and dictionaries
- **Duplicate Prevention**: Multiple daemon startup protection

### IPC Protocol
- **Unix Sockets**: `/Users/{user}/.voicevox/daemon.sock` default path
- **Tokio Async**: Full async/await support with length-delimited frames
- **Bincode Serialization**: Efficient binary protocol for requests/responses
- **Automatic Fallback**: Client automatically starts daemon if needed

### Voice System
- **99 Voice Styles**: 26 characters with multiple emotional variants
- **Style ID Mapping**: Direct speaker ID specification or name resolution
- **Character Variety**: From cute (Zundamon) to mature (No.7) to dramatic (后鬼)

## Tips

- **Production Deployment**: Always use `--release` builds for performance
- **Silent Operation**: Normal usage produces zero output (like macOS `say`)
- **Voice Discovery**: Use `--list-speakers` to see all available voices and IDs  
- **Development**: Use `--foreground` flag on daemon for debugging output
- **Performance**: Daemon startup takes ~3 seconds but subsequent synthesis is instant