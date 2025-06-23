# VOICEVOX TTS

Production-ready daemon-client TTS tool using VOICEVOX Core

## Features

- **Fast Voice Synthesis**: Background service for instant voice generation
- **macOS say Compatible**: Silent operation like macOS say command
- **99 Voice Styles**: 26 characters including ずんだもん (Zundamon), 四国めたん (Shikoku Metan), 春日部つむぎ (Kasukabe Tsumugi)
- **Instant Voice Synthesis**: Pre-loaded models for immediate voice generation
- **CPU-Only Processing**: macOS optimized (CUDA/DirectML disabled)
- **Zero Configuration**: Automatic setup with smart path discovery

## How It Works

The tool consists of two simple parts:
- **`voicevox-say`**: Main command you use (like macOS `say`)
- **`voicevox-daemon`**: Background service that loads voice models once for speed

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

### From Source

```bash
# Clone and build
git clone https://github.com/usabarashi/voicevox-tts
cd voicevox-tts
nix build

# Or with Cargo (requires VOICEVOX Core libraries)
cargo build --release
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

### Voice Discovery

```bash
# List available voices
voicevox-say -v "?"

# Detailed speaker information
voicevox-say --list-speakers

# Direct speaker ID specification
voicevox-say --speaker-id 3 "ずんだもん（ノーマル）"
```

### Advanced Options

```bash
# Use without daemon (slower but self-contained)
voicevox-say --standalone "独立実行モード"
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

## System Requirements

- **Platform**: macOS (Apple Silicon or Intel)
- **Audio Format**: WAV output
- **Performance**: Near-instant voice synthesis after initial setup

## Contributing

Issues and Pull Requests are welcome! See CLAUDE.md for development details.

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

---

ずんだもんと一緒に楽しい TTS ライフを送るのだ！
Enjoy a fun TTS life with Zundamon!
