# VOICEVOX CLI

Zero-configuration Japanese text-to-speech using VOICEVOX Core for Apple Silicon Macs

## Features

- **🚀 Zero Configuration**: Install and use immediately
- **🎭 26+ Voice Characters**: Automatic detection of available voice models
- **⚡ Instant Response**: Fast voice synthesis after initial setup
- **🔇 Silent Operation**: Works like macOS `say` command
- **💾 Lightweight**: Small download size, easy installation
- **👤 Interactive Setup**: Guided setup with license acceptance on first use

## Quick Start

```bash
# Try with Nix (temporary shell)
nix shell github:usabarashi/voicevox-cli

# First usage (triggers interactive setup)
voicevox-say "こんにちは、ずんだもんなのだ"

# Subsequent usage (instant)
voicevox-say "その後の呼び出しは瞬時なのだ"
```

**Note**: First use will prompt for license acceptance and download voice models (~200MB).

## Installation

### Permanent Installation

```bash
# Install permanently to your profile
nix profile install github:usabarashi/voicevox-cli
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

**Note**: Voice models and related components are stored in your user directory (`~/.local/share/voicevox/`) and only need to be downloaded once.

## Initial Setup

### Interactive First-Run Setup

On first use, VOICEVOX CLI will detect missing components and guide you through setup:

```bash
voicevox-say "こんにちは、ずんだもんなのだ"
```

**Setup Process**:
1. **License Review**: Terms for all 26+ voice characters
2. **Download**: Voice models (~200MB) to `~/.local/share/voicevox/`
3. **Ready**: Immediate voice synthesis capability

### Manual Setup

If you prefer manual setup or need to reinstall models:

```bash
# Download voice models
voicevox-setup-models
```

## Usage

### Basic Usage

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

# Check information
voicevox-say --status
```


### Daemon Management

```bash
# Basic daemon control
voicevox-daemon --start    # Start daemon (automatically detached)
voicevox-daemon --stop     # Stop daemon
voicevox-daemon --status   # Check daemon status
voicevox-daemon --restart  # Restart daemon

# Development options
voicevox-daemon --foreground  # Run in foreground (development mode)
voicevox-daemon --socket-path /custom/path/daemon.sock --start  # Custom socket
```

## Voice Management

### Available Voices

Voice characters (26+) are automatically detected from downloaded models:
- **ずんだもん** - Cheerful and energetic character
- **四国めたん** - Sweet and gentle character  
- **春日部つむぎ**, **雨晴はう**, **波音リツ**, **九州そら**, **もち子さん**, and many more

### Voice Commands

```bash
# Discover available voices
voicevox-say --list-models        # Show installed voice model files
voicevox-say --list-speakers      # Show detailed voice information
voicevox-say --status             # Check installation status

# Use different voices
voicevox-say --speaker-id 3 "声を変えてみるのだ"  # By speaker ID
voicevox-say --model 3 "違うモデルで試すのだ"      # By model number
```

## System Requirements

- **Platform**: macOS Apple Silicon (M1, M2, M3, etc.) only
- **Audio**: WAV file output and system audio playback
- **Storage**: Voice models stored in your user directory
- **Network**: Required for initial voice model download

**Note**: This version is designed specifically for Apple Silicon Macs. Intel Mac support is not included.

## Troubleshooting

### Common Issues

**License Setup**:
- Use arrow keys or Space to scroll through license terms
- Press 'q' to exit the license viewer
- Type 'y' and press Enter to accept terms

**Download/Model Issues**:
```bash
voicevox-say --status              # Check installation status
voicevox-setup-models              # Reinstall voice models
```

**Voice Synthesis Issues**:
```bash
voicevox-daemon --status           # Check daemon status
voicevox-daemon --restart          # Restart daemon
```

## License

This project includes multiple components with different licenses. See [LICENSE](LICENSE) for complete details.

**Quick Summary for Users:**
- ✅ Commercial and non-commercial use of generated audio allowed
- ⚠️ **Required**: Credit "VOICEVOX:[Character Name]" in your work (e.g., "VOICEVOX:ずんだもん")
- ❌ No redistribution of VOICEVOX software without permission
- 📋 Individual character license terms apply (displayed during setup)

**Important**: You'll need to accept license terms for all voice characters during first-run setup.

Details: [VOICEVOX Terms of Use](https://voicevox.hiroshiba.jp/term)

## Related Links

- [VOICEVOX](https://voicevox.hiroshiba.jp/)
- [VOICEVOX Core](https://github.com/VOICEVOX/voicevox_core)
- [Nix](https://nixos.org/)

---

ずんだもんと一緒に楽しい TTS ライフを送るのだ！
Enjoy a fun TTS life with Zundamon!
