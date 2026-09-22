# Phase 2: TLA+ → Quint Porting Plan

This is the property / assumption / dependency correspondence table for porting
the remaining TLA+ modules to Quint. It is written **before** the port, and is
the unit of work (properties and dependencies), not file count.

Phase 1 (`CONTRACT.md`) already covered the implementation-facing Synthesis
contract (`SynthesisRetry.qnt`) and target resolution (`TargetResolution.qnt`).

## Dependency-driven port order

`INSTANCE` composition in the TLA+ modules:

| Wave | Modules | Depends on |
|---|---|---|
| A (leaves) | `ONNXRuntime`, `Dictionary`, `Socket`, `Playback`, `IPC`, `Daemon`, `VoicevoxModel`, `SynthesisParallel` | — |
| B (compose) | `StartupResources`, `MCPServer`, `Say` | A: `StartupResources` → ONNXRuntime/Dictionary/Socket; `MCPServer` → Playback; `Say` → Daemon + Playback |
| C (integrate) | `System` | B: StartupResources + MCPServer + Synthesis |

`Synthesis` is special: Phase 1 replaced its abstract contract with
`SynthesisRetry.qnt`. `System` must therefore be adapted (see below).

## Property correspondence

Legend: **preserve** = port 1:1; **move** = same meaning, different home;
**retract** = intentionally dropped, with reason; **replace** = superseded by a
different property.

| Module | Old property | Disposition | Quint target / reason |
|---|---|---|---|
| `ONNXRuntime` | `TypeOK`, `ReadyHasNoPendingRetry` | preserve | `ONNXRuntime.qnt` (done) |
| `Dictionary` | `TypeOK`, `ReadyIsStable` | preserve | `Dictionary.qnt` (done) |
| `Socket` | `TypeOK`, `ReadyIsBounded` | preserve | `Socket.qnt` (done) |
| `Playback` | `TypeOK`, `PlayingRequiresAudio`, `CanceledImpliesStoppedOrFailed` | preserve | `Playback.qnt` (done) |
| `IPC` | `TypeOK`, `FailedImpliesError`, `DoneImpliesValidResponse`, `EventuallyLeavesInFlight` | preserve | `IPC.qnt` (done) |
| `Daemon` | `TypeOK`, `SocketImpliesReady`, `BusyImpliesReady`, `AlreadyRunningNotBusy`, `RetryBounded` | preserve | `Daemon.qnt` (done) |
| `Daemon` | `RecoveryPathExists` | retract | not checked by any cfg; does not hold under weak fairness (`daemonFail` can exhaust `retryCount` before `recover` is taken) |
| `SynthesisParallel` | `TypeOK`, `AtMostOneSynthesizing`, `WorkerMatchesSynthesis`, `EventuallyLeavesBusyWorker` | replace | `DaemonSerialization.qnt` (revised): the TLA+ model was client-side parallel jobs with cancellation releasing the worker and daemon-side retry/requeue. Production serializes one synthesis per daemon (`SerializedSynthesisPolicy` mutex), has no cancel request, and retries on the client. `TypeOK` (numeric counters) is dropped. |
| `StartupResources` | `TypeOK`, `DaemonReadyRequiresDownloads`, `DaemonStartRequiresDownloads`, `DaemonReadyRequiresSocket` | preserve | `StartupResources.qnt` (done, flattened) |
| `MCPServer` | `TypeOK`, `ConnectedImpliesDaemonReady`, `DegradedImpliesNotConnected`, `PlayingRequiresAudio` | preserve | `MCPServer.qnt` (done, flattened) |
| `Say` | `TypeOK`, `SynthesizingImpliesBusyReq`, `BusyReqOwnedBySay`, `DoneHasNoError`, `PlaybackFailureOnlyInPlayMode` (+ `PlayingRequiresAudio`, `EmittingUsesPlayMode`) | preserve (flattened) | `Say.qnt` (done) |
| `System` | `TypeOK`, `ViewsAligned`, `ClientConnectedImpliesDaemonReady`, `SynthesisRunningImpliesDaemonReady` | preserve (adapt INSTANCE to `SynthesisRetry`) | `System.qnt` (done) |
| `Synthesis` | `TypeOK` | replace | `SynthesisRetry.qnt` (Phase 1) |
| `Synthesis` | `TerminalStates` (`retryCount ≤ MAX_RETRY`) | replace | `attemptsBounded` / `backoffsBounded` (Phase 1) |
| `Synthesis` | `SynthesisNeedsDaemon` | retract | `daemonReady` gating dropped; daemon is environment (Phase 1 contract change 3) |
| `Synthesis` | `CanceledHasSource`, `NonCanceledHasNoSource` | retract | no `cancelSource` enum in code (Phase 1 contract change 4) |
| `Synthesis` | `InvalidTargetIsTerminalFailure` | move | invalid target is a non-retryable (`FatalFailure`) attempt outcome in `RetryPolicy` |
| `Synthesis` | `EventuallyLeavesSynthesizing` | replace | `eventuallyTerminal` (stronger: reaches Done/Failed/Canceled) (Phase 1 contract change 2) |
| `Synthesis` | `NormalFlowNoFailure` | retract | retry exists (Phase 1 contract change 1) |
| `VoicevoxModel` | `TypeOK` | replace | type bounds in `TargetResolution.qnt` |
| `VoicevoxModel` | `AcceptedRequiresReadyAndExistingTarget`, `RejectedMissingModelHasReason` | retract | download lifecycle belongs to `infrastructure/download`; target resolution modeled separately (Phase 1 contract change 5) |

### System adaptation

`System.tla` INSTANCEs `Synthesis` with `synthState`, `retryCount`,
`errorKind`, `cancelSource`, `daemonReady`. `SynthesisRetry.qnt` exposes
`outcome`, `attempts`, `backoffs` and no daemon gating. The ported `System.qnt`
does the following:

- `viewsAligned` reduces to the client view (`clientDaemonState`) derived from
  `daemonState`; the old `synthDaemonReady` view is gone.
- `SynthesisRunningImpliesDaemonReady` is **retracted**. Production auto-starts
  the daemon (`connect_daemon_client_auto_start`) and treats daemon loss as a
  retryable attempt failure, not a cancellation, so synthesis is
  environment-driven and does not require a ready daemon. `daemonLost` only
  updates the client view and resets the connect budget.
- A non-retryable attempt failure (`synthFatalFail`) fails from any attempt,
  matching `SynthesisRetry` (failure is not limited to the last attempt).
- The four startup resources are encoded as an indexed map
  (`resources: int -> LoadState`, `retries: int -> int`, indices
  0=runtime, 1=dictionary, 2=socket, 3=model) using `nondet` over the index. This
  preserves `resourceReady` and `DaemonReadyRequiresSocket` while keeping the
  module compact.
- Playback transitions are omitted: they are verified in `MCPServer.qnt` and do
  not affect the invariants checked by `System.integration.cfg`.

`System.qnt` verification explores ~85k distinct states with the TLC backend.

## Exit criteria for Phase 2

Porting is complete: every preserved property has a Quint equivalent that
verifies green with `quint verify --backend=tlc`. The handwritten
`modeling/tla`, `modeling/cfg`, the `tla-model-check` job, and `tlaplus` from
the devShell have been removed. The TLC engine is retained via Quint's TLC
backend.

## Scenario mapping (cfg → Quint)

The old `cfg` selects constants, a spec variant, invariants, and temporal
properties. In Quint these become `init`/`step` plus `--invariant` /
`--temporal` selections; constants are concrete `pure val`s (composition is
flattened, so instance parameters are not used).

The table below reflects the current `verify.sh`; constants now track the
production constants they abstract (see "Post-migration revisions").

| Module | Constants | Invariants / temporal properties |
|---|---|---|
| ONNXRuntime | `MAX_RETRY=3` | `typeOK` + temporal `loadTerminates` |
| Dictionary | `MAX_RETRY=3` | `typeOK` + temporal `loadedStaysReady` |
| Socket | `MAX_RETRY=3` | `typeOK` + temporal `bindingTerminates` |
| Playback | — | `playingRequiresAudio`, `canceledImpliesStoppedOrFailed` |
| IPC | frame/timeout values | `failedImpliesError`, `doneImpliesValidResponse`, `inFlightHasNoError` + temporal `eventuallyLeavesInFlight` |
| Daemon | `MAX_RETRY=10` | `typeOK`, `socketImpliesReady`, `busyImpliesReady`, `alreadyRunningNotBusy`, `retryBounded` |
| DaemonSerialization | — | `atMostOneSynthesizing`, `workerMatchesSynthesis` + temporal `eventuallyLeavesBusyWorker` |
| DaemonServer | `MAX_IN_FLIGHT=32` | `typeOK`, `inFlightMatchesHandling` + temporal `handling{0,1,2}Terminates` |
| McpRequestLifecycle | `MAX_CONCURRENT=4` | `typeOK`, `activeMatchesRunning` + temporal `allRequestsTerminate` |
| StartupResources | `MAX_RETRY=3` | `typeOK`, `daemonReadyRequiresDownloads`, `daemonStartRequiresDownloads`, `daemonReadyRequiresSocket` |
| MCPServer | `MAX_ATTEMPTS=10` | `typeOK`, `connectedImpliesDaemonReady`, `playingRequiresAudio` |
| Say | `MAX_RETRY=10` | `typeOK`, `synthesizingImpliesBusyReq`, `busyReqOwnedBySay`, `doneHasNoError`, `playbackFailureOnlyInPlayMode`, `outputFailureOnlyInFileMode`, `playingRequiresAudio`, `emittingUsesPlayMode` |
| System | resource retries 3, synth retries 2, connect attempts 10 | `typeOK`, `viewsAligned`, `clientConnectedImpliesDaemonReady` |
| StreamingSynthesis | — | `segmentsBounded`, `playbackRequiresAllSegments`, `canceledImpliesNoPlayback` |
| Download | `MAX_ATTEMPTS=3` | `attemptsBounded`, `failedHasReason`, `exhaustedImpliesAttempts`, `preparationFailureMeansNoAttempts` + temporal `terminates` |
| ModelLifecycle | — | `loadedImpliesPhase` + temporal `eventuallyUnloaded` |
| DaemonIpc | — | `catalogRequiresConnection` |
| DaemonSynthesize | — | `synthesizedRequiresCatalog` |
| SynthesisRetry | `MAX_RETRIES=2` | `attemptsBounded`, `backoffsBounded`, `backoffAfterAttempt` + temporal `eventuallyTerminal`, `cancelIsTerminal` |
| McpRequestParsing | — | typecheck + MBT only |
| TargetResolution | fixture catalog | MBT only |
| `VoicevoxModel.standard` | — | retracted (see above) |
| `Synthesis.*` (6) | — | replaced by `SynthesisRetry.qnt` (Phase 1) |

## Post-migration revisions

The following changes were made after the initial port to remove
model↔implementation drift (they are reflected in `verify.sh`, the `*.qnt`
files, and `STATE_TRACEABILITY.md`):

- **Bounds now track production constants.** `Daemon` and `MCPServer` connect to
  10 (`MAX_CONNECT_ATTEMPTS`), `StartupResources`/`Download` use 3
  (`download_missing_resources`), `Say` uses 10, and `System` separates resource
  retries (3), synth retries (2), and connect attempts (10).
- **`MCPServer` `Degraded` removed.** Production has no persistent degraded
  state; a failed connect episode is terminal (`ConnectFailed`) and a new request
  resets. The final connect after the retry loop is explicit
  (`finalConnectOk`/`finalConnectFail`).
- **Resource retry guard.** `beginLoad` only accepts `Missing`; re-entry after a
  failure goes through the budget-guarded `retryLoad`, so the retry counter is an
  actual bound rather than a saturated bookkeeping value.
- **Tautological properties replaced.** `readyHasNoPendingRetry` /
  `readyIsStable` / `readyIsBounded` were subsumed by `typeOK`; they are now
  `loadTerminates`, `loadedStaysReady`, and `bindingTerminates`.
  `canceledImpliesStoppedOrFailed` now refers to the `Canceled` error, not the
  pending `cancelRequested` flag.
- **`SynthesisParallel` → `DaemonSerialization`.** Cancel releasing the worker
  and daemon-side retry had no production counterpart. The model now encodes the
  serialized mutex with no cancel/retry.
- **`SynthesisRetry` attempt/backoff fidelity.** An in-flight `Attempting` state
  and `cancelIsTerminal` were added; `backoffs` is now counted at backoff
  *start*, matching `backoffs_started`. The module is scoped to the
  non-streaming path.
- **`StreamingSynthesis` added.** The default MCP streaming path
  (`default_streaming() == true`) is now modelled and MBT-checked.
- **`System` failure handling and retracted readiness gating.** `synthFatalFail`
  allows early non-retryable failure, and `synthBackoffs` is counted at backoff
  start. `daemonLost` resets the client view and connect budget but does **not**
  cancel synthesis: production auto-starts the daemon, so
  `synthRunningImpliesDaemonReady` / `synthBackoffImpliesDaemonReady` and the
  `SynthesisNeedsDaemon` gating are retracted (see Phase 1 contract change 3).
- **Connect-budget reset per episode.** `MCPServer` and `System` reset the
  attempt counter when a connected client loses the daemon, because each new
  request starts a fresh `connect_with_retry` budget.
- **Streaming failure/cancellation coverage.** `StreamingSynthesis` now models
  connection failure and cancellation before connection.
- **`Download` preparation failure.** `Download.qnt` separates preparation
  failure (before any downloader invocation) from retry exhaustion and counts
  invocations, replacing the unsound `failedImpliesExhausted`.
- **`Daemon` recovery bound.** `daemonFail` / `alreadyRunningUnresponsive` are
  budget-guarded so the retry counter bounds actual recovery failures.
- **`IPC` transport boundary.** Encode/write failure, EOF, oversized frame, and
  protocol `Error` responses were added; the dead `MAX_TIMEOUTS` counter was
  removed.
- **New models for previously uncovered paths.** `Download`, `ModelLifecycle`,
  and `McpRequestParsing` (+ MBT) cover installer retries, per-request model
  load/unload, and request parsing.

## Assumptions carried over

- `TypeOK`: enum/boolean bounds are subsumed by Quint's type system and are not
  ported as explicit invariants; only numeric bounds (retry/timeout counters)
  are ported (e.g. `typeOK` for `retryCount`).
- Variant labels that clash with Quint builtins are renamed (e.g. IPC's `None`
  becomes `NoResponse` / `NoError`, since `None` is the `Option` builtin).
- `ASSUME MAX_RETRY ∈ Nat` / `MAX_ATTEMPTS ∈ Nat` become concrete `pure val`
  bounds per module (and instance parameters where a module has several cfg
  values, e.g. `Say` with `MAX_RETRY=0` and `2`).
- Fairness (`WF_vars(...)`) is expressed with `weakFair(action, vars)`, as in
  `SynthesisRetry.qnt`.
- Composition is **flattened** rather than using Quint module instances: the TLC
  verification backend cannot assign constants through `import M(N = 3) as M`
  (it reports "constant parameter N is not assigned a value"), and
  `--tlc-config` only accepts `maxHeap`/`stackSize`/`workers`, not constants.
  Each ported module therefore carries concrete `pure val` constants and inlines
  the composed state/actions. Properties are preserved; the INSTANCE structure is
  not.
- Quint does **not** implicitly frame unmentioned variables: every action must
  assign every state variable, or TLC reports "Successor state is not completely
  specified". Flattened ports therefore assign all variables explicitly.
- Fairness must stay per-component: `SynthesisParallel`'s `EventuallyLeavesBusyWorker`
  needs separate `weakFair` for the J1 and J2 progress sets. A single combined
  fairness set is satisfiable by progress on one job alone and does not hold.
- `Stutter`/`UNCHANGED` become explicit no-op actions where the TLA+ model
  relied on them being enabled.
