# VOICEVOX TTS

Production-ready daemon-client TTS tool using VOICEVOX Core 0.16.0

## Features

- **Daemon-Client Architecture**: High-performance daemon process for fast voice synthesis
- **macOS say Compatible**: Silent operation like macOS say command
- **99 Voice Styles**: 26 characters including ずんだもん (Zundamon), 四国めたん (Shikoku Metan), 春日部つむぎ (Kasukabe Tsumugi)
- **Instant Voice Synthesis**: Pre-loaded models for immediate voice generation
- **CPU-Only Processing**: macOS optimized (CUDA/DirectML disabled)
- **XDG Compliant**: Standard Unix file placement rules
- **Environment Independent**: Zero-configuration with automatic path discovery

## Architecture

### Production System

1. **`voicevox-daemon`**: Background process with all VVM models pre-loaded
2. **`voicevox-say`**: Lightweight CLI client (primary interface)
3. **`voicevox-tts`**: Legacy standalone binary (compatibility maintained)

### IPC Communication

- **Unix Sockets**: XDG-compliant file placement
- **Tokio Async**: High-performance asynchronous I/O communication
- **Bincode**: Efficient binary protocol

### Socket Path Priority

1. `$VOICEVOX_SOCKET_PATH` (environment variable)
2. `$XDG_RUNTIME_DIR/voicevox/daemon.sock` (runtime)
3. `$XDG_STATE_HOME/voicevox/daemon.sock` (state)
4. `~/.local/state/voicevox/daemon.sock` (fallback)
5. `$TMPDIR/voicevox-daemon-{pid}.sock` (temporary)

## Installation

### Nix (Recommended)

```bash
# Build and install
nix build

# Direct execution
nix run . -- "こんにちは、ずんだもんなのだ"

# Development environment
nix develop
```

### Using as Nix Flake

#### Add as Input

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    voicevox-tts.url = "github:usabarashi/voicevox-tts";
  };

  outputs = { nixpkgs, voicevox-tts, ... }: {
    packages.aarch64-darwin.default = nixpkgs.legacyPackages.aarch64-darwin.mkShell {
      buildInputs = [ voicevox-tts.packages.aarch64-darwin.default ];
    };
  };
}
```

#### Using as Overlay

```nix
nixpkgs.overlays = [ voicevox-tts.overlays.default ];

environment.systemPackages = with pkgs; [
  voicevox-tts   # or voicevox-say
];
```

### Cargo (Development)

```bash
# Required environment variables
export DYLD_LIBRARY_PATH=./voicevox_core/c_api/lib:./voicevox_core/onnxruntime/lib

# Production build
cargo build --release --bin voicevox-daemon --bin voicevox-say

# Development build
cargo build --bin voicevox-daemon --bin voicevox-say
```

## Usage

### Daemon-Client Mode (Recommended)

```bash
# Voice synthesis with automatic daemon startup
voicevox-say "こんにちは、ずんだもんなのだ"

# Voice selection
voicevox-say -v zundamon-amama "あまあまモードなのだ♪"
voicevox-say -v metan-tsundere "ツンツンめたんです"

# File output
voicevox-say -o output.wav "保存するテキスト"

# From stdin
echo "パイプからの入力" | voicevox-say

# Daemon status check
voicevox-say --daemon-status
```

### Direct Daemon Operations

```bash
# Manual daemon startup (foreground)
voicevox-daemon --foreground

# Manual daemon startup (background)
voicevox-daemon

# Stop daemon
pkill -f voicevox-daemon
```

### Voice Discovery

```bash
# List available voices
voicevox-say -v "?"

# Detailed speaker information
voicevox-say --list-speakers

# Direct speaker ID specification
voicevox-say --speaker-id 3 "ずんだもん（ノーマル）"
```

### Standalone Mode

```bash
# Force standalone without daemon
voicevox-say --standalone "独立実行モード"

# Minimal models (fast startup)
voicevox-say --standalone --minimal-models "軽量モード"
```

## Voice Characters

### Main Characters

**ずんだもん (Zundamon) - 8 Variations**
- `zundamon` / `--speaker-id 3` - ノーマル (Normal)
- `zundamon-amama` / `--speaker-id 1` - あまあま (Sweet)
- `zundamon-tsundere` / `--speaker-id 7` - ツンツン (Tsundere)
- `zundamon-sexy` / `--speaker-id 5` - セクシー (Sexy)
- `zundamon-whisper` / `--speaker-id 22` - ささやき (Whisper)
- Plus 3 additional emotional expressions

**四国めたん (Shikoku Metan) - 6 Variations**
- `metan` / `--speaker-id 2` - ノーマル (Normal)
- `metan-amama` / `--speaker-id 0` - あまあま (Sweet)
- `metan-tsundere` / `--speaker-id 6` - ツンツン (Tsundere)
- Plus 3 additional emotional expressions

**Other 16 Characters**
- 春日部つむぎ (Kasukabe Tsumugi), 雨晴はう (Amehare Hau), 波音リツ (Namine Ritsu), 玄野武宏 (Kurono Takehiro), 白上虎太郎 (Shiragami Kotaro), etc.

## Technical Specifications

### Core Technology

- **VOICEVOX Core**: 0.16.0 (MIT License)
- **Runtime**: CPU-only processing on macOS
- **Audio Format**: WAV (16bit, 24kHz)
- **Language**: Rust with async/await
- **Communication**: Unix sockets + tokio
- **Platform**: macOS (aarch64/x86_64)

### Performance

- **Daemon Startup Time**: ~3 seconds (all models loaded)
- **Voice Synthesis Time**: ~100ms (daemon mode)
- **Memory Usage**: ~500MB (all models loaded)
- **File Size**: ~20MB (minimal configuration)

## Development

### Development Environment

```bash
# Nix development environment
nix develop

# Check dependencies
cargo build --bin voicevox-daemon --bin voicevox-say

# Run tests
cargo test

# Functional test
./target/debug/voicevox-daemon --foreground &
./target/debug/voicevox-say "動作テスト"
```

### Architecture Details

**Important Files**:
- `src/lib.rs` - Shared library, VoicevoxCore, IPC protocols
- `src/bin/daemon.rs` - Background daemon, model management
- `src/bin/client.rs` - Lightweight CLI client, primary interface
- `voicevox_core/` - VOICEVOX Core runtime libraries
- `models/*.vvm` - Voice model files (19 models)
- `dict/` - OpenJTalk dictionary

## License

### CLI Tool

MIT License OR Apache License 2.0

### VOICEVOX Core

MIT License
Copyright (c) 2021 Hiroshiba Kazuyuki

### ONNX Runtime

Custom License Terms
Commercial use allowed with attribution required
See: `voicevox_core/onnxruntime/TERMS.txt`

### Usage Notice

**Attribution required when generating audio**:
- "Generated using VOICEVOX" ("VOICEVOX を使用して生成")
- Follow individual character license terms
- Check individual licenses for commercial use

Details: [VOICEVOX Terms of Use](https://voicevox.hiroshiba.jp/term)

## Related Links

- [VOICEVOX](https://voicevox.hiroshiba.jp/)
- [VOICEVOX Core](https://github.com/VOICEVOX/voicevox_core)
- [Nix](https://nixos.org/)
- [XDG Base Directory](https://specifications.freedesktop.org/basedir-spec/basedir-spec-latest.html)

---

ずんだもんと一緒に楽しい TTS ライフを送るのだ！
Enjoy a fun TTS life with Zundamon!
