# CLAUDE.md

VOICEVOX CLI - Command-line text-to-speech tool using VOICEVOX Core.

## Architecture

- Client-server model with Unix socket IPC
- Dynamic VVM model loading per request (no memory caching)
- MCP server for AI assistant integration

## Commands

```bash
voicevox-say "テキスト"              # Text-to-speech client
voicevox-daemon --start             # Start daemon server
voicevox-mcp-server                 # MCP server for AI assistants
```

## Key Features

- Automatic daemon startup when needed
- Streaming synthesis for long text
- Rate control (0.5-2.0x speed)
- Multiple voice styles (speaker IDs)

## MCP Tools

- `text_to_speech`: Synthesize text with style ID and rate
- `list_voice_styles`: List available voices with filtering