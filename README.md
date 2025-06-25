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

# First usage (interactive setup)
voicevox-say "こんにちは、ずんだもんなのだ"
# ↳ Prompts for license acceptance, downloads models, synthesizes speech

# Subsequent usage (instant)
voicevox-say "その後の呼び出しは瞬時なのだ"
```

## Installation

### Try with Nix Shell (Recommended)

```bash
# Try in temporary shell environment (no permanent installation)
nix shell github:usabarashi/voicevox-cli

# First usage triggers interactive setup
voicevox-say "こんにちは、ずんだもんなのだ"
```

### Permanent Installation (Optional)

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

**Note**: Voice models and related components are stored in your user directory (`~/.local/share/voicevox/`) and only need to be downloaded once. Using `nix shell` provides a temporary environment to try the tool without permanent installation.

## Initial Setup

### Interactive First-Run Setup

On first use, VOICEVOX CLI will detect missing voice components and offer to download them with proper license acceptance:

```bash
# First time usage - interactive setup prompt will appear
voicevox-say "こんにちは、ずんだもんなのだ"

# You'll see:
# 🎭 VOICEVOX CLI - First Run Setup
# Voice models and dictionary are required for text-to-speech synthesis.
#
# Would you like to download voice models and dictionary now? [Y/n]: y
# 🔄 Starting voice models and dictionary download...
# Note: This will require accepting VOICEVOX license terms.
#
# 📦 Target directory: ~/.local/share/voicevox
# 🔄 Launching VOICEVOX downloader...
#    Please follow the on-screen instructions to accept license terms.
#    Press Enter when ready to continue...
```

**License Review Process**:
1. **License Display**: Review terms for all voice characters
2. **Navigation**: Use arrow keys to scroll, 'q' to exit
3. **Acceptance**: Type 'y' and press Enter to accept
4. **Download**: Voice models downloaded automatically

The setup process will:
- Show VOICEVOX license terms for all voice characters
- Require your confirmation before downloading
- Download voice models and related components to your computer
- Enable voice synthesis for immediate use

### Manual Setup

If you prefer manual setup or need to reinstall models:

```bash
# Download voice models
voicevox-setup-models

# Or use the downloader directly
voicevox-download --output ~/.local/share/voicevox
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

# Daemon status check
voicevox-say --daemon-status
```

### Voice Discovery

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

### Daemon Management

```bash
# Start daemon manually
voicevox-daemon --start

# Stop daemon
voicevox-daemon --stop

# Check daemon status
voicevox-daemon --status

# Restart daemon
voicevox-daemon --restart

# Development mode (foreground with debugging output)
voicevox-daemon --foreground
```

## Voice Characters

### Available Voices

Voice characters are automatically detected from your downloaded voice models.

**Find Available Voices:**
```bash
# See available voice models
voicevox-say --list-models

# See detailed voice information
voicevox-say --list-speakers
```

**Use Different Voices:**
```bash
# Use a specific voice (by speaker ID)
voicevox-say --speaker-id 3 "声を変えてみるのだ"

# Use a voice model (by model number)
voicevox-say --model 3 "違うモデルで試すのだ"
```

### Popular Characters

When you download voice models, you get 26+ characters including:
- **ずんだもん** - Cheerful and energetic character
- **四国めたん** - Sweet and gentle character
- **春日部つむぎ**, **雨晴はう**, **波音リツ**, **九州そら**, **もち子さん**, and many more

**Note**: Use `--list-speakers` to see all available voices on your system.

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

**Download Problems**:
```bash
# Check if voice models are installed
voicevox-say --list-models

# Reinstall voice models if needed
voicevox-setup-models
```

**Voice Synthesis Issues**:
```bash
# Check system status
voicevox-say --daemon-status

# Check daemon status with voicevox-daemon
voicevox-daemon --status

# List available voices
voicevox-say --list-speakers

# Restart daemon if needed
voicevox-daemon --restart
```

## Contributing

Issues and Pull Requests are welcome!

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
