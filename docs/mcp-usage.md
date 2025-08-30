# VOICEVOX MCP Server Usage

The VOICEVOX MCP (Model Context Protocol) server enables AI assistants to generate Japanese speech using VOICEVOX synthesis.

## Prerequisites

1. Complete resource setup:
   ```bash
   voicevox-setup
   ```

2. The VOICEVOX daemon will be started automatically when needed

## Starting the MCP Server

```bash
voicevox-mcp-server
```

The server communicates via stdio (standard input/output) using JSON-RPC protocol.

## Server Initialization

Before using any tools, initialize the MCP server:

```json
{
  "jsonrpc": "2.0",
  "method": "initialize",
  "params": {
    "protocolVersion": "2024-11-05",
    "capabilities": {},
    "clientInfo": {
      "name": "your-client",
      "version": "1.0.0"
    }
  },
  "id": 1
}
```

The server will respond with its capabilities and available tools.

## AI Assistant Instructions

The MCP server automatically loads behavioral instructions for AI assistants from `INSTRUCTIONS.md`. These instructions define:

- **Audio usage policies**: When and how to use voice synthesis
- **Voice style guidelines**: Which voice styles to use in different situations
- **Context-aware behavior**: How to adapt audio output to user workflow

### Default Instructions

By default, the server loads instructions from:
1. File specified by `VOICEVOX_MCP_INSTRUCTIONS` environment variable
2. `INSTRUCTIONS.md` in the executable directory
3. `INSTRUCTIONS.md` in the current working directory

### Custom Instructions

To use custom instructions for specific workflows:

```bash
export VOICEVOX_MCP_INSTRUCTIONS=/path/to/custom/instructions.md
voicevox-mcp-server
```

Example custom instructions structure:
```markdown
# Custom VOICEVOX Instructions

## Audio Usage Policy
- Use audio for all user-facing responses
- Prefer style ID 22 for technical discussions

## Voice Styles
- ID: 3 - Default communications
- ID: 1 - Success notifications
- ID: 76 - Error situations
```

## Available Tools

### 1. `text_to_speech`

Converts Japanese text to speech (TTS) and plays it on the server.

**Parameters:**
- `text` (required): Japanese text to synthesize
- `style_id` (required): Voice style ID (e.g., 3 for Zundamon Normal)
- `rate` (optional): Speech rate (0.5-2.0, default: 1.0)
- `streaming` (optional): Enable streaming playback (default: true)

**Example:**
```json
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "text_to_speech",
    "arguments": {
      "text": "こんにちは、ずんだもんなのだ",
      "style_id": 3,
      "streaming": true
    }
  },
  "id": 1
}
```

### 2. `list_voice_styles`

Retrieves available voices with optional filtering.

**Parameters:**
- `speaker_name` (optional): Filter by speaker name (partial match)
- `style_name` (optional): Filter by style name (partial match)

**Example:**
```json
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "list_voice_styles",
    "arguments": {
      "speaker_name": "ずんだもん"
    }
  },
  "id": 2
}
```

## Testing

### Initialize the server:
```bash
echo '{"jsonrpc":"2.0","method":"initialize","params":{},"id":1}' | voicevox-mcp-server
```

### List available tools:
```bash
echo '{"jsonrpc":"2.0","method":"tools/list","params":{},"id":2}' | voicevox-mcp-server
```

### Synthesize speech:
```bash
echo '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"text_to_speech","arguments":{"text":"テストなのだ","style_id":3}},"id":3}' | voicevox-mcp-server
```

## Integration with AI Assistants

Any MCP-compatible AI assistant can use this server. Configure your assistant to launch `voicevox-mcp-server` as an MCP server process.

### Claude Desktop Example

Configure Claude Desktop to use the VOICEVOX MCP server:

```json
{
  "mcpServers": {
    "voicevox": {
      "command": "voicevox-mcp-server",
      "args": [],
      "env": {
        "VOICEVOX_MCP_INSTRUCTIONS": "/path/to/custom/instructions.md"
      }
    }
  }
}
```

The AI assistant will automatically receive and follow the instructions from `INSTRUCTIONS.md`, enabling context-aware voice synthesis during conversations.

## Streaming vs Non-Streaming

- **Streaming mode (default)**: Text is split into segments and synthesized progressively for lower latency
- **Non-streaming mode**: Entire text is synthesized at once before playback

Streaming is recommended for longer texts to reduce perceived wait time.

## Error Handling

Common errors:
- "Resources not found": Run `voicevox-setup` to download required resources
- "Failed to connect to daemon": Daemon auto-start failed, check system resources
- "Invalid style_id": Use `list_voice_styles` to see available style IDs
- "Audio device not available": Check system audio settings