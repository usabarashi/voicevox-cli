# TLA+ State Traceability

This document is scoped to TLA+ artifacts only.
It maps state ownership, cross-module synchronization, and scenario coverage across `modeling/tla` and `modeling/cfg`.

## State Ownership

| Concern | Primary module | Core states/actions |
|---|---|---|
| Daemon lifecycle | `Daemon.tla` | `DaemonDown/Starting/AlreadyRunning/Ready/Recovering`, `StartDaemon`, `AlreadyRunningDetected`, `AlreadyRunningResponsive`, `AlreadyRunningUnresponsive`, `Recover`, `GiveUp` |
| Startup resources | `StartupResources.tla` | runtime/dictionary/socket/model readiness states and retries |
| ONNX runtime resource | `ONNXRuntime.tla` | runtime load/retry/fail transitions |
| Dictionary resource | `Dictionary.tla` | dictionary load/retry/fail transitions |
| Voice model availability | `VoicevoxModel.tla` | `Missing/Exists` style availability and invalid-target outcomes |
| Socket binding/readiness | `Socket.tla` | bind, ready, failure, recovery transitions |
| MCP server connect/playback view | `MCPServer.tla` | `StartConnect`, `ConnectOk`, `ConnectRetry`, `EnterDegraded`, `LeaveDegraded`, playback state transitions |
| Synthesis lifecycle | `Synthesis.tla` | `Idle/Queued/Synthesizing/Done/Failed/Canceled`, retry, invalid-target terminal fail, cancel source propagation |
| Parallel synthesis safety/progress | `SynthesisParallel.tla` | concurrent request safety/liveness abstraction |
| IPC contract behavior | `IPC.tla` | request/response safety and progress |
| Say command flow | `Say.tla` | synthesis + playback path for `say` use case |
| Integrated end-to-end | `System.tla` | composed transition families and cross-view synchronization |

## Cross-Module Synchronization

`System.tla` is the integration point and preserves these correspondences:

- daemon source of truth: `fsDaemonState`
- MCP connection view: `clientDaemonState = IF fsDaemonState = "Ready" THEN "Ready" ELSE "DaemonDown"`
- synthesis readiness view: `synthDaemonReady = (fsDaemonState = "Ready")`
- connected requires ready: `clientState = "Connected" => fsDaemonState = "Ready"`
- synthesizing requires ready: `synthState = "Synthesizing" => fsDaemonState = "Ready"`

This establishes a single authoritative daemon state with synchronized client/synthesis projections.

## Scenario Coverage (`cfg` -> `tla`)

| Scenario config | Module | Main check intent |
|---|---|---|
| `Daemon.startup.cfg` | `Daemon.tla` | daemon startup/recovery bounded behavior |
| `FirstStartup.bootstrap.cfg` | `StartupResources.tla` | first-start resource bootstrap completion |
| `ONNXRuntime.load.cfg` | `ONNXRuntime.tla` | runtime load progress/failure bounds |
| `Dictionary.load.cfg` | `Dictionary.tla` | dictionary load progress/failure bounds |
| `Socket.bind.cfg` | `Socket.tla` | socket bind/readiness transitions |
| `VoicevoxModel.standard.cfg` | `VoicevoxModel.tla` | model existence/missing and target validity |
| `MCPServer.connect.cfg` | `MCPServer.tla` | connect retries and terminal outcomes |
| `MCPServer.degraded.cfg` | `MCPServer.tla` | degraded-mode safety when daemon is unavailable |
| `IPC.safety.cfg` | `IPC.tla` | IPC safety invariants |
| `IPC.progress.cfg` | `IPC.tla` | IPC liveness/progress |
| `Synthesis.full.cfg` | `Synthesis.tla` | full synthesis state machine |
| `Synthesis.normal-flow.cfg` | `Synthesis.tla` | success-only normal path |
| `Synthesis.invalid-target.cfg` | `Synthesis.tla` | invalid target leads to terminal failure |
| `Synthesis.progress.cfg` | `Synthesis.tla` | eventual leave-from-synthesizing properties |
| `Synthesis.retry-boundary.cfg` | `Synthesis.tla` | bounded retry behavior with small retry limit |
| `Synthesis.cancel-sources.cfg` | `Synthesis.tla` | cancellation source attribution invariants |
| `SynthesisParallel.safety.cfg` | `SynthesisParallel.tla` | parallel safety properties |
| `SynthesisParallel.progress.cfg` | `SynthesisParallel.tla` | parallel liveness/progress |
| `Playback.standard.cfg` | `Playback.tla` | playback state transitions and completion |
| `Say.standard.cfg` | `Say.tla` | standard say flow |
| `Say.daemon.cfg` | `Say.tla` | say flow with daemon-coupled scenario |
| `System.integration.cfg` | `System.tla` | cross-module integration consistency |

## Modeling Rules

- Module-local details stay in each `*.tla`.
- Integration consistency is checked in `System.tla`.
- Scenario-specific constraints belong in `*.cfg`, not in module logic.
- If a new state is introduced, update:
  - owning `*.tla`
  - relevant integration mapping in `System.tla` (if shared)
  - at least one `*.cfg` scenario that exercises it
