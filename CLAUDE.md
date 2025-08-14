# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

VOICEVOX CLI - Command-line text-to-speech tool using VOICEVOX Core 0.16.0.

## Architecture

- Client-server model with Unix socket IPC
- Dynamic VVM model loading per request
- MCP server for AI assistant integration

## Structure

```
src/
├── bin/
│   ├── client.rs        # voicevox-say
│   ├── daemon.rs        # voicevox-daemon
│   └── mcp_server.rs    # voicevox-mcp-server
├── client/              # Client implementation
├── daemon/              # Daemon implementation
├── mcp/                 # MCP implementation
│   ├── server.rs        # JSON-RPC server
│   ├── handlers.rs      # Tool handlers
│   ├── tools.rs         # Tool definitions
│   └── types.rs         # Protocol types
├── synthesis/           # Audio synthesis
├── core.rs              # VOICEVOX Core wrapper
├── voice.rs             # Voice management
├── paths.rs             # Path utilities
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