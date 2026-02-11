# VOICEVOX CLI

[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-black)](https://github.com/usabarashi/voicevox-cli/blob/main/LICENSE)
[![VOICEVOX Core](https://img.shields.io/github/v/release/VOICEVOX/voicevox_core?color=blueviolet&label=voicevox-core)](https://github.com/VOICEVOX/voicevox_core/releases/latest)
[![Rust Version](https://img.shields.io/badge/dynamic/toml?url=https%3A%2F%2Fraw.githubusercontent.com%2Fusabarashi%2Fvoicevox-cli%2Fmain%2Frust-toolchain.toml&query=%24.toolchain.channel&color=D34516&label=rust)](https://github.com/rust-lang/rust/releases)
[![Nixpkgs](https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fraw.githubusercontent.com%2Fusabarashi%2Fvoicevox-cli%2Fmain%2Fflake.lock&query=%24.nodes.nixpkgs.locked.rev&color=5277C3&label=nixpkgs)](https://github.com/NixOS/nixpkgs)
[![CI Status](https://github.com/usabarashi/voicevox-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/usabarashi/voicevox-cli/actions/workflows/ci.yml)

Japanese text-to-speech CLI using [VOICEVOX Core](https://github.com/VOICEVOX/voicevox_core) for Apple Silicon Macs (requires [Nix](https://nixos.org/download.html#nix-install-macos)).

## Quick Start

```bash
nix shell github:usabarashi/voicevox-cli

voicevox-setup                              # Download resources (first time only)
voicevox-say "こんにちは、ずんだもんなのだ"   # Voice synthesis
```

## Usage

```bash
# Voice synthesis (daemon starts automatically)
voicevox-say "こんにちは、ずんだもんなのだ"
voicevox-say --speaker-id 3 "声を変えてみるのだ"
voicevox-say -o output.wav "保存するテキスト"
echo "パイプからの入力" | voicevox-say

# Voice discovery
voicevox-say --list-speakers
voicevox-say --status

# Daemon management
voicevox-daemon --start | --stop | --restart | --status
```

## MCP Server

Enable AI assistants to use VOICEVOX for Japanese speech synthesis.

```bash
voicevox-mcp-server
```

[See detailed MCP documentation](docs/mcp-usage.md)

## Troubleshooting

```bash
voicevox-say --status              # Check installation status
voicevox-setup                     # Reinstall all resources
voicevox-setup --purge             # Remove all local data for a clean reinstall
voicevox-daemon --restart          # Restart daemon
GH_TOKEN=$(gh auth token) voicevox-setup  # Avoid GitHub API rate limits
```

## License

See [LICENSE](LICENSE) for details. Generated audio requires credit "VOICEVOX:[Character Name]" (e.g., "VOICEVOX:ずんだもん"). License terms are displayed during `voicevox-setup`.

Details: [VOICEVOX Terms of Use](https://voicevox.hiroshiba.jp/term)

---

ずんだもんと一緒に楽しい TTS ライフを送るのだ！
