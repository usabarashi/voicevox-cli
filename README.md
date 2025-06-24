# VOICEVOX CLI

Zero-configuration Japanese text-to-speech using VOICEVOX Core for Apple Silicon Macs

## Features

- **🚀 Zero Configuration**: `nix profile install` → instant TTS
- **🎭 Dynamic Voice Detection**: 26+ characters automatically discovered from VVM models
- **⚡ Instant Response**: Shared background daemon for immediate synthesis
- **🔇 Silent Operation**: macOS `say` compatible (no output unless error)
- **📦 Nix Pure**: Reproducible builds with fixed SHA256 dependencies
- **👤 Interactive Setup**: Voice models with proper license acceptance on first use

## Quick Start

```bash
# Install with Nix
nix profile install github:usabarashi/voicevox-cli

# First usage (interactive setup)
voicevox-say "こんにちは、ずんだもんなのだ"
# ↳ Prompts for license acceptance, downloads models, synthesizes speech

# Subsequent usage (instant)
voicevox-say "その後の呼び出しは瞬時なのだ"
```

## Installation

### Direct Install (Recommended)

```bash
# Install directly with Nix
nix profile install github:usabarashi/voicevox-cli

# First usage triggers interactive setup
voicevox-say "こんにちは、ずんだもんなのだ"
```

### Development

```bash
# Clone and build
git clone https://github.com/usabarashi/voicevox-cli
cd voicevox-cli

# Nix development
nix develop
nix build

# Test directly
nix run . -- "テストメッセージなのだ"
```

### Direct Install

```bash
# Install directly via Nix
nix profile install github:usabarashi/voicevox-cli

# Use in shell environment
nix shell github:usabarashi/voicevox-cli
```

**Note**: Nix builds use fixed SHA256 hashes for reproducible dependency management:
- VOICEVOX Core 0.16.0 libraries
- ONNX Runtime 1.17.3
- OpenJTalk dictionary
- VOICEVOX Core downloader tool

Voice models are managed in user directories (`~/.local/share/voicevox/models/`) for mutable storage.

## Initial Setup

### Interactive First-Run Setup (Current Implementation)

On first use, VOICEVOX CLI will detect missing voice models and offer to download them with proper license acceptance:

```bash
# First time usage - interactive setup prompt will appear
voicevox-say "こんにちは、ずんだもんなのだ"

# You'll see:
# 🎭 VOICEVOX CLI - First Run Setup
# Voice models are required for text-to-speech synthesis.
#
# Would you like to download voice models now? [Y/n]: y
# 🔄 Starting voice model download...
# Note: This will require accepting VOICEVOX license terms.
#
# 📦 Target directory: ~/.local/share/voicevox/models
# 🔄 Launching VOICEVOX downloader...
#    Please follow the on-screen instructions to accept license terms.
#    Press Enter when ready to continue...
```

**License Review Process**:
1. **Complete License Display**: Interactive pager shows detailed terms for all 26+ voice characters
2. **Manual Navigation**: Use arrow keys to scroll, 'q' to exit license viewer
3. **Explicit Acceptance**: Type 'y' and press Enter to accept terms
4. **Download Progress**: Models downloaded after license acceptance (~1.1GB)

The setup process will:
- Display comprehensive VOICEVOX license terms for all voice characters
- Require manual user confirmation (no automated acceptance)
- Download all official voice models after license acceptance
- Install models to `~/.local/share/voicevox/models/`
- Enable immediate voice synthesis for subsequent usage

### Manual Setup

If you prefer manual setup or need to reinstall models:

```bash
# Download essential voice models (requires license acceptance)
voicevox-setup-models

# Or manually using the downloader (interactive license acceptance)
voicevox-download --output ~/.local/share/voicevox/models

# Note: All manual setup methods still require interactive license acceptance
```

## Usage

### Daemon-Client Mode (Recommended)

```bash
# Voice synthesis with automatic daemon startup
voicevox-say "こんにちは、ずんだもんなのだ"

# Voice selection by model or speaker ID
voicevox-say --model 3 "モデル3の音声なのだ"
voicevox-say --speaker-id 1 "スピーカーID1の音声なのだ"

# File output
voicevox-say -o output.wav "保存するテキスト"

# From stdin
echo "パイプからの入力" | voicevox-say

# Daemon status check
voicevox-say --daemon-status
```

### Voice Discovery (Dynamic Detection)

```bash
# List available VVM models
voicevox-say --list-models

# Detailed speaker information from loaded models
voicevox-say --list-speakers

# Check system status and available updates
voicevox-say --check-updates

# Use specific model or speaker ID
voicevox-say --model 3 "モデル3の音声"
voicevox-say --speaker-id 3 "スピーカーID3の音声"
```

### Advanced Options

```bash
# Use without daemon (slower but self-contained)
voicevox-say --standalone "独立実行モード"
```

## Voice Characters (Dynamic Detection)

### Dynamic Voice Management

This system uses **dynamic voice detection** - no hardcoded voice names. Voice characters are automatically discovered from available VVM model files.

**Discovery Commands:**
```bash
# See all available models
voicevox-say --list-models

# Get detailed speaker information
voicevox-say --list-speakers
```

**Usage Methods:**
```bash
# Method 1: Direct speaker ID (most reliable)
voicevox-say --speaker-id 3 "スピーカーID指定"

# Method 2: Model selection (uses first available style)
voicevox-say --model 3 "モデル番号指定"
```

### Character Examples (Typical Installation)

When models are downloaded, you typically get 26+ characters including:
- **ずんだもん (Zundamon)** - Multiple emotional variations
- **四国めたん (Shikoku Metan)** - Multiple emotional variations  
- **春日部つむぎ (Kasukabe Tsumugi)**, **雨晴はう (Amehare Hau)**, **波音リツ (Namine Ritsu)**, **九州そら (Kyushu Sora)**, **もち子さん (Mochiko-san)**, and many more

**Note**: Exact voice IDs and available characters depend on your downloaded models. Use `--list-speakers` for definitive information.

## System Requirements

- **Platform**: macOS Apple Silicon (aarch64) only
- **Architecture**: arm64/aarch64 optimized binaries
- **Audio Format**: WAV output with rodio/afplay playback
- **Performance**: Near-instant voice synthesis after initial setup
- **Storage**: ~1.1GB for all voice models in `~/.local/share/voicevox/models/`
- **Network**: Required for initial model download and license acceptance

**Note**: This build is specifically optimized for Apple Silicon Macs (M1, M2, M3, etc.). Intel Mac support is not included in this distribution.

## Troubleshooting

### First-Run Setup Issues

**License Pager Navigation**:
- Use ↑↓ arrow keys or Space to scroll through license terms
- Press 'q' to exit the license viewer
- Type 'y' and press Enter to accept terms
- If pager crashes, restart and try again

**Model Download Fails**:
```bash
# Check if models directory exists
ls ~/.local/share/voicevox/models/

# Manual cleanup and retry
rm -rf ~/.local/share/voicevox/models/
voicevox-say "再試行テストなのだ"
```

**Daemon Connection Issues**:
```bash
# Check daemon status
voicevox-say --daemon-status

# Force standalone mode
voicevox-say --standalone "スタンドアロンテスト"
```

## Contributing

Issues and Pull Requests are welcome! See CLAUDE.md for development details.

## License

This project includes multiple components with different licenses. See [LICENSE](LICENSE) for complete details.

**Quick Summary for Users:**
- ✅ Commercial and non-commercial use of generated audio allowed
- ⚠️ **Required**: Credit "VOICEVOX:[Character Name]" in your work (e.g., "VOICEVOX:ずんだもん")
- ❌ No redistribution of VOICEVOX software without permission
- 📋 Individual character license terms apply (displayed during setup)

**Important**: License terms for all 26+ voice characters are displayed interactively during first-run setup and must be manually accepted.

Details: [VOICEVOX Terms of Use](https://voicevox.hiroshiba.jp/term)

## Related Links

- [VOICEVOX](https://voicevox.hiroshiba.jp/)
- [VOICEVOX Core](https://github.com/VOICEVOX/voicevox_core)
- [Nix](https://nixos.org/)

---

ずんだもんと一緒に楽しい TTS ライフを送るのだ！
Enjoy a fun TTS life with Zundamon!
