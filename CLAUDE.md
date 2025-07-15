# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

VOICEVOX CLI - Command-line text-to-speech tool using VOICEVOX Core 0.16.0.

## Architecture

- **Client-server model**: Unix socket IPC between CLI client and daemon
- **No caching**: Models loaded/unloaded per request
- **MCP support**: JSON-RPC 2.0 server for AI assistants

## Structure

```
src/
├── bin/
│   ├── client.rs        # voicevox-say
│   ├── daemon.rs        # voicevox-daemon
│   └── mcp_server.rs    # voicevox-mcp-server
├── client/              # Client-side functionality
├── daemon/              # Server-side functionality
├── mcp/                 # MCP protocol implementation
├── synthesis/           # Audio synthesis
├── core.rs              # VOICEVOX Core wrapper
├── voice.rs             # Voice management
├── paths.rs             # Path utilities
└── ipc.rs               # IPC protocol
```

## Key Implementation Details

- **Voice Discovery**: Dynamic scanning of VVM files
- **Audio Playback**: rodio with system command fallbacks
- **Streaming**: Text split at Japanese punctuation
- **Error Handling**: Silent on success, stderr on error

## Usage

```bash
voicevox-daemon --start
voicevox-say "テキスト"
voicevox-mcp-server  # For AI assistants
```