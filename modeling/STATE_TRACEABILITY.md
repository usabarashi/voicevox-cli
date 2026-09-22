# State Traceability (Quint)

TLA+ has been fully migrated to Quint. The handwritten TLA+ modules/configs and
the TLC CI job are gone; verification now runs through Quint's TLC backend
(`modeling/quint/verify.sh`), and the implementation-facing contracts are
connected to production code through Quint Connect MBT.

See also:

- `modeling/quint/CONTRACT.md` — verified contracts, Phase 1 status, toolchain notes.
- `modeling/quint/PORTING.md` — property-level correspondence from the old TLA+
  models (preserve / move / retract / replace) and the cfg → Quint mapping.

## State ownership

| Concern | Quint module | Core states/actions | Checked by |
|---|---|---|---|
| Daemon lifecycle | `Daemon.qnt` | `DaemonDown/Starting/AlreadyRunning/Ready/Recovering`, `startDaemon`, `daemonReady`, recovery transitions | `socketImpliesReady`, `busyImpliesReady`, `alreadyRunningNotBusy`, `retryBounded`, `typeOK` |
| Startup resources | `StartupResources.qnt` | runtime/dictionary/socket/model readiness + daemon bootstrap | `daemonReadyRequiresDownloads`, `daemonStartRequiresDownloads`, `daemonReadyRequiresSocket`, `typeOK` |
| ONNX runtime resource | `ONNXRuntime.qnt` | load/retry/fail transitions | `readyHasNoPendingRetry`, `typeOK` |
| Dictionary resource | `Dictionary.qnt` | load/retry/fail transitions | `readyIsStable`, `typeOK` |
| Socket binding/readiness | `Socket.qnt` | bind, ready, permission-denied, retry | `readyIsBounded`, `typeOK` |
| MCP client connect/playback view | `MCPServer.qnt` | `startConnect`, `connectOk`, `connectRetry`, `enterDegraded`, `leaveDegraded`, playback | `connectedImpliesDaemonReady`, `degradedImpliesNotConnected`, `playingRequiresAudio`, `typeOK` |
| Synthesis retry/cancel loop | `SynthesisRetry.qnt` | `Running/Backoff/Done/Failed/Canceled`, `attempts`, `backoffs` | `attemptsBounded`, `backoffsBounded`, `backoffAfterAttempt`, `eventuallyTerminal` |
| Parallel synthesis | `SynthesisParallel.qnt` | worker + two jobs, queue/start/finish/fail/cancel/reset | `atMostOneSynthesizing`, `workerMatchesSynthesis`, `eventuallyLeavesBusyWorker` |
| IPC contract behavior | `IPC.qnt` | request/response safety and progress | `failedImpliesError`, `doneImpliesValidResponse`, `eventuallyLeavesInFlight`, `typeOK` |
| Playback | `Playback.qnt` | launch/playing/stop/cancel/fail | `playingRequiresAudio`, `canceledImpliesStoppedOrFailed` |
| Say command flow | `Say.qnt` | validate → synthesize → emit with daemon + playback | `synthesizingImpliesBusyReq`, `busyReqOwnedBySay`, `doneHasNoError`, `playbackFailureOnlyInPlayMode`, `playingRequiresAudio`, `emittingUsesPlayMode` |
| Integrated end-to-end | `System.qnt` | startup resources + daemon + client + refined synthesis with view sync | `viewsAligned`, `clientConnectedImpliesDaemonReady`, `synthRunningImpliesDaemonReady`, `typeOK` |
| Target resolution (MBT) | `TargetResolution.qnt` | style/model/unknown resolution | `mbt/tests/target_resolution.rs` (Quint Connect) |

## Cross-module synchronization

`System.qnt` is the integration point and preserves these correspondences:

- daemon source of truth: `daemonState`
- client view: `clientDaemonState = if daemonState == DaemonReady then ViewDaemonReady else ViewDaemonDown` (`viewsAligned`)
- connected requires ready: `clientState == Connected => daemonState == DaemonReady`
  (`clientConnectedImpliesDaemonReady`)
- synthesis requires ready: attempts start only while the daemon is ready, and
  going not-ready cancels a `Running`/`Backoff` synthesis
  (`synthRunningImpliesDaemonReady`).

## Verification gate

`modeling/quint/verify.sh` typechecks every spec and runs the checks above with
`quint verify --backend=tlc`, plus two negative controls
(`negative/SafetyViolation.qnt`, `negative/LivenessViolation.qnt`) that must be
detected. Model-based tests run separately (`mbt/`).

## Rules

- Module-local details stay in each `*.qnt`.
- Integration consistency is checked in `System.qnt`.
- Scenario-specific configuration is expressed as `init`/`step` action pairs and
  `--invariant` / `--temporal` selections; regressions become fixed `run`s or
  MBT scenarios. See `PORTING.md` for the cfg → Quint mapping.
- If a new state is introduced, update the owning `*.qnt`, the `System.qnt`
  mapping (if shared), and add a `verify.sh` check that exercises it.
