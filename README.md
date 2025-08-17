# VOICEVOX CLI

[![Dependency Status](https://img.shields.io/badge/deps-automated-green)](https://github.com/usabarashi/voicevox-cli/blob/main/.github/dependabot.yml)
[![CI Status](https://github.com/usabarashi/voicevox-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/usabarashi/voicevox-cli/actions/workflows/ci.yml)
[![Last Update](https://img.shields.io/badge/updated-monthly-blue)](https://github.com/usabarashi/voicevox-cli/blob/main/.github/workflows/update-flake.yml)

Zero-configuration Japanese text-to-speech using VOICEVOX Core for Apple Silicon Macs

## Features

- **🚀 Zero Configuration**: Install and use immediately
- **🎭 26+ Voice Characters**: Automatic detection of available voice models
- **⚡ Instant Response**: Fast voice synthesis after initial setup
- **🔇 Silent Operation**: Works like macOS `say` command
- **💾 Lightweight**: Small download size, easy installation
- **👤 Interactive Setup**: Guided setup with license acceptance on first use

## Quick Start

**Prerequisites**: [Nix package manager for macOS](https://nixos.org/download.html#nix-install-macos) must be installed.

```bash
# Try temporarily
nix shell github:usabarashi/voicevox-cli

# Or install permanently
nix profile install github:usabarashi/voicevox-cli

# First usage (triggers interactive setup)
voicevox-say "こんにちは、ずんだもんなのだ"
```

**Note**: First use will prompt for license acceptance and download voice models (~200MB).

## Installation

### Development

```bash
# Clone repository
git clone https://github.com/usabarashi/voicevox-cli
cd voicevox-cli

# Enter development shell
nix develop

# Build and test
nix build
nix run . -- "テストメッセージなのだ"
```

**Note**: Voice models are stored in your user directory (`~/.local/share/voicevox/`) and only need to be downloaded once. This project uses Nix's `nixos-unstable` channel for package dependencies, but is designed exclusively for macOS Apple Silicon (not NixOS).

### Manual Model Setup

For manual setup or model reinstallation:

```bash
# Download voice models manually
voicevox-setup-models
```

## Usage

### Basic Usage

```bash
# Voice synthesis with automatic daemon startup
voicevox-say "こんにちは、ずんだもんなのだ"

# Voice selection by model or speaker ID
voicevox-say --model 3 "モデル3の音声なのだ"
voicevox-say --speaker-id 3 "声を変えてみるのだ"

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

### Voice Discovery

```bash
# Discover available voices
voicevox-say --list-models        # Show installed voice model files
voicevox-say --list-speakers      # Show detailed voice information
voicevox-say --status             # Check installation status
```

## System Requirements

- **Platform**: macOS Apple Silicon (M1, M2, M3, etc.) only
- **Package Manager**: [Nix package manager for macOS](https://nixos.org/download.html#nix-install-macos) required
- **Audio**: WAV file output and system audio playback
- **Storage**: Voice models stored in your user directory (`~/.local/share/voicevox/`)
- **Network**: Required for initial voice model download

**Important**: This version is designed exclusively for Apple Silicon Macs with Nix package manager. NixOS, Linux, Intel Mac, and Windows are not supported.

## MCP Server (AI Assistant Integration)

Enable AI assistants to use VOICEVOX for Japanese speech synthesis:

```bash
# Start MCP server
voicevox-mcp-server  # Communicates via stdin/stdout
```

Available tools:
- `text_to_speech`: Convert Japanese text to speech (TTS)
- `list_voice_styles`: List available voice styles

[See detailed MCP documentation](docs/mcp-usage.md)

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

## Dependency Management

- **Automated Updates**: Monthly dependency checks via Dependabot and Nix flake workflows
- **Review Process**: All updates reviewed through CODEOWNERS enforcement
- **Rollback Protection**: Automatic rollback on test failures
- **Monitoring**: Status badges show current automation health

## Related Links

- [VOICEVOX](https://voicevox.hiroshiba.jp/)
- [VOICEVOX Core](https://github.com/VOICEVOX/voicevox_core)
- [Nix Package Manager for macOS](https://nixos.org/download.html#nix-install-macos)

---

ずんだもんと一緒に楽しい TTS ライフを送るのだ！
Enjoy a fun TTS life with Zundamon!
