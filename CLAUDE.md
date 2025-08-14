# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

VOICEVOX CLI - Command-line text-to-speech tool using VOICEVOX Core 0.16.0.

## Architecture

- Client-server model with Unix socket IPC
- Dynamic VVM model loading per request
- MCP server for AI assistant integration
- Automatic daemon startup with exponential backoff polling (100ms-1s, max 10 retries)

## Structure

```
src/
├── lib.rs               # Library entry point
├── bin/
│   ├── client.rs        # voicevox-say
│   ├── daemon.rs        # voicevox-daemon
│   └── mcp_server.rs    # voicevox-mcp-server
├── client/              # Client implementation
│   ├── mod.rs           # Module definitions
│   ├── audio.rs         # Audio playback
│   ├── daemon_client.rs # Daemon client connection
│   ├── download.rs      # Model downloader
│   └── input.rs         # Input handling
├── daemon/              # Daemon implementation
│   ├── mod.rs           # Module definitions
│   ├── server.rs        # Unix socket server
│   └── process.rs       # Process management
├── mcp/                 # MCP implementation
│   ├── mod.rs           # Module definitions
│   ├── server.rs        # JSON-RPC server
│   ├── handlers.rs      # Tool handlers
│   ├── tools.rs         # Tool definitions
│   └── types.rs         # Protocol types
├── synthesis/           # Audio synthesis
│   ├── mod.rs           # Module definitions
│   └── streaming.rs     # Streaming synthesis
├── core/                # Core functionality
├── ipc/                 # IPC utilities
├── config.rs            # Configuration
├── core.rs              # VOICEVOX Core wrapper
├── voice.rs             # Voice management
├── paths.rs             # Path utilities
├── setup.rs             # Setup utilities
└── ipc.rs               # IPC protocol
```

## MCP Implementation

Protocol: Model Context Protocol 2025-03-26 over JSON-RPC 2.0 via stdio

Methods:
- `initialize`: Server initialization
- `notifications/initialized`: Client ready notification
- `tools/list`: List available tools
- `tools/call`: Execute tool

Tools:
- `text_to_speech`: Synthesize Japanese text with voice style ID
- `list_voice_styles`: List available voice styles with optional filtering

## Usage

```bash
voicevox-daemon --start
voicevox-say "テキスト"
voicevox-say --speaker-id 3 "テキスト"
voicevox-mcp-server
```