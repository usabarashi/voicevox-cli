# VOICEVOX CLI

[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-black)](https://github.com/usabarashi/voicevox-cli/blob/main/LICENSE)
[![VOICEVOX Core](https://img.shields.io/github/v/release/VOICEVOX/voicevox_core?color=blueviolet&label=voicevox-core)](https://github.com/VOICEVOX/voicevox_core/releases/latest)
[![Rust Version](https://img.shields.io/badge/dynamic/toml?url=https%3A%2F%2Fraw.githubusercontent.com%2Fusabarashi%2Fvoicevox-cli%2Fmain%2Frust-toolchain.toml&query=%24.toolchain.channel&color=D34516&label=rust)](https://github.com/rust-lang/rust/releases)
[![Nixpkgs](https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fraw.githubusercontent.com%2Fusabarashi%2Fvoicevox-cli%2Fmain%2Fflake.lock&query=%24.nodes.nixpkgs.locked.rev&color=5277C3&label=nixpkgs)](https://github.com/NixOS/nixpkgs)
[![CI Status](https://github.com/usabarashi/voicevox-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/usabarashi/voicevox-cli/actions/workflows/ci.yml)

Japanese text-to-speech using VOICEVOX Core for Apple Silicon Macs

## Features

- **Easy Setup**: Install with Nix, then run `voicevox-setup` for resources
- **26+ Voice Characters**: Automatic detection of available voice models
- **Instant Response**: Fast voice synthesis after initial setup
- **Silent Operation**: Works like macOS `say` command
- **Lightweight**: Small download size, easy installation

## Quick Start

**Prerequisites**: [Nix package manager for macOS](https://nixos.org/download.html#nix-install-macos) must be installed.

```bash
# Try temporarily
nix shell github:usabarashi/voicevox-cli

# Or install permanently
nix profile install github:usabarashi/voicevox-cli

# Setup required resources first
voicevox-setup  # Download all required resources

# Then use voice synthesis
voicevox-say "こんにちは、ずんだもんなのだ"
```

**Note**: `voicevox-setup` downloads required resources. `voicevox-say` requires setup to be completed first.

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

### Manual Resource Setup

For manual setup or resource reinstallation:

```bash
# Download all required resources manually
voicevox-setup
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

Enable AI assistants to use VOICEVOX for Japanese speech synthesis.

```bash
voicevox-mcp-server  # Start MCP server
```

[See detailed MCP documentation](docs/mcp-usage.md)

## Troubleshooting

### Common Issues

**Resource Setup**:
- Run `voicevox-setup` to download required resources
- Wait for download completion
- Resources include voice models, ONNX Runtime, and OpenJTalk dictionary

**Download/Model Issues**:
```bash
voicevox-say --status              # Check installation status
voicevox-setup                     # Reinstall all resources
```

**Voice Synthesis Issues**:
```bash
voicevox-daemon --restart          # Restart daemon
```

## License

This project includes multiple components with different licenses. See [LICENSE](LICENSE) for complete details.

**Quick Summary for Users:**
- Commercial and non-commercial use of generated audio allowed
- **Required**: Credit "VOICEVOX:[Character Name]" in your work (e.g., "VOICEVOX:ずんだもん")
- No redistribution of VOICEVOX software without permission
- Individual character license terms apply (displayed during setup)

**Important**: You'll need to accept license terms for all voice characters during first-run setup.

Details: [VOICEVOX Terms of Use](https://voicevox.hiroshiba.jp/term)

## Related Links

- [VOICEVOX](https://voicevox.hiroshiba.jp/)
- [VOICEVOX Core](https://github.com/VOICEVOX/voicevox_core)
- [Nix Package Manager for macOS](https://nixos.org/download.html#nix-install-macos)

---

ずんだもんと一緒に楽しい TTS ライフを送るのだ！
Enjoy a fun TTS life with Zundamon!
