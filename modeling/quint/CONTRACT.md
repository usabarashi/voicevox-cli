# TLA+ → Quint Migration Contract (Phase 1)

This document is the Phase 1 gate artifact: the property / assumption /
dependency correspondence table plus the observation contract for the
model-based tests. It is written **before** the specs and the code are
reworked, so that every intentional change of meaning is recorded.

Phase 1 is a **feasibility gate**, not the full migration. It proves that the
Quint toolchain (verification + model-based testing) can carry the existing
guarantees and detect intended faults in production code. The old TLA+/TLC
artifacts are kept for now (see "Removal plan"). Nothing in `modeling/tla` or
`modeling/cfg` is deleted until its properties have an equivalent in Quint and
both are green.

## Toolchain and version pinning

| Tool | Version | Source | Notes |
|---|---|---|---|
| `quint` | 0.32.0 | `nixpkgs` (pinned by `flake.lock`) | added to the devShell in `flake.nix` |
| TLC backend | TLC 2.19 (through `quint verify --backend=tlc`) | bundled with the `quint` package | JRE 21 bundled as well; **no external Java or TLA+ install required** |
| `tlaplus` / TLC | existing | `nixpkgs` | kept for the existing `tla-model-check` job |

Rationale for the TLC backend: the existing suite relies on exhaustive
finite-state exploration and fairness-based liveness. Apalache (the default
backend) is bounded, and `quint run --invariant` is random simulation; neither
replaces TLC's checking. Quint's TLC backend emits TLA+ and runs TLC
internally, so the handwritten TLA+ sources can be removed later while the
TLC *engine* stays.

Run the gate:

```bash
nix develop --accept-flake-config --command bash modeling/quint/verify.sh
```

## Property / assumption / dependency correspondence

Phase 1 targets the Synthesis retry/cancel loop first (the walking skeleton for
`VoicevoxModel` follows).

| Concern | Old TLA+ source | Quint artifact | Status |
|---|---|---|---|
| Retry/cancel loop state | `Synthesis.tla` (`synthState`, `retryCount`, `errorKind`, `cancelSource`, `daemonReady`) | `quint/SynthesisRetry.qnt` (`outcome`, `attempts`, `backoffs`) + `domain/synthesis/{lifecycle,retry}.rs` | reworked (see contract changes) |
| Bounded retries (safety) | `Synthesis.tla` `TypeOK`, `TerminalStates` (`retryCount ≤ MAX_RETRY`) | `attemptsBounded`, `backoffsBounded` | preserved (bound changed) |
| Backoff only between attempts | (implicit) | `backoffAfterAttempt` | added |
| Termination (liveness) | `Synthesis.tla` `EventuallyLeavesSynthesizing` under `WF_vars` (`ProgressSpec`) | `eventuallyTerminal` under `weakFair` | strengthened (see contract changes) |
| Cancel attribution | `Synthesis.tla` `CanceledHasSource` / `NonCanceledHasNoSource` (`cancelSource` 4-value enum) | not modeled in Phase 1 | deferred (code has no enum; see contract changes) |
| Daemon readiness gating | `Synthesis.tla` `SynthesisNeedsDaemon` (`daemonReady`) | not modeled | dropped (code auto-starts; see contract changes) |
| Target resolution | `VoicevoxModel.tla` `SetTargetExists`/`SetTargetMissing`, accept/reject | `quint/TargetResolution.qnt` + `mbt/tests/target_resolution.rs` | done (Phase 1 walking skeleton) |
| Download lifecycle / retries | `VoicevoxModel.tla` `StartDownload`/`DownloadOk`/`DownloadFail`, `retryCount` | not modeled | **pending relocation** (belongs to `infrastructure/download`, not the catalog) |
| Integrated system | `System.tla` INSTANCE wiring of `StartupResources`/`MCPServer`/`Synthesis` | not modeled | Phase 2 |

The old `cfg` files selected constants, a specification variant
(`Spec`/`NormalSpec`/`ProgressSpec`/`InvalidTargetSpec`), invariants, and
temporal properties. In Quint these become the module's `init`/`step` plus
`--invariant` / `--temporal` selections (and, later, instance-based constants).
The fixed `run` definitions are additional regression scenarios. The full
`cfg` → Quint mapping is a Phase 2 deliverable; Phase 1 covers the properties
above with the default `init`/`step`.

## Assumptions and environment

| Kind | Assumption |
|---|---|
| Fairness | `weakFair(advance, vars)` where `advance = attempt | backoffDone` and `vars = (outcome, attempts, backoffs)`. Continuously enabled while `Running` (attempts remaining) or `Backoff`. |
| Environment input | An attempt's result (`ok` / `retryable` / `fatal`) is an input event, not real I/O. |
| Environment input | Cancellation is an input event accepted at explicit checkpoints. |
| Environment input | Backoff completion is an input event; no real clock is used in the model. |
| Stuttering | Terminal states have no enabled action; TLC adds stuttering. `skip`/stalling is modeled explicitly in the negative control. |
| State space bound | `MAX_ATTEMPTS = 3`, `MAX_RETRIES = 2`. |

## Recorded contract changes (vs the old TLA+ models)

These are **behavioural contract decisions**, not notation-only ports. They are
part of the approved Phase 1 scope.

1. **Retry bound: 3 → 2.** `Synthesis.progress.cfg` set `MAX_RETRY = 3`
   (initial + 3 retries). The production contract is `MCP_DAEMON_MAX_RETRIES = 2`
   (initial + 2 retries = at most 3 attempts). The model now encodes the
   production contract.
2. **Liveness strengthened.** The old `EventuallyLeavesSynthesizing`
   (`Synthesizing` is eventually left) is weaker than final completion; it can
   be satisfied by returning to `Idle` on daemon loss. Phase 1 requires
   `eventuallyTerminal` (reach `Done`/`Failed`/`Canceled`).
3. **`daemonReady` gating dropped.** Production auto-starts and connects to the
   daemon (`connect_daemon_client_auto_start`); there is no persistent readiness
   variable. Daemon availability is an environment concern.
4. **`cancelSource` enum not modeled.** Production carries a free-form reason
   string (`oneshot::channel::<String>()`), not a 4-value enum. Phase 1 models a
   single prioritized cancel event. If a source enum is introduced later, the
   `CanceledHasSource` / `NonCanceledHasNoSource` properties return.
5. **`VoicevoxModel` download properties are not carried** by the Phase 1
   target-resolution model. Their new home ( `infrastructure/download` ) must be
   recorded before any deletion.

"Preserving existing guarantees" therefore means preserving the *approved
contracts above*, not verbatim porting.

## Observation contract (Phase 1 MBT)

1. **Counting and simultaneity.**
   - At most 3 attempt *starts* (initial 1 + up to 2 retries); at most 2
     backoff *starts*; retries only for retryable errors.
   - Cancellation is evaluated at checkpoints (before an attempt, during
     backoff, during the in-flight wait) and **takes priority** when both a
     cancel and a completion are ready at the same poll instant.
   - Implementation: use a biased select with the cancel branch first; do not
     apply a blanket bias to every `select!`.
2. **Injected inputs / observed outputs.**
   - Inject: cancel timing (before attempt / during backoff / during the
     in-flight wait); the attempt result sequence (Ok / retryable error /
     non-retryable error); explicit release of the controllable wait (do **not**
     rely on `tokio::time::pause()` auto-advance).
   - Observe: attempt-start count, backoff-start count, terminal outcome, that
     **no further attempt starts after cancellation**, and the component's phase
     via its production accessors.
   - Never copy the expected model state into the "implementation state".
3. **Boundary.**
   - A single attempt is the whole client-side flow
     (`Validate → EnsureResources → Connect → Synthesize`).
   - The daemon is an environment returning `Ok(wav)` or a
     retryable/non-retryable error; daemon internals and connect auto-start are
     environment; daemon internals are deferred to Phase 3.
   - Cancellation means client-side wait termination only; it does not stop
     already-sent daemon work or roll back side effects.
   - The outer error classification (`interface/mcp_server/daemon_error.rs`) is
     exercised **separately with real errors**, because MBT injects
     pre-classified results.

## Negative controls

`verify.sh` asserts the checker detects violations for the intended reason:

- `negative/SafetyViolation.qnt`: unbounded attempts → `attemptsBounded`
  violated (safety detection).
- `negative/LivenessViolation.qnt`: an always-enabled `skip` with no fairness →
  `eventuallyTerminal` violated by an infinite stall, not by a deadlock
  (liveness detection, including the stuttering counterexample).

## Phase 1 walking skeleton: target resolution (MBT)

The first end-to-end MBT path connects the real target-resolution logic to
Quint:

- Spec: `modeling/quint/TargetResolution.qnt`
- Driver: `mbt/tests/target_resolution.rs`
- Run: `nix develop --accept-flake-config --command cargo test --locked --manifest-path mbt/Cargo.toml`

The driver calls the **production** function
`voicevox_cli::infrastructure::daemon::state::catalog::resolve_target` (the same
function `ModelCatalog::resolve_synthesis_target` delegates to) with the fixture
catalog modelled by the spec, and records the observed result. The observed
state is never copied from the spec, satisfying the observation contract.

`mbt` is a standalone Cargo workspace (own `Cargo.lock`, `[workspace]` in
`mbt/Cargo.toml`), so it is excluded from the root build and from
`nix flake check` / the crane sandbox, which have no `quint`.

### Toolchain limitation: `#[quint_test]` is unusable with quint 0.32.0

`#[quint_test]` generates traces with `quint test ... --out-itf`, and quint
0.32.0's `quint test` has **no `--mbt` flag** and emits no `mbt::actionTaken`
metadata, so quint-connect 0.1.2 fails with `Missing mbt::actionTaken variable
in the trace`. All tests therefore use `#[quint_run]`, which passes `--mbt` to
`quint run`. Fixed regression scenarios are expressed as scenario-specific
`init`/`step` action pairs (e.g. `initCollision` + `hold`) selected via
`#[quint_run(init = ..., step = ...)]`. This matches the planned "scenario
variants as separate init/step" approach. Revisit `#[quint_test]` when quint's
`quint test` supports `--mbt`.

### Mutation acceptance

The acceptance criterion for this skeleton is that breaking the production
decision order is detected through the production path:

- Mutation: resolve a known model ID before checking the style map (model
  priority instead of style priority).
- Result: `target_resolution_id_collision` fails with `Specification and
  implementation states diverge` (reproducible via the printed `QUINT_SEED`).

### Known devShell quirk

In this devShell, the fenix stable toolchain aborts (`SIGABRT`, `fatal runtime
error: failed to initiate panic`) on **any** test panic — an isolated empty
crate reproduces it. It is not caused by this crate. Failing MBT tests still
exit non-zero, so CI detection works; debugging a failure requires running the
test with `--nocapture`, because the harness cannot print captured output
before the abort.

## Phase 1: in-process retry orchestration

The client-side retry loop is now driven by the pure domain rules and tested
with fake synthesis results and a controllable wait seam:

- `domain/synthesis/lifecycle.rs`: single-attempt lifecycle
  (`Idle/Queued/Synthesizing/Done/Failed/Canceled`), used by
  `interface/synthesis/flow.rs`.
- `domain/synthesis/retry.rs`: retry policy (`max_retries`, `after_attempt`,
  `backoff_delay`). The daemon error classification stays in the interface.
- `interface/mcp_server/tools/text_to_speech.rs`: `run_retry_loop` orchestrates
  a generic `SynthesisAttempt` + `BackoffWaiter`. Production uses
  `RealSynthesisAttempt` (daemon client) and `RealBackoffWaiter`
  (`tokio::select!` with `biased;` and the cancel branch first, so cancellation
  wins when both are ready at the same poll instant).

The rule's return value actually drives the loop (attempt / backoff / stop);
there is no bookkeeping-only state machine.

### Observation contract, satisfied by the orchestration tests

- Attempts are counted when started and backoffs when started. The fake attempt
  also counts how many times the production loop invoked it, so "no attempt
  after cancellation" is observed directly.
- Cancellation is injected (pre-sent signal, or the wait seam returning
  `Cancelled`) instead of relying on wall-clock or auto-advancing time.
- The wait seam is released explicitly by the test, not by `tokio::time::pause`.

### Mutation acceptance (this step)

- Off-by-one in the retry bound (`after_attempt(attempts_started + 1, ...)`)
  makes `retry_loop_exhausts_retryable_failures` fail.
- Ignoring the wait's `Cancelled` outcome makes
  `retry_loop_cancel_during_backoff_prevents_next_attempt` fail (a second
  attempt is observed).
- Reversing the style/model precedence (previous step) makes the target
  resolution MBT fail.

These confirm the tests are wired to the production path, not to a copy of the
expected behavior.

### Exploration configuration & reproduction

- `#[quint_run]` uses a random seed unless `QUINT_SEED` is set. quint-connect
  reads `QUINT_SEED` at **compile time** (`option_env!`), so changing it requires
  recompiling the `mbt` crate.
- CI sets `QUINT_SEED=0x5eed` (see `.github/workflows/ci.yml`) for deterministic
  traces; leave it unset locally for broader exploration.
- On failure, quint-connect prints the seed (`Reproduce this error with
  QUINT_SEED=...`) and, with `QUINT_VERBOSE=1|2`, the trace. quint-connect 0.1.2
  does not persist traces to disk, so reproduction is seed-based:
  `QUINT_SEED=<printed> cargo test --locked --manifest-path mbt/Cargo.toml -- --nocapture`.
- `max_samples` / `max_steps` are set explicitly in the test attributes
  (200 / 6 for the simulation, 1 / 1 for the fixed scenarios), because supplying
  a seed also changes quint's default sample count.

## Phase 1 status

Phase 1 was defined as a feasibility gate with these success criteria: preserve
the existing guarantees, detect intended production faults, and keep the
maintenance cost acceptable.

| Criterion | Evidence |
|---|---|
| Quint can carry the verification role | `verify.sh`: `SynthesisRetry` safety + liveness pass; negative controls detected (safety violation, infinite stall). `quint verify --backend=tlc` needs no external Java/TLA+. |
| Specs connect to production code | `mbt/tests/target_resolution.rs` calls the real `resolve_target` (the same function `ModelCatalog::resolve_synthesis_target` delegates to); 4 tests pass. |
| Intended faults are detected through the production path | style/model precedence flip, retry-bound off-by-one, and ignored wait-cancellation all fail their tests. |
| Existing abstract TLA+ kept during migration | the `tla-model-check` job and `modeling/tla` / `modeling/cfg` are untouched. |
| Maintenance cost acceptable | verification gate and MBT both run in seconds and need no model downloads. |

Not yet done (Phase 2+): port the remaining TLA+ modules, preserve their
properties with `quint verify`, retire the handwritten TLA+/cfg/direct job, and
decide Phase 3 (process/socket MBT).

## Removal plan (not in this change)

Handwritten `modeling/tla` and `modeling/cfg` plus the direct `tlc` CI job are
removed only after each module has an equivalent Quint artifact that verifies
green. The TLC verification *engine* (through Quint's TLC backend) is retained.
