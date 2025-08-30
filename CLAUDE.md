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

## Implementation Details

### Binary Modules (`src/bin/`)
- `client.rs`: Main CLI interface (`voicevox-say`)
- `daemon.rs`: Background daemon server
- `mcp_server.rs`: MCP protocol server

### Core Modules (`src/`)
- `client/`: Client logic, audio playback, model management
- `daemon/`: Server implementation, request handling
- `core/`: VOICEVOX Core FFI bindings
- `ipc/`: Inter-process communication protocol
- `synthesis/`: Streaming synthesis for long text
- `mcp/`: MCP tools and handlers

### Daemon Auto-Start Mechanism
1. Client attempts Unix socket connection
2. On connection failure, checks for available VVM models
3. Automatically spawns daemon with `--start --detach`
4. Retries connection with exponential backoff
5. Provides user feedback during startup process

### Synthesis Modes
- **Direct mode**: Single request to daemon, audio playback via client
- **Streaming mode**: Text segmentation, concurrent synthesis and playback
- **MCP mode**: Two paths - streaming (default) or daemon-based synthesis

## Commands

```bash
voicevox-say "テキスト"              # Text-to-speech with auto-daemon
voicevox-daemon --start             # Manual daemon startup
voicevox-mcp-server                 # MCP server for AI integration
```

## MCP Tools

- `text_to_speech`: Synthesize with style ID, rate, streaming mode
- `list_voice_styles`: Query available speakers with filtering