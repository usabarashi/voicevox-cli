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
| Retry/cancel loop state | `Synthesis.tla` (`synthState`, `retryCount`, `errorKind`, `cancelSource`, `daemonReady`) | `quint/SynthesisRetry.qnt` (`outcome`, `attempts`, `backoffs`) | reworked (see contract changes) |
| Bounded retries (safety) | `Synthesis.tla` `TypeOK`, `TerminalStates` (`retryCount ≤ MAX_RETRY`) | `attemptsBounded`, `backoffsBounded` | preserved (bound changed) |
| Backoff only between attempts | (implicit) | `backoffAfterAttempt` | added |
| Termination (liveness) | `Synthesis.tla` `EventuallyLeavesSynthesizing` under `WF_vars` (`ProgressSpec`) | `eventuallyTerminal` under `weakFair` | strengthened (see contract changes) |
| Cancel attribution | `Synthesis.tla` `CanceledHasSource` / `NonCanceledHasNoSource` (`cancelSource` 4-value enum) | not modeled in Phase 1 | deferred (code has no enum; see contract changes) |
| Daemon readiness gating | `Synthesis.tla` `SynthesisNeedsDaemon` (`daemonReady`) | not modeled | dropped (code auto-starts; see contract changes) |
| Target resolution | `VoicevoxModel.tla` `SetTargetExists`/`SetTargetMissing`, accept/reject | pending (`VoicevoxModel` walking skeleton) | pending Phase 1 |
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

## Removal plan (not in this change)

Handwritten `modeling/tla` and `modeling/cfg` plus the direct `tlc` CI job are
removed only after each module has an equivalent Quint artifact that verifies
green. The TLC verification *engine* (through Quint's TLC backend) is retained.
