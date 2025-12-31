# Integration Tests

Integration tests for VOICEVOX CLI to verify end-to-end functionality.

## Quick Start

```bash
# 1. Build the project
nix develop -c cargo build

# 2. Verify binaries
cargo test --test verify_binaries -- --nocapture

# 3. Start the daemon (optional, for synthesis tests)
./target/debug/voicevox-daemon --start --detach

# 4. Run tests
cargo test --test mcp_protocol
cargo test --test synthesis_modes --ignored  # Requires daemon
```

## Test Structure

```
tests/integration/
├── common/
│   └── mod.rs              # Shared test utilities (McpClient, helpers)
├── verify_binaries.rs      # Binary verification tests
├── mcp_protocol.rs         # MCP protocol compliance tests
└── synthesis_modes.rs      # Audio synthesis tests (requires daemon)
```

## Test Suites

### `verify_binaries.rs`

Tests binary verification and environment setup:

- `test_check_running_daemon` - Check if daemon is running and if it's development build
- `test_binaries_exist` - Verify all binaries are built
- `test_compare_with_system_binaries` - Compare hashes with system-installed versions
- `test_mcp_protocol_version` - Verify MCP protocol version (2024-11-05)
- `test_print_summary` - Print test summary

**No daemon required** - Can run anytime after build.

**Usage:**
```bash
cargo test --test verify_binaries -- --nocapture
```

### `mcp_protocol.rs`

Tests Model Context Protocol implementation:

- `test_initialize_sequence` - Initialize handshake
- `test_tools_list` - Tool discovery
- `test_invalid_tool_call` - Error handling
- `test_list_voice_styles` - Voice styles query
- `test_parameter_validation_*` - Input validation

**No daemon required** - Tests only MCP protocol layer.

**Usage:**
```bash
cargo test --test mcp_protocol
```

### `synthesis_modes.rs`

Tests both synthesis modes with actual audio generation:

- `test_daemon_mode_synthesis` - Daemon-based synthesis
- `test_streaming_mode_synthesis` - Streaming synthesis
- `test_different_voice_styles` - Multiple style IDs
- `test_different_speech_rates` - Rate variations

**Requires daemon running** - Tests marked with `#[ignore]`.

**Usage:**
```bash
# Start daemon first
./target/debug/voicevox-daemon --start --detach

# Run synthesis tests
cargo test --test synthesis_modes --ignored
```


## Common Test Utilities

The `common` module provides:

### `McpClient`

JSON-RPC client for MCP protocol testing:

```rust
use common::{McpClient, JsonRpcRequest};

let mut client = McpClient::start("./target/debug/voicevox-mcp-server")?;
client.initialize()?;

let request = JsonRpcRequest::new("tools/list").with_id(1);
let response = client.call(&request)?;
```

### Helper Functions

- `get_server_path()` - Get MCP server binary path
- `is_daemon_running()` - Check if daemon is running

## Common Workflows

### Full Test Suite

```bash
# 1. Clean build
nix develop -c cargo clean
nix develop -c cargo build

# 2. Verify binaries
cargo test --test verify_binaries -- --nocapture

# 3. Run protocol tests (no daemon needed)
cargo test --test mcp_protocol

# 4. Start daemon
./target/debug/voicevox-daemon --start --detach
sleep 2

# 5. Run synthesis tests
cargo test --test synthesis_modes --ignored
```

### Quick Verification After Code Change

```bash
# Rebuild
nix develop -c cargo build

# Test protocol only (fast)
cargo test --test mcp_protocol
```

### Run All Tests (with daemon)

```bash
# Ensure daemon is running
pgrep voicevox-daemon || ./target/debug/voicevox-daemon --start --detach

# Run all integration tests
cargo test --tests --ignored
```

## Critical Testing Points

### 1. Binary Isolation

Tests use absolute paths to avoid PATH confusion:

```rust
// ✅ GOOD - Explicit path from environment or default
let server_path = get_server_path();

// ❌ BAD - May use system binary
let server_path = "voicevox-mcp-server";
```

### 2. Daemon Verification

Check running daemon is development build:

```bash
# Should show target/debug path, not /nix/store
pgrep -fl voicevox-daemon
```

### 3. Test Isolation

Each test creates its own `McpClient` instance. Tests are independent and can run in parallel.

### 4. Ignored Tests

Tests requiring daemon are marked with `#[ignore]`:

```rust
#[test]
#[ignore = "requires daemon running"]
fn test_daemon_mode_synthesis() -> Result<()> {
    // ...
}
```

Run with `--ignored` flag when daemon is available.

## Troubleshooting

### "Failed to spawn MCP server"

```bash
# Check binary exists
ls -lh ./target/debug/voicevox-mcp-server

# Rebuild if needed
nix develop -c cargo build --bin voicevox-mcp-server
```

### "Daemon not running" (synthesis tests)

```bash
# Start daemon
./target/debug/voicevox-daemon --start --detach

# Verify it's running
pgrep -fl voicevox-daemon
```

### "Wrong binary running"

```bash
# Stop system daemon
pkill voicevox-daemon

# Start dev daemon
./target/debug/voicevox-daemon --start --detach

# Verify path
pgrep -fl voicevox-daemon | grep target/debug
```

### "Test hangs"

MCP server may be waiting for input. Check:

```bash
# Kill any stuck processes
pkill voicevox-mcp-server

# Re-run test with verbose output
cargo test --test mcp_protocol -- --nocapture
```

## CI Integration

For automated testing in CI:

```yaml
- name: Build
  run: nix develop -c cargo build

- name: Run protocol tests
  run: cargo test --test mcp_protocol

- name: Start daemon
  run: |
    ./target/debug/voicevox-daemon --start --detach
    sleep 5

- name: Run synthesis tests
  run: cargo test --test synthesis_modes --ignored

- name: Cleanup
  if: always()
  run: pkill voicevox-daemon || true
```

## Development Notes

- Tests use only standard library and project dependencies (no external test frameworks)
- JSON-RPC communication via stdin/stdout
- Tests are idempotent (can run multiple times)
- Synthesis tests require actual VVM models installed

For more details, see [AGENTS.md](../../AGENTS.md#integration-testing).
