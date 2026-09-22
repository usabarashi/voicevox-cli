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
| `SynthesisParallel` | `TypeOK`, `AtMostOneSynthesizing`, `WorkerMatchesSynthesis`, `EventuallyLeavesBusyWorker` | preserve | `SynthesisParallel.qnt` (done) |
| `StartupResources` | `TypeOK`, `DaemonReadyRequiresDownloads`, `DaemonStartRequiresDownloads`, `DaemonReadyRequiresSocket` | preserve | `StartupResources.qnt` (done, flattened) |
| `MCPServer` | `TypeOK`, `ConnectedImpliesDaemonReady`, `DegradedImpliesNotConnected`, `PlayingRequiresAudio` | preserve | `MCPServer.qnt` (done, flattened) |
| `Say` | `TypeOK`, `SynthesizingImpliesBusyReq`, `BusyReqOwnedBySay`, `DoneHasNoError`, `PlaybackFailureOnlyInPlayMode` (+ `PlayingRequiresAudio`, `EmittingUsesPlayMode`) | preserve (flattened) | `Say.qnt` (done) |
| `System` | `TypeOK`, `ViewsAligned`, `ClientConnectedImpliesDaemonReady`, `SynthesisRunningImpliesDaemonReady` | preserve (adapt INSTANCE to `SynthesisRetry`) | `System.qnt` (pending; see "System adaptation") |
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
must:

- synchronize `clientDaemonState` / `synthDaemonReady` from `fsDaemonState`
  (unchanged), and
- drive `SynthesisRetry` without `daemonReady` gating; `SynthesisRunningImpliesDaemonReady`
  becomes vacuous for the refined model and is replaced by "an attempt only
  starts while the environment is ready", which is an environment assumption,
  not an invariant. Record the change.

## Scenario mapping (cfg → Quint)

The old `cfg` selects constants, a spec variant, invariants, and temporal
properties. In Quint these become `init`/`step` plus `--invariant` /
`--temporal` selections; constants become module values or instance parameters.

| cfg scenario | Module | Constants | Invariants / properties |
|---|---|---|---|
| `ONNXRuntime.load` | ONNXRuntime | `MAX_RETRY=3` | `typeOK`, `readyHasNoPendingRetry` |
| `Dictionary.load` | Dictionary | `MAX_RETRY=3` | `typeOK`, `readyIsStable` |
| `Socket.bind` | Socket | `MAX_RETRY=3` | `typeOK`, `readyIsBounded` |
| `Playback.standard` | Playback | — | `typeOK`, `playingRequiresAudio`, `canceledImpliesStoppedOrFailed` |
| `IPC.safety` | IPC | `MAX_TIMEOUTS=3` | `typeOK`, `failedImpliesError`, `doneImpliesValidResponse` |
| `IPC.progress` | IPC | `MAX_TIMEOUTS=3` | `typeOK` + temporal `eventuallyLeavesInFlight` |
| `Daemon.startup` | Daemon | `MAX_RETRY=3` | `typeOK`, `socketImpliesReady`, `busyImpliesReady`, `alreadyRunningNotBusy`, `retryBounded` |
| `SynthesisParallel.safety` | SynthesisParallel | `MAX_RETRY=2` | `typeOK`, `atMostOneSynthesizing`, `workerMatchesSynthesis` |
| `SynthesisParallel.progress` | SynthesisParallel | `MAX_RETRY=2` | same + temporal `eventuallyLeavesBusyWorker` |
| `FirstStartup.bootstrap` | StartupResources | `MAX_RETRY=2` | `typeOK`, `daemonReadyRequiresDownloads`, `daemonStartRequiresDownloads`, `daemonReadyRequiresSocket` |
| `MCPServer.connect` | MCPServer | `MAX_ATTEMPTS=3` | `typeOK`, `connectedImpliesDaemonReady`, `degradedImpliesNotConnected`, `playingRequiresAudio` |
| `MCPServer.degraded` | MCPServer | `MAX_ATTEMPTS=3` | same as above |
| `Say.standard` | Say | `MAX_RETRY=0` | `typeOK`, `doneHasNoError`, `playbackFailureOnlyInPlayMode` |
| `Say.daemon` | Say | `MAX_RETRY=2` | `typeOK`, `synthesizingImpliesBusyReq`, `busyReqOwnedBySay`, `doneHasNoError` |
| `System.integration` | System | `MAX_RETRY=2`, `MAX_ATTEMPTS=3` | `typeOK`, `viewsAligned`, `clientConnectedImpliesDaemonReady`, `synthesisRunningImpliesDaemonReady` |
| `VoicevoxModel.standard` | VoicevoxModel | `MAX_RETRY=2` | retracted (see above) |
| `Synthesis.*` (6) | Synthesis | `MAX_RETRY=1..3` | replaced by `SynthesisRetry.qnt` (Phase 1) |

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

## Exit criteria for Phase 2

- Every preserved property has a Quint equivalent that verifies green with
  `quint verify --backend=tlc`.
- The correspondence table above is updated to mark each row `done`.
- Only then are `modeling/tla`, `modeling/cfg`, the `tla-model-check` job, and
  `tlaplus` from the devShell removed.
