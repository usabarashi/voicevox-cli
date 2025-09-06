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

The MCP server automatically loads behavioral instructions for AI assistants from `VOICEVOX.md`. These instructions define:

- **Audio usage policies**: When and how to use voice synthesis
- **Voice style guidelines**: Which voice styles to use in different situations
- **Context-aware behavior**: How to adapt audio output to user workflow

### Default Instructions

The server loads instructions using XDG Base Directory specification with the following priority order:

1. **Environment variable**: File specified by `VOICEVOX_MCP_INSTRUCTIONS` (highest priority)
2. **XDG user config**: `$XDG_CONFIG_HOME/voicevox/VOICEVOX.md` (user-specific settings)
3. **Config fallback**: `~/.config/voicevox/VOICEVOX.md` (when XDG_CONFIG_HOME is not set)
4. **Executable directory**: `VOICEVOX.md` bundled with the binary (distribution default)
5. **Current directory**: `VOICEVOX.md` in working directory (development use)

### Custom Instructions

You can customize the AI assistant behavior using several methods:

#### Method 1: Environment Variable (Highest Priority)
```bash
export VOICEVOX_MCP_INSTRUCTIONS=/path/to/custom/instructions.md
voicevox-mcp-server
```

#### Method 2: XDG_CONFIG_HOME (If Set)
```bash
# When XDG_CONFIG_HOME is configured (higher priority)
mkdir -p $XDG_CONFIG_HOME/voicevox
cp custom-instructions.md $XDG_CONFIG_HOME/voicevox/VOICEVOX.md
voicevox-mcp-server
```

#### Method 3: Config Fallback (Recommended for most users)
```bash
# Create user-specific configuration (XDG default location)
mkdir -p ~/.config/voicevox
cp custom-instructions.md ~/.config/voicevox/VOICEVOX.md
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

### XDG Base Directory Support

The VOICEVOX MCP server follows the [XDG Base Directory Specification](https://specifications.freedesktop.org/basedir-spec/basedir-spec-latest.html), providing clean separation between:

- **User configurations**: Personal settings that persist across updates
- **Distribution defaults**: Settings bundled with the application
- **Development settings**: Project-specific configurations for development

#### Benefits

1. **User-specific customization**: Settings in `~/.config/voicevox/` persist across application updates
2. **Multi-environment support**: Different configurations for different projects using XDG_CONFIG_HOME
3. **Clean separation**: User settings don't interfere with distribution defaults
4. **Standard compliance**: Follows Unix/Linux configuration management conventions

#### Debugging Configuration Loading

The MCP server logs which configuration file it loads:

```bash
# Enable debug output to see configuration loading
voicevox-mcp-server 2>&1 | grep "instructions"
```

Example output:
```
Trying instructions from XDG_CONFIG_HOME: /home/user/.config/voicevox/VOICEVOX.md
Loaded instructions from: /home/user/.config/voicevox/VOICEVOX.md
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

The AI assistant will automatically receive and follow the instructions from `VOICEVOX.md`, enabling context-aware voice synthesis during conversations.

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