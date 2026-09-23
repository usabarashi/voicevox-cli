# State Traceability (Quint)

The models under `modeling/quint/` are checked with the TLC verification backend
of `quint` (`modeling/quint/verify.sh`), and the implementation-facing contracts
are connected to production code through Quint Connect model-based tests
(`mbt/`). This document is the map from each production concern to its model and
its evidence.

## Toolchain

| Tool | Version | Source | Notes |
|---|---|---|---|
| `quint` | 0.32.0 | `nixpkgs` (pinned by `flake.lock`) | on `PATH` in the devShell (`flake.nix`) |
| TLC backend | TLC 2.19 (through `quint verify --backend=tlc`) | bundled with the `quint` package | JRE 21 bundled as well; **no external Java or TLA+ install required** |

The TLC backend is used because the suite relies on exhaustive finite-state
exploration and fairness-based liveness. Apalache (the default backend) is
bounded, and `quint run --invariant` is random simulation; neither replaces TLC.

## State ownership

| Concern | Quint module | Core states/actions | Checked by |
|---|---|---|---|
| Daemon lifecycle | `Daemon.qnt` | `DaemonDown/Starting/AlreadyRunning/Ready/Recovering`, recovery transitions (`MAX_RETRY = 10`) | `socketImpliesReady`, `busyImpliesReady`, `alreadyRunningNotBusy`, `retryBounded`, `typeOK` |
| Startup resources / socket | `StartupResources.qnt` | runtime/dictionary/socket/model readiness + one-shot socket bind/drop + daemon bootstrap (`MAX_RETRY = 3`) | `daemonReadyRequiresDownloads`, `daemonStartRequiresDownloads`, `daemonReadyRequiresSocket`, `typeOK`, `bindingTerminates`, `permissionDeniedIsTerminal` (temporal) |
| Startup safety / duplicate prevention (composed) | `StartupSafety.qnt` | resources readiness × socket scenario (absent/stale/live) × start ordering (no bind or TOCTOU re-probe modelled) | `readyRequiresResources`, `readyRequiresStartableSocket`, `startedImpliesNoLive`, `liveNeverRemoved`, `liveNeverStarted`, `staleRemovedBeforeStart`, `removedOnlyStale`, `alreadyRunningOnlyLive`, `failedImpliesResourceFailure`, `terminates` (temporal) |
| Startup resource load | `ResourceLoad.qnt` | one-shot load of ONNX Runtime / OpenJTalk dictionary (production has no load retry; the installer retries) | `loadTerminates`, `loadedStaysReady` (temporal) |
| MCP client connect/playback | `MCPServer.qnt` | `startConnect`, `connectOk`, `connectRetry`, `finalConnectOk/Fail`, `connectFailed` (`MAX_ATTEMPTS = 10`), playback | `typeOK`, `connectedImpliesDaemonReady`, `playingRequiresAudio`; connect budget via `mbt/tests/mcp_connect.rs` |
| Synthesis retry/cancel loop (non-streaming) | `SynthesisRetry.qnt` | `Running/Attempting/Backoff/Done/Failed/Canceled`, `attempts` (started), `backoffs` (started) | `attemptsBounded`, `backoffsBounded`, `backoffAfterAttempt`, `eventuallyTerminal`, `cancelIsTerminal` |
| Streaming synthesis (default MCP path) | `StreamingSynthesis.qnt` | connect (or connect failure) → split → per-segment synthesis → concatenate → play; cancel before/after connect and at any point before playback; fail | `segmentsBounded`, `playbackRequiresAllSegments`, `canceledImpliesNoPlayback` |
| Daemon synthesis serialization (MBT) | `DaemonSerialization.qnt` | one worker (mutex), queued jobs, no cancel, no daemon-side retry | `atMostOneSynthesizing`, `workerMatchesSynthesis`, `eventuallyLeavesBusyWorker`; `mbt/tests/daemon_serialization.rs` (two concurrent real-daemon requests) |
| Daemon request path (composed) | `DaemonSynthesisPath.qnt` | server admission (verify scale `MAX_IN_FLIGHT = 2`, production `PRODUCTION_MAX_IN_FLIGHT = 32`) × serialized worker | `inFlightMatchesHolding`, `atMostOneSynthesizing`, `workerBusyMatchesSynthesizing`, `workerEventuallyIdle` (temporal) |
| Daemon IPC server | `DaemonServer.qnt` | per-client accept/handle/finish, shared request permits (verify scale `MAX_IN_FLIGHT = 2`, production `PRODUCTION_MAX_IN_FLIGHT = 32`), idle-timeout close | `typeOK`, `inFlightMatchesHandling`, `handling{0,1,2}Terminates` (temporal) |
| MCP request lifecycle | `McpRequestLifecycle.qnt` | admit/complete, two-step cancel (`Cancelling` -> `Cancelled`), busy rejection, `cancelAll`; verify scale `MAX_CONCURRENT = 2` (production `PRODUCTION_MAX_CONCURRENT = 4`) | `typeOK`, `activeMatchesHolding`, `allRequestsTerminate` (temporal, non-vacuous) |
| MCP daemon startup / recovery | `McpStartup.qnt` | first attempt (started / already-running / error), single recovery, non-fatal failure | `doneHasOutcome`, `recoveryOnlyAfterAlreadyRunning`, `terminates` (temporal) |
| IPC transport contract | `IPC.qnt` | request/response with encode/write/corrupt/mismatch/timeout/EOF/frame-limit/protocol-error | `failedImpliesError`, `doneImpliesValidResponse`, `inFlightHasNoError`, `eventuallyLeavesInFlight` |
| Playback (MBT) | `Playback.qnt` | launch/playing/stop/cancel/fail | `playingRequiresAudio`, `canceledImpliesStoppedOrFailed` (about the `Canceled` error); emit/play dispatch via `mbt/tests/playback.rs` (fake backend) |
| Say command flow | `Say.qnt` | validate → synthesize → emit with daemon + playback; `Play/WriteFile/Silent` output, early failures | `synthesizingImpliesBusyReq`, `busyReqOwnedBySay`, `doneHasNoError`, `playbackFailureOnlyInPlayMode`, `outputFailureOnlyInFileMode`, `playingRequiresAudio`, `emittingUsesPlayMode` |
| Download/install (MBT) | `Download.qnt` | preparation failure, downloader invocations with cleanup, give-up (`MAX_ATTEMPTS = 3`) | `attemptsBounded`, `failedHasReason`, `exhaustedImpliesAttempts`, `preparationFailureMeansNoAttempts`, `terminates` (temporal); retry loop via `mbt/tests/download.rs` |
| Model presence (MBT) | `ModelPresence.qnt` | models directory `Missing`/`Ready` × `missing_startup_resources` report | `reportMatchesPresence` (verify.sh + `mbt/tests/model_presence.rs`, Quint Connect over a real filesystem fixture) |
| Daemon model load/unload (MBT) | `ModelLifecycle.qnt` | load → synthesize → guard unload, incl. failure paths | `loadedImpliesPhase`, `eventuallyUnloaded` (temporal); `mbt/tests/model_lifecycle.rs` (production executor + recording fake runtime) |
| Integrated end-to-end | `System.qnt` | startup resources + daemon (incl. loss) + client view/connect budget + environment-driven synthesis | `viewsAligned`, `clientConnectedImpliesDaemonReady`, `daemonStartingRequiresResources`, `daemonReadyRequiresResources`, `daemonReadyRequiresSocket`, `typeOK` |
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
  cancellation. `daemonLost` only resets the client view and the connect budget.
- `synthBackoffs` counts backoff **starts**, matching production
  `backoffs_started` and `SynthesisRetry.qnt`.
- a non-retryable (`fatal`) attempt failure reaches `SynthFailed` from any
  attempt (`synthFatalFail`), not only the last one.
- losing a connection resets the connect budget: the next request calls
  `connect_with_retry` from the start (`attempt = 0`).
- the four startup resources are encoded as a map indexed 0..3 (0=runtime,
  1=dictionary, 2=socket, 3=model). A resource that exhausts its retry budget is
  terminal (re-entry is only through the budget-guarded `retryLoad`).

## Cross-spec consistency

The specs are **per-concern models**, not one composed top-level model:
`System.qnt` composes startup resources + daemon + client + synthesis;
`StartupSafety.qnt` composes the startup path (resources × socket scenario ×
start ordering); `DaemonSynthesisPath.qnt` composes server admission × the
serialized worker. The remaining protocols (MCP request lifecycle, startup
recovery) are separate. There is therefore no single proof that *all* layers fit
together; their integration is a reviewed contract, listed here.

What *is* machine-checked:

- **Shared constants** are asserted on **both sides** by `verify.sh` from
  `modeling/quint/EXPECTED_CONSTANTS`: each `spec|...` line checks the spec's
  `pure val`, and each `prod|...` line checks the exact production declaration
  substring. Drift on either side (a model constant or a production constant)
  fails the gate.
- **Attempt vs retry semantics** are not interchangeable: `SynthesisRetry`
  `MAX_RETRIES` counts retries (attempts = 1 + `MAX_RETRIES`), while
  `Download.MAX_ATTEMPTS` and `StartupResources.MAX_RETRY` bound total attempts.
  Mixing them is an off-by-one hazard; see the note in `EXPECTED_CONSTANTS`.
- **Verification-scale constants**: `McpRequestLifecycle`, `DaemonServer`, and
  `DaemonSynthesisPath` use a small `MAX_*` so their admission limits are
  reachable with the tiny ID sets (otherwise the guard and the upper-bound
  invariant would be vacuous). The production value is declared as
  `PRODUCTION_*` and asserted by `EXPECTED_CONSTANTS`; do not compare the scale
  constant to production.
- **Every spec is classified** (`MODEL_CLASSIFICATION`) as `mbt` or
  `verified-only`; `verify.sh` rejects an unclassified spec or an `mbt` spec with
  no driver.

Layer ownership:

| Layer | Spec(s) |
|---|---|
| Process / daemon lifecycle | `Daemon.qnt`, `StartupSafety.qnt` |
| IPC server (admission / connections / worker) | `DaemonServer.qnt`, `DaemonSerialization.qnt`, `DaemonSynthesisPath.qnt` |
| IPC transport (client) | `IPC.qnt`, `DaemonIpc.qnt`, `MCPServer.qnt` |
| MCP server (stdio) | `McpRequestParsing.qnt`, `McpNotificationParsing.qnt`, `McpRequestLifecycle.qnt`, `McpStartup.qnt` |
| Startup resources / ordering | `ResourceLoad.qnt`, `StartupResources.qnt`, `Download.qnt`, `ModelPresence.qnt`, `StartupSafety.qnt` |
| Synthesis | `SynthesisRetry.qnt`, `StreamingSynthesis.qnt`, `TargetResolution.qnt`, `ModelLifecycle.qnt` |
| Integrated | `System.qnt`, `Say.qnt` |

Design-only specs (not derived from production code; correspondence is a design
decision, not an observed behavior): `Daemon.qnt`, `ResourceLoad.qnt` (one-shot
load only), `Say.qnt`, `System.qnt`.

## Verification gate

`modeling/quint/verify.sh` typechecks every spec, then runs the safety and
liveness checks above with `quint verify --backend=tlc`, and finally asserts that
two negative controls are detected for the intended reason:

- `negative/SafetyViolation.qnt`: unbounded attempts → `attemptsBounded`
  violated (safety detection).
- `negative/LivenessViolation.qnt`: an always-enabled `skip` with no fairness →
  `eventuallyTerminal` violated by an infinite stall, not by a deadlock
  (liveness detection, including the stuttering counterexample).

The gate also enforces the cross-spec constant checks and model classification
described above. Run it with:

```bash
nix develop --accept-flake-config --command bash modeling/quint/verify.sh
```

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
| `mbt/tests/model_presence.rs` | `has_available_models` + `missing_startup_resources` against a real filesystem fixture, evaluated in a `model_presence_probe` child process configured via `VOICEVOX_MODELS_DIR` |
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

The remaining Lifecycle models (`Daemon`, `DaemonServer`, `DaemonSynthesisPath`,
`McpStartup`, `McpRequestLifecycle`, `StartupResources`, `ResourceLoad`,
`Say`, `System`) are verified exhaustively but are
**not** executable refinements; their correspondence is the prose above plus
ordinary tests.

### Observation contract

- **Counting and simultaneity.** At most 3 attempt *starts* (initial 1 + up to 2
  retries); at most 2 backoff *starts*; retries only for retryable errors.
  Cancellation is evaluated before each attempt and while waiting for a backoff.
  During the backoff wait, a **delivered** cancellation takes priority and
  returns without waiting out the timer. In-flight synthesis cancellation
  priority is **not** guaranteed for `synthesize_bytes_via_daemon_cancellable`
  (`flow.rs`), whose `select!` is unbiased; when cancellation and completion are
  ready at the same instant, either outcome is allowed there. The production
  backoff wait checks an already-delivered cancellation first, then races with a
  biased select (cancel branch first); do not apply a blanket bias to every
  `select!`.
- **Injected inputs / observed outputs.** Inject cancel timing (before attempt /
  during backoff / during the in-flight wait) and the attempt result sequence
  (Ok / retryable error / non-retryable error), and release the controllable wait
  explicitly (do **not** rely on `tokio::time::pause()` auto-advance). Observe
  attempt-start count, backoff-start count, terminal outcome, that **no further
  attempt starts after cancellation**, and the component's phase via its
  production accessors. Never copy the expected model state into the
  "implementation state".
- **Boundary.** A single attempt is the whole client-side flow
  (`Validate → EnsureResources → Connect → Synthesize`). The daemon is an
  environment returning `Ok(wav)` or a retryable/non-retryable error; daemon
  internals and connect auto-start are environment. Cancellation means
  client-side wait termination only; it does not stop already-sent daemon work or
  roll back side effects. The outer error classification
  (`interface/mcp_server/daemon_error.rs`) is exercised **separately with real
  errors**, because MBT injects pre-classified results.

### Exploration and reproduction

- `#[quint_run]` uses a random seed unless `QUINT_SEED` is set. quint-connect
  reads `QUINT_SEED` at **compile time** (`option_env!`), so changing it requires
  recompiling the `mbt` crate.
- CI sets `QUINT_SEED=0x5eed` (see `.github/workflows/ci.yml`) for deterministic
  traces; leave it unset locally for broader exploration.
- On failure, quint-connect prints the seed (`Reproduce this error with
  QUINT_SEED=...`) and, with `QUINT_VERBOSE=1|2`, the trace. quint-connect 0.1.2
  does not persist traces to disk, so reproduction is seed-based:
  `nix develop --accept-flake-config --command bash -c 'QUINT_SEED=<printed> cargo test --locked --manifest-path mbt/Cargo.toml -- --nocapture'`
  (`quint` and the Rust toolchain come from the devShell).
- `max_samples` / `max_steps` are set explicitly in the test attributes
  (200 / 6 for the simulation, 1 / 1 for the fixed scenarios), because supplying
  a seed also changes quint's default sample count.

`mbt` is a member of the root Cargo workspace (shared `Cargo.lock`, which keeps
its dependency resolution identical to the shipped binaries) but is excluded
from `default-members`, so the normal `cargo build`/`cargo test` and the
`nix flake check` / crane sandbox never need `quint`. Run the non-daemon MBT
explicitly with:

```bash
nix develop --accept-flake-config --command bash -c \
  'cargo test --locked --manifest-path mbt/Cargo.toml'
```

The daemon-backed MBT (`daemon_ipc`, `daemon_synthesize`,
`daemon_serialization`, `streaming_synthesis`) is `#[ignore]`d and run by the
`quint-mbt-daemon` CI job, which provisions VOICEVOX resources with
`voicevox-download` (feeding `yes` for its license agreement, since the
`voicevox-setup` wrapper's interactive prompt does not work with piped stdin)
into the default XDG location, caches them, and runs the ignored tests with
`-- --ignored`. The daemon socket must live in a directory owned by the user with
mode 0700 (the daemon rejects group/world-accessible parents); the drivers create
such a directory and an empty temporary models directory by default.
`VOICEVOX_MBT_MODELS_DIR` (optional) points at real models; `VOICEVOX_DAEMON_BIN`
overrides the daemon binary path.

The `quint-mbt-explore` CI job runs the non-daemon MBT with a random seed
(`QUINT_SEED` unset) on a nightly schedule, because the PR/push jobs pin the
seed for deterministic traces.

### Quint Connect toolchain limitation

`#[quint_test]` generates traces with `quint test ... --out-itf`, and quint
0.32.0's `quint test` has **no `--mbt` flag** and emits no `mbt::actionTaken`
metadata, so quint-connect 0.1.2 fails with `Missing mbt::actionTaken variable in
the trace`. All tests therefore use `#[quint_run]`, which passes `--mbt` to
`quint run`. Fixed regression scenarios are expressed as scenario-specific
`init`/`step` action pairs (e.g. `initCollision` + `hold`) selected via
`#[quint_run(init = ..., step = ...)]`. Revisit `#[quint_test]` when quint's
`quint test` supports `--mbt`.

### Mutation acceptance

These mutations make the named check fail, which is how the suite is kept wired
to the production path rather than to a copy of the expected behavior:

- `MCP_DAEMON_MAX_RETRIES` 2 → 1 makes `synthesis_retry_simulation` fail.
- `.min` → `.max` in `build_model_default_style_map` makes
  `target_resolution_model_default_style` fail.
- Reversing the style/model precedence makes the target-resolution MBT fail.
- An off-by-one in the retry bound (`after_attempt(attempts_started + 1, ...)`)
  makes `retry_loop_exhausts_retryable_failures` fail.
- Moving attempt counting after the await, or counting backoffs at completion,
  in `run_retry_loop` makes `retry_loop_cancel_in_flight_third_attempt` /
  `retry_loop_exhausts_retryable_failures` fail.
- Ignoring the wait's `Cancelled` outcome makes
  `retry_loop_cancel_during_backoff_prevents_next_attempt` fail.
- Dropping the backoff count in `RetryTracker::record_attempt` (or counting it in
  `end_backoff`) makes `synthesis_retry_simulation` fail.
- A wrong `tools/call` argument rule makes `mcp_request_parsing_bad_arguments`
  fail.
- Removing `#[serde(tag = "tag", content = "value")]` from the streaming
  driver's `Phase` makes `decode_tests::state_decodes_from_itf_encoding` fail.
- Pointing `VOICEVOX_SOCKET_PATH` at a live socket makes
  `streaming_connect_failure` fail (it expects the connect to fail).
- Serving a valid frame in the `ipc_corrupt_frame` scenario, or treating a
  mismatched/`Error` response as success, makes the corresponding `ipc_*` test
  fail.
- Removing the `ModelUnloadGuard` (or skipping the unload) makes
  `model_lifecycle_success` / `model_lifecycle_synth_failure` fail on `loaded`.
- Breaking daemon-side serialization (deadlock or a dropped concurrent request)
  makes `daemon_serialization_concurrent` fail.
- Changing the installer invocation budget (e.g. counting retries instead of
  total invocations) makes `download_succeeds_third_attempt` /
  `download_exhausts_attempts` fail.
- Dropping the final connect, or changing the connect attempt budget, makes
  `mcp_connect_budget_exhausted` fail.

### Known devShell quirk

In the devShell, the fenix stable toolchain aborts (`SIGABRT`, `fatal runtime
error: failed to initiate panic`) on **any** test panic — an isolated empty crate
reproduces it, so it is not caused by this crate. Failing MBT tests still exit
non-zero, so CI detection works; debugging a failure requires running the test
with `--nocapture`, because the harness cannot print captured output before the
abort.

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
  MBT scenarios.
- If a new state is introduced, update the owning `*.qnt`, the `System.qnt`
  mapping (if shared), and add a `verify.sh` check that exercises it.
- A new production path should either get a model + MBT or be added to the
  "Modelling boundary" section above.
