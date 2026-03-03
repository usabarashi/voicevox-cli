# AGENTS.md

VOICEVOX CLI design and implementation guidelines.

## System Shape

- Three binaries:
- `voicevox-say` (CLI)
- `voicevox-daemon` (daemon)
- `voicevox-mcp-server` (MCP server)
- Client/daemon communication uses Unix domain socket IPC.

## Layer Structure

- `src/bin/*`: entrypoints only (argument parsing, top-level error/exit handling).
- `src/interface/*`: protocol/UI boundary (CLI, MCP, stdio, playback orchestration).
- `src/infrastructure/*`: external systems (VOICEVOX Core, daemon process/socket, downloads, filesystem, IPC wire types).
- `src/domain/*`: pure rules and value-level logic (validation, splitting, synthesis constraints).

## Current Responsibility Map

- `src/interface/cli/*`: CLI flows and user-facing behavior.
- `src/interface/mcp_server/*`: MCP protocol handling and tool routing.
- `src/interface/synthesis/*`: shared synthesis orchestration used by CLI and MCP.
- `src/interface/playback.rs`: shared playback path used by CLI and MCP.
- `src/infrastructure/daemon/*`: daemon runtime, daemon client transport, process control.
- `src/infrastructure/ipc/*`: daemon IPC contract and frame limits.
- `src/infrastructure/voicevox.rs`: VOICEVOX model/speaker discovery and mappings.

## Design Rules

- Keep entrypoints thin; move behavior into interface/infrastructure/domain modules.
- Keep domain free of CLI/MCP/process concepts.
- Keep interface free of OS/process primitives where possible; push those to infrastructure.
- Keep IPC contracts explicit and stable; update both client and daemon in the same change.
- Do not add compatibility layers unless explicitly requested.

## Synthesis Policy

- Do not cache voice models in memory.
- Load/unload models per request.
- Prefer predictable memory behavior over raw latency micro-optimizations.
- Keep text segmentation logic replaceable.

## Delivery Notes

- Backward compatibility is not required by default.
- `voicevox-say` and `voicevox-daemon` are treated as a matched set.
- `nix flake check` uses the Git-tracked flake snapshot.
