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
- `client.rs`: Main CLI interface implementing `voicevox-say` command
- `daemon.rs`: Background server handling VOICEVOX Core operations
- `mcp_server.rs`: MCP protocol server for AI assistant integration

### Core Modules (`src/`)
- `client/`: Client-side logic including audio playback and model management
- `daemon/`: Server implementation with request handling and lifecycle management
- `core/`: VOICEVOX Core FFI bindings and voice synthesis interface
- `ipc/`: Inter-process communication protocol for Unix socket messaging
- `synthesis/`: Streaming synthesis engine for processing long text segments
- `mcp/`: MCP protocol tools and request handlers

### Daemon Auto-Start Mechanism
1. Client attempts Unix socket connection
2. On connection failure, checks for available VVM models
3. Automatically spawns daemon with `--start --detach`
4. Retries connection with exponential backoff
5. Provides user feedback during startup process

### Synthesis Modes
- **Direct mode**: Single synthesis request sent to daemon, audio played through client
- **Streaming mode**: Long text segmented and processed with concurrent synthesis and playback  
- **MCP mode**: Dual-path operation supporting both streaming (default) and daemon-based synthesis

## Command Interface

```bash
voicevox-say "テキスト"              # Text-to-speech with automatic daemon startup
voicevox-daemon --start             # Manual daemon startup for persistent operation
voicevox-mcp-server                 # MCP protocol server for AI assistant integration
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