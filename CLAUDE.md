# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

VOICEVOX CLI (`voicevox-cli`) - A command-line text-to-speech tool using VOICEVOX Core 0.16.0. Provides macOS `say` command-compatible interface for Japanese TTS.

**Architecture**: Daemon-client model via Unix sockets
- Client (`voicevox-say`): Lightweight CLI interface
- Daemon (`voicevox-daemon`): Background service handling synthesis

**Key Implementation**:
- **No Memory Caching**: Models are loaded for each synthesis request and immediately unloaded
- **Dynamic Voice Detection**: Discovers available voices by scanning VVM files at startup
- **Zero-Copy Transfer**: Uses file descriptor passing for efficient audio streaming
- **macOS Compatibility**: Silent operation on success, errors to stderr only

## Current Model Management

```
User Request → Load Model → Synthesize → Unload Model → Return Audio
```

Every synthesis request follows this pattern:
1. Receive synthesis request with style ID
2. Load the required VVM model file
3. Perform voice synthesis
4. Unload the model to free memory
5. Return audio data to client

**No persistent model storage** - daemon memory usage remains minimal between requests.

## File Structure

```
src/
├── lib.rs              # Shared library & IPC protocols
├── bin/
│   ├── client.rs       # CLI client (voicevox-say)
│   └── daemon.rs       # Background daemon
├── core.rs             # VOICEVOX Core wrapper
├── voice.rs            # Dynamic voice detection
├── paths.rs            # XDG-compliant paths
├── config.rs           # Simple daemon configuration
├── ipc.rs              # IPC message definitions
├── client/             # Client functionality
│   ├── download.rs     # Model downloader
│   ├── daemon_client.rs# Unix socket client
│   └── audio.rs        # Audio playback
└── daemon/             # Server functionality
    ├── server.rs       # Request handler (load/unload per request)
    ├── fd_passing.rs   # Zero-copy support
    └── process.rs      # Process management
```

## Configuration

Minimal configuration in `~/.config/voicevox/config.toml`:
```toml
[daemon]
socket_path = "/custom/path"   # Optional
startup_timeout = 10           # Seconds
debug = false                  # Debug logging
```

No memory management settings - models are never cached.

## Build and Usage

```bash
# Build with Nix (recommended)
nix build

# Start daemon
./result/bin/voicevox-daemon --start

# Use TTS (loads and unloads model per request)
./result/bin/voicevox-say "こんにちはなのだ"

# Stop daemon
./result/bin/voicevox-daemon --stop
```

## Testing

```bash
# Watch model loading/unloading
./result/bin/voicevox-daemon --foreground

# In another terminal
./result/bin/voicevox-say "テストなのだ"
# Output shows: "✓ Loaded model X" → synthesis → "✓ Unloaded model X"
```