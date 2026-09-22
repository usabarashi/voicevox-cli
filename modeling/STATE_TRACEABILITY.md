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
| ONNX runtime resource | `ONNXRuntime.qnt` | one-shot load (production has no load retry; the installer retries) | `loadTerminates`, `readyIsStable` (temporal) |
| OpenJTalk dictionary | `Dictionary.qnt` | one-shot load | `loadTerminates`, `loadedStaysReady` (temporal) |
| Socket binding/readiness | `Socket.qnt` | one-shot bind, ready, permission-denied (terminal), socket drop | `bindingTerminates`, `permissionDeniedIsTerminal` (temporal) |
| MCP client connect/playback | `MCPServer.qnt` | `startConnect`, `connectOk`, `connectRetry`, `finalConnectOk/Fail`, `connectFailed` (`MAX_ATTEMPTS = 10`), playback | `typeOK`, `connectedImpliesDaemonReady`, `playingRequiresAudio`; connect budget via `mbt/tests/mcp_connect.rs` |
| Synthesis retry/cancel loop (non-streaming) | `SynthesisRetry.qnt` | `Running/Attempting/Backoff/Done/Failed/Canceled`, `attempts` (started), `backoffs` (started) | `attemptsBounded`, `backoffsBounded`, `backoffAfterAttempt`, `eventuallyTerminal`, `cancelIsTerminal` |
| Streaming synthesis (default MCP path) | `StreamingSynthesis.qnt` | connect (or connect failure) → split → per-segment synthesis → concatenate → play; cancel before/after connect and at any point before playback; fail | `segmentsBounded`, `playbackRequiresAllSegments`, `canceledImpliesNoPlayback` |
| Daemon synthesis serialization (MBT) | `DaemonSerialization.qnt` | one worker (mutex), queued jobs, no cancel, no daemon-side retry | `atMostOneSynthesizing`, `workerMatchesSynthesis`, `eventuallyLeavesBusyWorker`; `mbt/tests/daemon_serialization.rs` (two concurrent real-daemon requests) |
| Daemon IPC server | `DaemonServer.qnt` | per-client accept/handle/finish, shared `MAX_IN_FLIGHT = 32` permits, `MAX_CONNECTIONS = 32` accept permits, idle-timeout close | `typeOK`, `inFlightMatchesHandling`, `connectionsMatchClient`, `handling{0,1,2}Terminates` (temporal) |
| Daemon startup / duplicate prevention | `DaemonStartup.qnt` | absent/stale/live socket scenarios; probe, TOCTOU re-check, stale removal, start | `liveNeverRemoved`, `liveNeverStarted`, `removedOnlyStale`, `staleRemovedBeforeStart`, `decides` (temporal) |
| MCP request lifecycle | `McpRequestLifecycle.qnt` | admit/complete/cancel, `MAX_CONCURRENT = 4` slots, busy rejection, `cancelAll` on disconnect | `typeOK`, `activeMatchesRunning`, `allRequestsTerminate` (temporal) |
| MCP daemon startup / recovery | `McpStartup.qnt` | first attempt (started / already-running / error), single recovery, non-fatal failure | `doneHasOutcome`, `recoveryOnlyAfterAlreadyRunning`, `terminates` (temporal) |
| IPC transport contract | `IPC.qnt` | request/response with encode/write/corrupt/mismatch/timeout/EOF/frame-limit/protocol-error | `failedImpliesError`, `doneImpliesValidResponse`, `inFlightHasNoError`, `eventuallyLeavesInFlight` |
| Playback (MBT) | `Playback.qnt` | launch/playing/stop/cancel/fail | `playingRequiresAudio`, `canceledImpliesStoppedOrFailed` (about the `Canceled` error); emit/play dispatch via `mbt/tests/playback.rs` (fake backend) |
| Say command flow | `Say.qnt` | validate → synthesize → emit with daemon + playback; `Play/WriteFile/Silent` output, early failures | `synthesizingImpliesBusyReq`, `busyReqOwnedBySay`, `doneHasNoError`, `playbackFailureOnlyInPlayMode`, `outputFailureOnlyInFileMode`, `playingRequiresAudio`, `emittingUsesPlayMode` |
| Download/install (MBT) | `Download.qnt` | preparation failure, downloader invocations with cleanup, give-up (`MAX_ATTEMPTS = 3`) | `attemptsBounded`, `failedHasReason`, `exhaustedImpliesAttempts`, `preparationFailureMeansNoAttempts`, `terminates` (temporal); retry loop via `mbt/tests/download.rs` |
| Daemon model load/unload (MBT) | `ModelLifecycle.qnt` | load → synthesize → guard unload, incl. failure paths | `loadedImpliesPhase`, `eventuallyUnloaded` (temporal); `mbt/tests/model_lifecycle.rs` (production executor + recording fake runtime) |
| Integrated end-to-end | `System.qnt` | startup resources + daemon (incl. loss) + client view/connect budget + environment-driven synthesis | `viewsAligned`, `clientConnectedImpliesDaemonReady`, `typeOK` |
| Target resolution (MBT) | `TargetResolution.qnt` | style/model/no-style/unknown resolution, collision | `mbt/tests/target_resolution.rs` (Quint Connect) |
| Retry arithmetic (MBT) | `SynthesisRetry.qnt` | attempt/backoff/cancel transitions | `mbt/tests/synthesis_retry.rs` (Quint Connect) |
| MCP request parsing (MBT) | `McpRequestParsing.qnt` | initialize/tools-list/tools-call/invalid/unknown | `mbt/tests/mcp_request_parsing.rs` (Quint Connect) |
| MCP notification parsing (MBT) | `McpNotificationParsing.qnt` | initialized/cancelled/unknown | `mbt/tests/mcp_notification_parsing.rs` (Quint Connect) |
| IPC transport (MBT) | `IPC.qnt` | valid / corrupt / mismatch / EOF / protocol-error responses via a fake server | `mbt/tests/ipc_transport.rs` (Quint Connect) |
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

## Cross-spec consistency

The specs are **per-concern models**, not one composed top-level model:
`System.qnt` composes startup resources + daemon + client + synthesis, but the
other protocols (daemon startup/duplicate prevention, daemon server admission,
MCP request lifecycle, startup recovery) are separate. There is therefore no
single proof that the layers fit together; their integration is a reviewed
contract, listed here.

What *is* machine-checked:

- **Shared constants** are asserted by `verify.sh` from
  `modeling/quint/EXPECTED_CONSTANTS`, so a value changed in one spec but not the
  others (e.g. the connect budget) fails the gate. That file also records the
  production constant each value maps to.
- **Attempt vs retry semantics** are not interchangeable: `MAX_RETRY` counts
  retries (attempts = 1 + `MAX_RETRY`, e.g. the installer-side specs), while
  `MAX_ATTEMPTS` counts total attempts (`Download.qnt`, `MCPServer.qnt`). Mixing
  them is an off-by-one hazard; see the note in `EXPECTED_CONSTANTS`.
- **Every spec is classified** (`MODEL_CLASSIFICATION`) as `mbt` or
  `verified-only`; `verify.sh` rejects an unclassified spec or an `mbt` spec with
  no driver.

Layer ownership:

| Layer | Spec(s) |
|---|---|
| Process / daemon lifecycle | `Daemon.qnt`, `DaemonStartup.qnt` |
| IPC server (admission / connections) | `DaemonServer.qnt`, `DaemonSerialization.qnt` |
| IPC transport (client) | `IPC.qnt`, `DaemonIpc.qnt`, `MCPServer.qnt` |
| MCP server (stdio) | `McpRequestParsing.qnt`, `McpNotificationParsing.qnt`, `McpRequestLifecycle.qnt`, `McpStartup.qnt` |
| Startup resources | `ONNXRuntime.qnt`, `Dictionary.qnt`, `Socket.qnt`, `StartupResources.qnt`, `Download.qnt` |
| Synthesis | `SynthesisRetry.qnt`, `StreamingSynthesis.qnt`, `TargetResolution.qnt`, `ModelLifecycle.qnt` |
| Integrated | `System.qnt`, `Say.qnt` |

Design-only specs (not derived from production code; correspondence is a design
decision, not an observed behavior): `Daemon.qnt`, `ONNXRuntime.qnt` (one-shot
load only), `Dictionary.qnt`, `Socket.qnt`, `Say.qnt`, `System.qnt`.

## Verification gate

`modeling/quint/verify.sh` typechecks every spec and runs the checks above with
`quint verify --backend=tlc`, plus two negative controls
(`negative/SafetyViolation.qnt`, `negative/LivenessViolation.qnt`) that must be
detected. Model-based tests run separately (`mbt/`).

## Model-based testing

`modeling/quint/MODEL_CLASSIFICATION` is the machine-checked source of truth for
which specs are MBT-backed (`mbt`) and which are `verified-only`. `verify.sh`
fails if a spec is unclassified, has an unknown tier, or is marked `mbt` without
a driver referencing it. Only drivers with an executable correspondence to
production code count as refinement evidence:

| Driver | Production entry point |
|---|---|
| `mbt/tests/target_resolution.rs` | `catalog::resolve_target` + `catalog::build_model_default_style_map` |
| `mbt/tests/synthesis_retry.rs` | `domain::synthesis::retry::RetryTracker` (production retry state machine), one step per spec action |
| `mbt/tests/synthesis_retry_loop.rs` | `run_retry_loop` with scripted attempt/waiter seams (orchestration + counters) |
| `mbt/tests/mcp_request_parsing.rs` | `mcp_server::protocol::parse_request_message` |
| `mbt/tests/mcp_notification_parsing.rs` | `mcp_server::protocol::parse_notification_message` |
| `mbt/tests/ipc_transport.rs` | `DaemonClient` against a fake Unix-socket server (no daemon) |
| `mbt/tests/model_lifecycle.rs` | `DaemonSynthesisExecutor` + `SerializedSynthesisPolicy` with a recording fake `ModelRuntime` |
| `mbt/tests/download.rs` | `install_with_retries` + `DownloadTracker` with a scripted fake `ResourceInstaller` |
| `mbt/tests/mcp_connect.rs` | `retry_with_final` (behind `connect_with_retry`) with a counting fake `ConnectAttempt` |
| `mbt/tests/playback.rs` | `emit_and_play_with_backend` with a scripted fake `AudioPlayback` |
| `mbt/tests/daemon_ipc.rs` | `DaemonClient` over a real daemon |
| `mbt/tests/daemon_synthesize.rs` | `DaemonClient::synthesize` over a real daemon |
| `mbt/tests/daemon_serialization.rs` | two concurrent `DaemonClient::synthesize` over a real daemon |
| `mbt/tests/streaming_synthesis.rs` | `StreamingSynthesizer` + `TextSplitter` + `concatenate_wav_segments` over a real daemon; plus daemon-free `handle_text_to_speech_cancellable` connect-failure/cancel scenarios (dead `VOICEVOX_SOCKET_PATH`) |

Scope note: `synthesis_retry.rs` drives the production `RetryTracker` one step
per spec action over random traces (attempts/backoffs counted at start,
cancellation terminal). `synthesis_retry_loop.rs` runs the real `run_retry_loop`
with injected attempt/waiter seams and checks the observed counters and terminal
outcome for fixed scenarios (exhaustion, success after retry, early fatal,
cancel before the first attempt, cancel during backoff, cancel in flight). The
in-file fake-seam tests in `src/interface/mcp_server/tools/text_to_speech.rs`
remain as finer-grained unit evidence. `streaming_synthesis.rs` covers both the
successful pipeline against a real daemon and the connect-failure /
pre-connect-cancel paths daemon-free through the MCP entry point; the remaining
streaming failure/cancel interleavings are verified in `StreamingSynthesis.qnt`
but have no executable driver.

The remaining Lifecycle models (`Daemon`, `DaemonServer`, `DaemonStartup`,
`McpStartup`, `McpRequestLifecycle`, `StartupResources`, `ONNXRuntime`,
`Dictionary`, `Socket`, `Say`, `System`) are verified exhaustively but are
**not** executable
refinements; their correspondence is the prose above plus ordinary tests.

## Modelling boundary (not modelled)

Explicitly out of scope, with the reason:

- **Audio playback backends** (`interface/playback.rs`, `interface/audio.rs`):
  external player fallback, rodio output, and child-process cleanup depend on
  the host; `Playback.qnt` models only the abstract lifecycle.
- **MCP line framing / response correlation** (`server/stdio.rs`): the 256 KiB
  line limit and the 64-entry response queue are transport concerns; request and
  notification *parsing* are modelled and MBT-checked, and the concurrency /
  cancellation / termination lifecycle is modelled in `McpRequestLifecycle.qnt`
  (duplicate-request-ID double-response is not modelled).
- **IPC timeout** (`IPC.qnt` `ResponseTimeout`): the 30 s response timeout has no
  driver because a test cannot wait it out; the other IPC fault classes are
  MBT-checked in `ipc_transport.rs`.
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
