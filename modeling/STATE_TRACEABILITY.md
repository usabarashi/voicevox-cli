# State Traceability (Quint)

TLA+ has been fully migrated to Quint. The handwritten TLA+ modules/configs and
the TLC CI job are gone; verification now runs through Quint's TLC backend
(`modeling/quint/verify.sh`), and the implementation-facing contracts are
connected to production code through Quint Connect MBT.

See also:

- `modeling/quint/CONTRACT.md` — verified contracts, the observation contract,
  and the recorded contract changes.
- `modeling/quint/PORTING.md` — property-level correspondence from the old TLA+
  models (preserve / move / retract / replace) and the cfg → Quint mapping.

## State ownership

| Concern | Quint module | Core states/actions | Checked by |
|---|---|---|---|
| Daemon lifecycle | `Daemon.qnt` | `DaemonDown/Starting/AlreadyRunning/Ready/Recovering`, recovery transitions (`MAX_RETRY = 10`) | `socketImpliesReady`, `busyImpliesReady`, `alreadyRunningNotBusy`, `retryBounded`, `typeOK` |
| Startup resources | `StartupResources.qnt` | runtime/dictionary/socket/model readiness + daemon bootstrap (`MAX_RETRY = 3`) | `daemonReadyRequiresDownloads`, `daemonStartRequiresDownloads`, `daemonReadyRequiresSocket`, `typeOK` |
| ONNX runtime resource | `ONNXRuntime.qnt` | load/retry/fail transitions | `typeOK`, `loadTerminates` (temporal) |
| Dictionary resource | `Dictionary.qnt` | load/retry/fail transitions | `typeOK`, `loadedStaysReady` (temporal) |
| Socket binding/readiness | `Socket.qnt` | bind, ready, permission-denied, retry | `typeOK`, `bindingTerminates` (temporal) |
| MCP client connect/playback | `MCPServer.qnt` | `startConnect`, `connectOk`, `connectRetry`, `finalConnectOk/Fail`, `connectFailed` (`MAX_ATTEMPTS = 10`), playback | `typeOK`, `connectedImpliesDaemonReady`, `playingRequiresAudio` |
| Synthesis retry/cancel loop (non-streaming) | `SynthesisRetry.qnt` | `Running/Attempting/Backoff/Done/Failed/Canceled`, `attempts` (started), `backoffs` (started) | `attemptsBounded`, `backoffsBounded`, `backoffAfterAttempt`, `eventuallyTerminal`, `cancelIsTerminal` |
| Streaming synthesis (default MCP path) | `StreamingSynthesis.qnt` | connect (or connect failure) → split → per-segment synthesis → concatenate → play; cancel before/after connect and at any point before playback; fail | `segmentsBounded`, `playbackRequiresAllSegments`, `canceledImpliesNoPlayback` |
| Daemon synthesis serialization | `DaemonSerialization.qnt` | one worker (mutex), queued jobs, no cancel, no daemon-side retry | `atMostOneSynthesizing`, `workerMatchesSynthesis`, `eventuallyLeavesBusyWorker` |
| IPC transport contract | `IPC.qnt` | request/response with encode/write/corrupt/mismatch/timeout/EOF/frame-limit/protocol-error | `failedImpliesError`, `doneImpliesValidResponse`, `inFlightHasNoError`, `eventuallyLeavesInFlight` |
| Playback | `Playback.qnt` | launch/playing/stop/cancel/fail | `playingRequiresAudio`, `canceledImpliesStoppedOrFailed` (about the `Canceled` error) |
| Say command flow | `Say.qnt` | validate → synthesize → emit with daemon + playback; `Play/WriteFile/Silent` output, early failures | `synthesizingImpliesBusyReq`, `busyReqOwnedBySay`, `doneHasNoError`, `playbackFailureOnlyInPlayMode`, `outputFailureOnlyInFileMode`, `playingRequiresAudio`, `emittingUsesPlayMode` |
| Download/install | `Download.qnt` | preparation failure, downloader invocations with cleanup, give-up (`MAX_ATTEMPTS = 3`) | `attemptsBounded`, `failedHasReason`, `exhaustedImpliesAttempts`, `preparationFailureMeansNoAttempts`, `terminates` (temporal) |
| Daemon model load/unload | `ModelLifecycle.qnt` | load → synthesize → guard unload, incl. failure paths | `loadedImpliesPhase`, `eventuallyUnloaded` (temporal) |
| Integrated end-to-end | `System.qnt` | startup resources + daemon (incl. loss) + client view/connect budget + environment-driven synthesis | `viewsAligned`, `clientConnectedImpliesDaemonReady`, `typeOK` |
| Target resolution (MBT) | `TargetResolution.qnt` | style/model/no-style/unknown resolution, collision | `mbt/tests/target_resolution.rs` (Quint Connect) |
| Retry arithmetic (MBT) | `SynthesisRetry.qnt` | attempt/backoff/cancel transitions | `mbt/tests/synthesis_retry.rs` (Quint Connect) |
| MCP request parsing (MBT) | `McpRequestParsing.qnt` | initialize/tools-list/tools-call/invalid/unknown | `mbt/tests/mcp_request_parsing.rs` (Quint Connect) |
| Real-daemon IPC (MBT) | `DaemonIpc.qnt` | client connect / catalog read over the socket | `mbt/tests/daemon_ipc.rs` (Quint Connect, `#[ignore]`; CI job `quint-mbt-daemon`) |
| Real-daemon synthesize (MBT) | `DaemonSynthesize.qnt` | connect → listSpeakers → synthesize | `mbt/tests/daemon_synthesize.rs` (Quint Connect, `#[ignore]`) |
| Real-daemon streaming (MBT) | `StreamingSynthesis.qnt` | connect → split → segment synthesis → concatenate | `mbt/tests/streaming_synthesis.rs` (Quint Connect, `#[ignore]`) |

## Cross-module synchronization

`System.qnt` is the integration point and preserves these correspondences:

- daemon source of truth: `daemonState`
- client view: `clientDaemonState = if daemonState == DaemonReady then ViewDaemonReady else ViewDaemonDown` (`viewsAligned`)
- connected requires ready: `clientState == Connected => daemonState == DaemonReady`
  (`clientConnectedImpliesDaemonReady`)
- synthesis is **environment-driven**: production auto-starts the daemon
  (`connect_daemon_client_auto_start`), so an attempt may run while the daemon is
  not ready, and a lost daemon is a retryable attempt failure rather than a
  cancellation. Readiness gating and cancel-on-daemon-loss
  (`SynthesisNeedsDaemon`, `SynthesisRunningImpliesDaemonReady` /
  `synthBackoffImpliesDaemonReady`) are **retracted** (consistent with Phase 1
  contract change 3). `daemonLost` only resets the client view and the connect
  budget.
- `synthBackoffs` counts backoff **starts**, matching production
  `backoffs_started` and `SynthesisRetry.qnt`.
- a non-retryable (`fatal`) attempt failure reaches `SynthFailed` from any
  attempt (`synthFatalFail`), not only the last one.
- losing a connection resets the connect budget: the next request calls
  `connect_with_retry` from the start (`attempt = 0`).

## Verification gate

`modeling/quint/verify.sh` typechecks every spec and runs the checks above with
`quint verify --backend=tlc`, plus two negative controls
(`negative/SafetyViolation.qnt`, `negative/LivenessViolation.qnt`) that must be
detected. Model-based tests run separately (`mbt/`).

## Model-based testing

Only drivers with an executable correspondence to production code count as
refinement evidence:

| Driver | Production entry point |
|---|---|
| `mbt/tests/target_resolution.rs` | `catalog::resolve_target` + `catalog::build_model_default_style_map` |
| `mbt/tests/synthesis_retry.rs` | `domain::synthesis::retry::RetryPolicy` + `MCP_DAEMON_MAX_RETRIES` |
| `mbt/tests/mcp_request_parsing.rs` | `mcp_server::protocol::parse_request_message` |
| `mbt/tests/daemon_ipc.rs` | `DaemonClient` over a real daemon |
| `mbt/tests/daemon_synthesize.rs` | `DaemonClient::synthesize` over a real daemon |
| `mbt/tests/streaming_synthesis.rs` | `StreamingSynthesizer` + `TextSplitter` + `concatenate_wav_segments` over a real daemon |

Scope note: `synthesis_retry.rs` covers the retry **decision policy**
(`RetryPolicy` + `MCP_DAEMON_MAX_RETRIES`), not `run_retry_loop` itself. The
loop's orchestration (attempt-start counting, cancellation checkpoints, the wait
seam) is covered by the in-file fake-seam tests in
`src/interface/mcp_server/tools/text_to_speech.rs`, which satisfy the observation
contract but are not Quint Connect drivers. `streaming_synthesis.rs` covers the
successful pipeline only; the streaming failure/cancellation paths are verified
in `StreamingSynthesis.qnt` but have no executable driver.

The remaining Lifecycle models (`Daemon`, `StartupResources`, `MCPServer`,
`Say`, `System`, `Playback`, `IPC`, `DaemonSerialization`, `Download`,
`ModelLifecycle`) are verified exhaustively but are **not** executable
refinements; their correspondence is the prose above plus ordinary tests.

## Modelling boundary (not modelled)

Explicitly out of scope, with the reason:

- **Audio playback backends** (`interface/playback.rs`, `interface/audio.rs`):
  external player fallback, rodio output, and child-process cleanup depend on
  the host; `Playback.qnt` models only the abstract lifecycle.
- **MCP line framing / response correlation** (`server/stdio.rs`): the 256 KiB
  line limit, the 64-entry response queue, and cancellation routing are
  transport concerns; only request *parsing* is modelled and MBT-checked.
- **Daemon-side retry**: the daemon returns an error; retries belong to the
  client (`SynthesisRetry.qnt`).
- **Clock/time**: timeout values (30 s response, backoff delays) are modelled as
  inputs, not as real time.
- **Kani**: numeric proofs (rate bounds, style-id bounds, WAV chunk arithmetic)
  are covered separately with `cargo kani`.

These boundaries are the honest limit of the state traceability claim: a spec
being green establishes the properties listed above, not end-to-end
correspondence for paths outside the table.

## Rules

- Module-local details stay in each `*.qnt`.
- Integration consistency is checked in `System.qnt`.
- Scenario-specific configuration is expressed as `init`/`step` action pairs and
  `--invariant` / `--temporal` selections; regressions become fixed `run`s or
  MBT scenarios. See `PORTING.md` for the cfg → Quint mapping.
- If a new state is introduced, update the owning `*.qnt`, the `System.qnt`
  mapping (if shared), and add a `verify.sh` check that exercises it.
- A new production path should either get a model + MBT or be added to the
  "Modelling boundary" section above.
