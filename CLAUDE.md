# CLAUDE.md

VOICEVOX CLI - Command-line text-to-speech tool using VOICEVOX Core.

## Architecture

Client-server model with three main binaries:
- `voicevox-say`: CLI client for text-to-speech synthesis
- `voicevox-daemon`: Background server handling VOICEVOX Core operations
- `voicevox-mcp-server`: MCP integration for AI assistants

Key design principles:
- Unix socket IPC for client-daemon communication
- Dynamic VVM model loading (no persistent memory caching)
- Automatic daemon lifecycle management
- Transparent auto-startup for seamless user experience
- Platform: macOS Apple Silicon (aarch64-darwin)

## Implementation Details

### Binary Modules (`src/bin/`)
- `client.rs`: Main CLI interface implementing `voicevox-say` command
- `daemon.rs`: Background server handling VOICEVOX Core operations
- `mcp_server.rs`: MCP protocol server for AI assistant integration

### Library Modules (`src/`)
- `core.rs`: VOICEVOX Core FFI bindings (`VoicevoxCore` struct, `CoreSynthesis` trait)
- `ipc.rs`: IPC protocol types (`DaemonRequest`/`DaemonResponse` enums, bincode serialization)
- `paths.rs`: XDG Base Directory path resolution for socket, models, dictionary, ONNX Runtime
- `config.rs`: TOML configuration (`~/.config/voicevox-cli/config.toml`, `TextSplitterConfig`)
- `setup.rs`: Initial setup orchestration (automatic and manual setup flows)
- `voice.rs`: Voice model scanning, style-to-model mapping, voice name resolution
- `client/`: Client-side logic (see submodules below)
- `daemon/`: Server implementation (see submodules below)
- `synthesis/`: Streaming synthesis engine (see submodules below)
- `mcp/`: MCP protocol server (see submodules below)

### Client Submodules (`src/client/`)
- `daemon_client.rs`: Unix socket client (`DaemonClient`) with auto-start, exponential backoff retry, `LengthDelimitedCodec` framing
- `audio.rs`: Dual audio playback (rodio primary, `afplay`/`play` system fallback; `VOICEVOX_LOW_LATENCY` for rodio-first mode)
- `download.rs`: Resource download management with retry logic
- `input.rs`: Text input handling (positional argument, `--input-file`, stdin)

### Daemon Submodules (`src/daemon/`)
- `server.rs`: `DaemonState` with dynamic style-to-model mapping, per-request model load/unload, `run_daemon` loop
- `process.rs`: Duplicate daemon prevention via `pgrep`, stale socket detection, PID discovery

### Synthesis Submodules (`src/synthesis/`)
- `streaming.rs`: `StreamingSynthesizer` for long text with `TextSplitter` (configurable delimiters/max length), concurrent segment synthesis and rodio `Sink` playback

### MCP Submodules (`src/mcp/`)
- `server.rs`: JSON-RPC 2.0 server over stdin/stdout (MCP protocol version `2025-03-26`)
- `tools.rs`: Tool definitions (`text_to_speech`, `list_voice_styles` with JSON Schema)
- `handlers.rs`: Tool request handlers (streaming path via `StreamingSynthesizer`, daemon path via `DaemonClient`)
- `types.rs`: MCP type definitions (`JsonRpcRequest`, `JsonRpcResponse`, `ToolDefinition`, `ToolCallResult`)

### Daemon Auto-Start Mechanism
1. Client attempts Unix socket connection
2. On connection failure, checks for available VVM models
3. Automatically spawns daemon with `--start --detach`
4. Retries connection with exponential backoff
5. Provides user feedback during startup process

### Synthesis Modes
- **Direct mode**: Single synthesis request sent to daemon, audio played through client
- **Streaming mode**: Long text segmented and processed with concurrent synthesis and playback  
- **MCP mode**: Streaming path (default, `StreamingSynthesizer` with direct VOICEVOX Core) or daemon path (`DaemonClient`)

## Command Interface

### voicevox-say
```bash
voicevox-say "テキスト"              # Basic text-to-speech with auto daemon startup
voicevox-say -v NAME "テキスト"      # Specify voice by name (-v ? to list)
voicevox-say -r 1.5 "テキスト"       # Adjust speech rate (0.5-2.0)
voicevox-say -o output.wav "テキスト" # Save to audio file
voicevox-say -f input.txt            # Read from file
echo "テキスト" | voicevox-say -f -  # Read from stdin
voicevox-say -q -o out.wav "テキスト" # Save without playback
voicevox-say -m 3 "テキスト"         # Specify model number (3.vvm)
voicevox-say --speaker-id 3 "テキスト" # Direct style ID
voicevox-say --list-speakers         # List all speakers and styles
voicevox-say --list-models           # List available VVM models
voicevox-say --status                # Show installation status
voicevox-say -S /path/to/sock "テキスト" # Custom socket path
```

### voicevox-daemon
```bash
voicevox-daemon --start              # Start daemon (shows usage if no flags)
voicevox-daemon --start --detach     # Start as background process
voicevox-daemon --foreground         # Run in foreground (development)
voicevox-daemon --stop               # Stop running daemon
voicevox-daemon --status             # Check daemon status
voicevox-daemon --restart            # Stop then start
voicevox-daemon -s /path/to/sock     # Custom socket path
```

### voicevox-mcp-server
```bash
voicevox-mcp-server                  # Start MCP server (stdin/stdout JSON-RPC)
```

## MCP Integration

### Available Tools
- `text_to_speech`: Convert Japanese text to speech with configurable voice style, rate, and streaming
- `list_voice_styles`: Query available voice styles with optional filtering by speaker or style name

### Instruction System
The MCP server dynamically loads behavior instructions to guide AI assistant interactions:

1. **Environment variable**: `VOICEVOX_MCP_INSTRUCTIONS` pointing to custom file
2. **Executable directory**: `INSTRUCTIONS.md` bundled with binary
3. **Current directory**: `INSTRUCTIONS.md` for development

**Configuration example:**
```bash
export VOICEVOX_MCP_INSTRUCTIONS=/path/to/custom/instructions.md
voicevox-mcp-server
```

Server operates normally without instruction files. Default behavior defined in [INSTRUCTIONS.md](INSTRUCTIONS.md).

## Configuration

User config path: `~/.config/voicevox-cli/config.toml`

```toml
[text_splitter]
delimiters = ["。", "！", "？", "．", "\n"]
max_length = 100
```

Used by `StreamingSynthesizer` for text segmentation in streaming mode.

## Environment Variables

| Variable | Purpose |
|---|---|
| `VOICEVOX_SOCKET_PATH` | Custom Unix socket path |
| `VOICEVOX_MODELS_DIR` | Custom models directory (aliases: `VOICEVOX_MODEL_DIR`, `VOICEVOX_MODELS_PATH`, `VOICEVOX_MODEL_PATH`, `VOICEVOX_MODELS`) |
| `VOICEVOX_OPENJTALK_DICT` | Custom OpenJTalk dictionary path |
| `VOICEVOX_MCP_INSTRUCTIONS` | Custom MCP instruction file path |
| `VOICEVOX_LOW_LATENCY` | Enable rodio low-latency audio playback |
| `ORT_DYLIB_PATH` | Custom ONNX Runtime library path |

Default paths follow XDG Base Directory specification (models: `~/.local/share/voicevox`, socket: `$XDG_RUNTIME_DIR/voicevox-daemon.sock`).

## Build System

### Cargo Features
| Feature | Dependencies | Purpose |
|---|---|---|
| `simd` | rayon | Parallel model scanning |
| `fast-strings` | compact_str | Memory-efficient strings |
| `small-vectors` | smallvec | Stack-allocated small vectors |
| `performance` | All above + mimalloc | Full optimization bundle |

### Nix Flake
```bash
nix build                    # Build package
nix run                      # Run voicevox-say
nix flake check              # Run checks (formatting, clippy, build)
nix develop                  # Enter development shell
```

Platform: macOS Apple Silicon (aarch64-darwin) only.