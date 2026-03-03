# Rust Phase to TLA+ Traceability

This document maps runtime phases in Rust to the current TLA+ models.
It is intentionally approximate: TLA+ models are abstractions, not line-by-line code mirrors.

## Rust source of phase enums

- `src/app/system_state.rs`

## Mapping

| Rust Phase | Rust meaning | TLA+ module | TLA+ states/actions |
|---|---|---|---|
| `StartupPhase::InitialConnect` | Try existing daemon socket first | `Client.tla`, `Daemon.tla` | `StartConnect`, `ConnectOk`, daemon-side `AcceptReq` precondition |
| `StartupPhase::ValidateModels` | Verify startup resources exist before spawn | `FirstStartup.tla` | `ResourceReady` guard (`runtimeState/dictState/modelState = "Ready"`) |
| `StartupPhase::StartDaemon` | Spawn/recover daemon process | `Daemon.tla`, `FirstStartup.tla` | `StartDaemon`, `DaemonReady`, `DaemonFail`, `Recover`, `GiveUp` |
| `StartupPhase::ConnectRetry` | Retry connect with backoff after spawn | `Client.tla` | `ConnectRetry`, `ConnectOk`, `ConnectFail` |
| `McpStartupPhase::InitialStart` | MCP startup attempt for daemon | `Daemon.tla` | `StartDaemon`, `DaemonReady`, `DaemonFail` |
| `McpStartupPhase::RecoverAlreadyRunning` | Kill/recover stuck daemon and retry | `Daemon.tla` | `CrashFromReady` + `Recover` abstraction |
| `SynthesisPhase::Validate` | Validate text/style/rate request | `Synthesis.tla` | pre-`Enqueue` guard (abstracted) |
| `SynthesisPhase::EnsureResources` | Optional on-demand resource setup | `FirstStartup.tla`, `System.tla` | startup/resource readiness path before synthesis |
| `SynthesisPhase::Connect` | Obtain daemon client/session | `Client.tla`, `System.tla` | `StartConnect`, `ConnectOk` |
| `SynthesisPhase::Synthesize` | RPC synthesis call execution | `Synthesis.tla` | `Enqueue`, `StartSynth`, `SynthOk`, `SynthFail`, `InvalidTargetFail`, `Cancel` |
| `SayPhase::Validate` | CLI input validation | `Client.tla` | pre-connection guard (abstracted) |
| `SayPhase::Synthesize` | Run daemon synthesis pipeline | `System.tla`, `Synthesis.tla` | same as synthesis path above |
| `SayPhase::Emit` | Playback/write output | `Playback.tla` | `AudioArrived`, `StartPlayback`, `LaunchOk`, `NaturalEnd`, `LaunchFail` |
| `McpTtsPhase::Attempt` | Synthesis attempt (with cancellation checks) | `Synthesis.tla`, `SynthesisParallel.tla` | `StartSynth`, `SynthOk`, `SynthFail`, `InvalidTargetFail`, `Cancel` |
| `McpTtsPhase::Backoff` | Retry delay before next attempt | `Synthesis.tla` | retry progression via `SynthFail` + bounded `retryCount` |
| `McpTtsPhase::Finish` | Terminal return path | `Synthesis.tla` | terminal states `Done/Failed/Canceled` |

## Notes on abstraction gaps

- Rust has concrete error formatting and IO details; TLA+ intentionally omits message text and file paths.
- Rust has explicit sleep/backoff durations; TLA+ models retry as bounded counters and nondeterministic scheduling.
- `McpStartupPhase::RecoverAlreadyRunning` includes PID/signal handling in Rust, but is represented as state transitions (`Ready/Recovering/Starting`) in TLA+.
- Playback is modeled separately (`Playback.tla`) and is only loosely coupled in Rust via emit stage.
