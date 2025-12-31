# AGENTS.md

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

### Project Structure

```
voicevox-cli/
├── src/
│   ├── bin/
│   │   ├── client.rs          # voicevox-say binary
│   │   ├── daemon.rs          # voicevox-daemon binary
│   │   └── mcp_server.rs      # voicevox-mcp-server binary
│   ├── client/                # Client-side logic
│   ├── daemon/                # Daemon server logic
│   ├── core/                  # VOICEVOX Core FFI bindings
│   ├── ipc/                   # Unix socket IPC protocol
│   ├── synthesis/             # Streaming synthesis engine
│   ├── mcp/                   # MCP protocol (rmcp-based)
│   └── lib.rs                 # Library root
├── tests/
│   └── integration/           # Integration test suite
│       ├── verify_binaries.sh
│       ├── test_mcp_protocol.py
│       ├── test_synthesis_modes.py
│       └── README.md
├── Cargo.toml
└── VOICEVOX.md               # MCP server instructions
```

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
- `mcp/`: MCP protocol implementation using [rmcp](https://github.com/4t145/rmcp) framework

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

### Implementation Architecture

The MCP server is built using the [rmcp](https://github.com/4t145/rmcp) crate, providing a standardized framework for Model Context Protocol implementation.

**Implementation:**
- Protocol: MCP 2024-11-05
- Tool definitions: `#[tool]` macro on async methods
- Tool routing: `#[tool_router]` macro generates routing logic
- Server handler: `#[tool_handler]` macro **REQUIRED** for tool registration
- Parameter schemas: Automatic generation via `#[derive(JsonSchema)]`
- Validation: `McpError::invalid_params()` for parameter errors

### Available Tools

- `text_to_speech`: Convert Japanese text to speech with configurable voice style, rate, and streaming
- `list_voice_styles`: Query available voice styles with optional filtering by speaker or style name

### Instruction System

The MCP server dynamically loads behavior instructions to guide AI assistant interactions.

**Loading Priority (XDG Base Directory compliant):**

1. **Environment variable**: `VOICEVOX_MCP_INSTRUCTIONS` (highest priority)
2. **XDG user config**: `$XDG_CONFIG_HOME/voicevox/VOICEVOX.md` (user-specific settings)
3. **Config fallback**: `~/.config/voicevox/VOICEVOX.md` (when XDG_CONFIG_HOME is not set)
4. **Executable directory**: `VOICEVOX.md` bundled with the binary (distribution default)
5. **Current directory**: `VOICEVOX.md` in working directory (development use)

**Configuration examples:**

```bash
# Method 1: Environment variable (highest priority)
export VOICEVOX_MCP_INSTRUCTIONS=/path/to/custom/instructions.md
voicevox-mcp-server

# Method 2: XDG_CONFIG_HOME (if set)
mkdir -p $XDG_CONFIG_HOME/voicevox
cp custom-instructions.md $XDG_CONFIG_HOME/voicevox/VOICEVOX.md
voicevox-mcp-server

# Method 3: XDG user configuration
mkdir -p ~/.config/voicevox
cp custom-instructions.md ~/.config/voicevox/VOICEVOX.md
voicevox-mcp-server
```

Server operates normally without instruction files. Default behavior defined in [VOICEVOX.md](VOICEVOX.md).

## Integration Testing

### Prerequisites

**CRITICAL**: Always verify you're testing the newly built binaries, not system-installed versions.

**Automated verification:**
```bash
# Run binary verification tests (recommended)
cargo test --test verify_binaries -- --nocapture
```

This test suite automatically checks:
- All binaries are built and timestamped
- Running daemon is development build (not system version)
- MD5 hashes differ from system-installed binaries
- MCP protocol version is correct (2024-11-05)

### Test Workflow

#### 1. Build and Unit Tests

```bash
# Clean build to ensure fresh artifacts
nix develop -c cargo clean -p voicevox-cli
nix develop -c cargo build

# Run unit tests and clippy
nix develop -c cargo test --lib
nix develop -c cargo clippy
```

#### 2. Binary Verification (Automated)

```bash
# Run verification tests before integration tests
cargo test --test verify_binaries -- --nocapture
```

If manual verification is needed:
```bash
# Check running daemon
pgrep -fl voicevox-daemon  # Should show target/debug path

# Check binary hashes
md5 target/debug/voicevox-daemon
which voicevox-daemon && md5 $(which voicevox-daemon)
```

#### 3. Integration Tests

Run MCP protocol integration tests:

```bash
# 1. Verify binaries (recommended first step)
cargo test --test verify_binaries -- --nocapture

# 2. Run protocol tests (no daemon required)
cargo test --test mcp_protocol

# 3. Start daemon for synthesis tests
./target/debug/voicevox-daemon --start --detach
sleep 2

# 4. Run synthesis tests (requires daemon)
cargo test --test synthesis_modes --ignored
```

Tests are located in `tests/integration/`:
- `verify_binaries.rs` - Binary verification and environment checks
- `mcp_protocol.rs` - MCP protocol compliance
- `synthesis_modes.rs` - Audio synthesis (daemon and streaming modes)
- `common/mod.rs` - Shared test utilities (`McpClient`, helpers)

### Binary Verification Checklist

Before running integration tests:

- [ ] All system daemons stopped (`pgrep voicevox`)
- [ ] Fresh build completed (`ls -lh target/debug/voicevox-*`)
- [ ] MD5 hashes differ from system binaries
- [ ] Test scripts use **absolute paths** to `target/debug/` binaries
- [ ] No `which voicevox-*` paths used in tests

### Common Pitfalls

1. **PATH confusion**: System-installed binaries (Nix/Homebrew) may shadow development builds
2. **Daemon persistence**: Old daemon processes continue running after code changes
3. **Test isolation**: Tests using `which` or bare command names may invoke wrong binary
4. **Hash verification**: Always compare MD5/timestamps to confirm binary identity

### Automated Test Suite

Integration tests are written in Rust:

```
tests/integration/
├── common/
│   └── mod.rs                # Test utilities (McpClient, helpers)
├── verify_binaries.rs        # Binary verification tests
├── mcp_protocol.rs           # MCP protocol compliance tests
└── synthesis_modes.rs        # Synthesis mode tests
```

Run full test suite:

```bash
# 1. Verify binaries
cargo test --test verify_binaries -- --nocapture

# 2. Run protocol tests
cargo test --test mcp_protocol

# 3. Run synthesis tests (requires daemon)
./target/debug/voicevox-daemon --start --detach
cargo test --test synthesis_modes --ignored
```
