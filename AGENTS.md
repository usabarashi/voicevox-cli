# AGENTS.md

VOICEVOX CLI design and implementation guidelines.

## System Shape

- Client-server architecture with three binaries:
- `voicevox-say`: CLI client
- `voicevox-daemon`: synthesis daemon
- `voicevox-mcp-server`: MCP server
- Unix domain socket IPC between client and daemon

## Current Project Policy

- Optimize for the current architecture and current IPC contract.
- Backward compatibility is not a default requirement.
- `voicevox-say` and `voicevox-daemon` are released as a matched set.
- Do not add runtime client-daemon version/feature negotiation unless explicitly requested.

## Design Guidelines

- Keep binary entrypoints (`src/bin/*`) thin.
- Put use-case orchestration in `src/app/*`.
- Keep daemon internals modular: request handling, startup, process control, and synthesis policy should stay separated.
- Prefer explicit policies over implicit behavior (example: serialized synthesis policy, no model cache policy).
- Favor simple fixed contracts over compatibility layers when changing IPC.

## Synthesis and Model Handling

- Do not cache voice models in memory.
- Load/unload models per request to keep memory usage predictable.
- Prioritize end-to-end user experience (including playback time), not only synthesis latency.
- Keep synthesis and playback concerns separated where practical.
- Keep text segmentation logic replaceable (strategy-style abstractions are preferred).

## Client/Daemon Behavior

- Client should support automatic daemon startup and retry on first connection failure.
- Startup/retry/backoff behavior should be shared, not duplicated across call paths.
- User-facing startup messages should be concise and actionable.

## IPC Guidelines

- IPC types in `src/ipc.rs` define the source of truth for client-daemon communication.
- Prefer explicit request/response enums with clear error paths.
- When changing IPC, update client and daemon together in the same change.
- Remove obsolete IPC variants instead of keeping unused compatibility branches.

## Module Responsibilities

- `src/app/*`: application/use-case orchestration
- `src/client/*`: CLI-side daemon connection, playback, downloads/setup
- `src/daemon/*`: daemon startup, socket server, process control, request execution
- `src/synthesis/*`: synthesis backend preparation, streaming, segmentation, shared synthesis services
- `src/mcp/*`: MCP protocol handling and tool execution
- `src/core*`: VOICEVOX Core bindings/integration

## Implementation Notes

- `nix flake check` uses the Git-tracked flake source snapshot.
- New files may be excluded until tracked (for example `git add -N <path>`).
- MCP instruction loading behavior is defined by the implementation and `VOICEVOX.md`; keep `AGENTS.md` focused on design/implementation guidance.
