# VOICEVOX MCP Server Usage

The VOICEVOX MCP (Model Context Protocol) server enables AI assistants to generate Japanese speech using VOICEVOX synthesis.

## Prerequisites

1. VOICEVOX daemon must be running:
   ```bash
   voicevox-daemon --start
   ```

2. Ensure voice models are available in the models directory

## Starting the MCP Server

```bash
voicevox-mcp-server
```

The server communicates via stdio (standard input/output) using JSON-RPC protocol.

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

## Streaming vs Non-Streaming

- **Streaming mode (default)**: Text is split into segments and synthesized progressively for lower latency
- **Non-streaming mode**: Entire text is synthesized at once before playback

Streaming is recommended for longer texts to reduce perceived wait time.

## Error Handling

Common errors:
- "Failed to connect to daemon": Start the VOICEVOX daemon first
- "Invalid style_id": Use `list_voice_styles` to see available style IDs
- "Audio device not available": Check system audio settings