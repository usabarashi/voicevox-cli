// Ported from `modeling/tla/System.tla` (Phase 2), revised.
//
// Top-level integrated model: startup resources + daemon + client + refined
// Synthesis, with the cross-view synchronization.
//
// Adaptations (see PORTING.md):
// - `Synthesis` is the refined `SynthesisRetry` contract: `synthOutcome`
//   (Idle/Running/Backoff/Done/Failed/Canceled), `synthAttempts`, `synthBackoffs`.
// - **Synthesis does not require a ready daemon.** Production auto-starts the
//   daemon (`connect_daemon_client_auto_start`) and treats a not-ready daemon as
//   a retryable attempt failure, not as a cancellation. The old
//   `SynthesisRunningImpliesDaemonReady` / `SynthesisNeedsDaemon` gating is
//   therefore **retracted** (consistent with the Phase 1 contract change 3); a
//   daemon-loss transition no longer cancels a `Running`/`Backoff` synthesis.
// - A non-retryable (`fatal`) attempt outcome fails immediately from any attempt
//   (`synthFatalFail`).
// - `synthBackoffs` is counted when a backoff **starts** (`synthFailRetry`),
//   matching production `backoffs_started` and `SynthesisRetry.qnt`.
// - The four startup resources are encoded as a map indexed 0..3
//   (0=runtime, 1=dictionary, 2=socket, 3=model). A resource that exhausts its
//   retry budget is terminal (re-entry is only through the budget-guarded
//   `retryLoad`).
// - Connect uses the production budget `MAX_CONNECT_ATTEMPTS` (10) plus one
//   final connect, and has no persistent "degraded" service mode. Losing a
//   connection ends the episode, so the budget resets for the next one.
// - Playback transitions are omitted: they are verified in `MCPServer.qnt`.

module System {
  pure val MAX_RESOURCE_RETRIES = 3
  pure val MAX_SYNTH_RETRIES = 2
  pure val MAX_SYNTH_ATTEMPTS = MAX_SYNTH_RETRIES + 1
  pure val MAX_CONNECT_ATTEMPTS = 10

  type DaemonPhase = Down | DaemonStarting | DaemonReady | DaemonFailed
  type LoadState = Missing | Loading | Ready | Failed
  type ViewDaemon = ViewDaemonDown | ViewDaemonReady
  type ClientState = ClientIdle | Connecting | Connected | ClientFailed
  type SynthOutcome = SynthIdle | Running | Backoff | Done | SynthFailed | Canceled

  var daemonState: DaemonPhase
  var resources: int -> LoadState
  var retries: int -> int
  var clientDaemonState: ViewDaemon
  var clientState: ClientState
  var clientAttempt: int
  var synthOutcome: SynthOutcome
  var synthAttempts: int
  var synthBackoffs: int

  val resourceReady =
    resources.get(0) == Ready and resources.get(1) == Ready and resources.get(3) == Ready

  action init = all {
    daemonState' = Down,
    resources' = Map(0 -> Missing, 1 -> Missing, 2 -> Missing, 3 -> Missing),
    retries' = Map(0 -> 0, 1 -> 0, 2 -> 0, 3 -> 0),
    clientDaemonState' = ViewDaemonDown,
    clientState' = ClientIdle,
    clientAttempt' = 0,
    synthOutcome' = SynthIdle,
    synthAttempts' = 0,
    synthBackoffs' = 0,
  }

  // ---- Framing helpers ---------------------------------------------------

  action frameResources = all {
    resources' = resources,
    retries' = retries,
  }

  action frameDaemon = all {
    daemonState' = daemonState,
  }

  action frameClient = all {
    clientDaemonState' = clientDaemonState,
    clientState' = clientState,
    clientAttempt' = clientAttempt,
  }

  action frameSynth = all {
    synthOutcome' = synthOutcome,
    synthAttempts' = synthAttempts,
    synthBackoffs' = synthBackoffs,
  }

  /// Updates the client's view of daemon readiness. A connection lost while
  /// connected resets the client to idle and **resets the connect budget**: the
  /// next request calls `connect_with_retry` again from the start. Synthesis is
  /// not affected (the daemon is an environment that the attempt auto-starts).
  action viewsForDaemon(newDaemon) = all {
    clientDaemonState' = if (newDaemon == DaemonReady) ViewDaemonReady else ViewDaemonDown,
    clientState' = if (newDaemon != DaemonReady and clientState == Connected) ClientIdle else clientState,
    clientAttempt' = if (newDaemon != DaemonReady and clientState == Connected) 0 else clientAttempt,
    synthOutcome' = synthOutcome,
    synthAttempts' = synthAttempts,
    synthBackoffs' = synthBackoffs,
  }

  // ---- Startup resources (indices 0..3) ----------------------------------

  action beginLoad = all {
    nondet i = Set(0, 1, 2, 3).oneOf()
    all {
      // First load only. Re-entry after a failure goes through `retryLoad`,
      // which enforces the retry budget.
      resources.get(i) == Missing,
      resources' = resources.set(i, Loading),
      retries' = retries,
      frameDaemon, frameClient, frameSynth,
    },
  }

  action loadOk = all {
    nondet i = Set(0, 1, 2, 3).oneOf()
    all {
      resources.get(i) == Loading,
      resources' = resources.set(i, Ready),
      retries' = retries,
      frameDaemon, frameClient, frameSynth,
    },
  }

  action loadFail = all {
    nondet i = Set(0, 1, 2, 3).oneOf()
    all {
      resources.get(i) == Loading,
      resources' = resources.set(i, Failed),
      retries' = retries.set(i, if (retries.get(i) < MAX_RESOURCE_RETRIES) retries.get(i) + 1 else retries.get(i)),
      frameDaemon, frameClient, frameSynth,
    },
  }

  action retryLoad = all {
    nondet i = Set(0, 1, 2, 3).oneOf()
    all {
      resources.get(i) == Failed,
      retries.get(i) < MAX_RESOURCE_RETRIES,
      resources' = resources.set(i, Loading),
      retries' = retries,
      frameDaemon, frameClient, frameSynth,
    },
  }

  // ---- Daemon ------------------------------------------------------------

  action startDaemon = all {
    daemonState == Down,
    resourceReady,
    daemonState' = DaemonStarting,
    resources' = resources,
    retries' = retries,
    viewsForDaemon(DaemonStarting),
  }

  action daemonBootOk = all {
    daemonState == DaemonStarting,
    resources.get(2) == Ready,
    daemonState' = DaemonReady,
    resources' = resources,
    retries' = retries,
    viewsForDaemon(DaemonReady),
  }

  action daemonBootFail = all {
    daemonState == DaemonStarting,
    daemonState' = DaemonFailed,
    resources' = resources,
    retries' = retries,
    viewsForDaemon(DaemonFailed),
  }

  // The daemon is lost after having been ready. This resets the client view and
  // the connect budget, but does not cancel an in-flight synthesis.
  action daemonLost = all {
    daemonState == DaemonReady,
    daemonState' = DaemonFailed,
    resources' = resources,
    retries' = retries,
    viewsForDaemon(DaemonFailed),
  }

  action resetDaemon = all {
    daemonState == DaemonFailed,
    daemonState' = Down,
    resources' = resources,
    retries' = retries,
    viewsForDaemon(Down),
  }

  // ---- Client connect ----------------------------------------------------
  //
  // Mirrors `transport::connect_with_retry`: up to MAX_CONNECT_ATTEMPTS loop
  // connects, then one final connect without backoff. A failed episode ends in
  // `ClientFailed`; production stores no persistent degraded mode.

  action startConnect = all {
    clientState == ClientIdle,
    clientState' = Connecting,
    clientDaemonState' = clientDaemonState,
    clientAttempt' = clientAttempt,
    frameDaemon, frameResources, frameSynth,
  }

  action connectOk = all {
    clientState == Connecting,
    clientDaemonState == ViewDaemonReady,
    clientAttempt < MAX_CONNECT_ATTEMPTS,
    clientState' = Connected,
    clientDaemonState' = clientDaemonState,
    clientAttempt' = clientAttempt,
    frameDaemon, frameResources, frameSynth,
  }

  action connectRetry = all {
    clientState == Connecting,
    clientDaemonState == ViewDaemonDown,
    clientAttempt < MAX_CONNECT_ATTEMPTS,
    clientState' = ClientIdle,
    clientAttempt' = clientAttempt + 1,
    clientDaemonState' = clientDaemonState,
    frameDaemon, frameResources, frameSynth,
  }

  action finalConnectOk = all {
    clientState == Connecting,
    clientDaemonState == ViewDaemonReady,
    clientAttempt == MAX_CONNECT_ATTEMPTS,
    clientState' = Connected,
    clientDaemonState' = clientDaemonState,
    clientAttempt' = clientAttempt,
    frameDaemon, frameResources, frameSynth,
  }

  action finalConnectFail = all {
    clientState == Connecting,
    clientDaemonState == ViewDaemonDown,
    clientAttempt == MAX_CONNECT_ATTEMPTS,
    clientState' = ClientFailed,
    clientDaemonState' = clientDaemonState,
    clientAttempt' = clientAttempt,
    frameDaemon, frameResources, frameSynth,
  }

  action resetClientFailed = all {
    clientState == ClientFailed,
    clientState' = ClientIdle,
    clientAttempt' = 0,
    clientDaemonState' = clientDaemonState,
    frameDaemon, frameResources, frameSynth,
  }

  // ---- Synthesis (refined contract) --------------------------------------
  //
  // The daemon is an environment: attempts may run while it is not ready and
  // auto-start it.

  action enqueue = all {
    synthOutcome == SynthIdle,
    synthOutcome' = Running,
    synthAttempts' = 1,
    synthBackoffs' = 0,
    frameDaemon, frameResources, frameClient,
  }

  action synthOk = all {
    synthOutcome == Running,
    synthOutcome' = Done,
    synthAttempts' = synthAttempts,
    synthBackoffs' = synthBackoffs,
    frameDaemon, frameResources, frameClient,
  }

  // A retryable failure starts a backoff (counted at start).
  action synthFailRetry = all {
    synthOutcome == Running,
    synthAttempts < MAX_SYNTH_ATTEMPTS,
    synthOutcome' = Backoff,
    synthAttempts' = synthAttempts,
    synthBackoffs' = synthBackoffs + 1,
    frameDaemon, frameResources, frameClient,
  }

  action synthFailFinal = all {
    synthOutcome == Running,
    synthAttempts == MAX_SYNTH_ATTEMPTS,
    synthOutcome' = SynthFailed,
    synthAttempts' = synthAttempts,
    synthBackoffs' = synthBackoffs,
    frameDaemon, frameResources, frameClient,
  }

  // A non-retryable failure stops immediately, from any attempt.
  action synthFatalFail = all {
    synthOutcome == Running,
    synthOutcome' = SynthFailed,
    synthAttempts' = synthAttempts,
    synthBackoffs' = synthBackoffs,
    frameDaemon, frameResources, frameClient,
  }

  action backoffDone = all {
    synthOutcome == Backoff,
    synthOutcome' = Running,
    synthAttempts' = synthAttempts + 1,
    synthBackoffs' = synthBackoffs,
    frameDaemon, frameResources, frameClient,
  }

  action cancelSynth = all {
    synthOutcome == Running or synthOutcome == Backoff,
    synthOutcome' = Canceled,
    synthAttempts' = synthAttempts,
    synthBackoffs' = synthBackoffs,
    frameDaemon, frameResources, frameClient,
  }

  action resetSynth = all {
    synthOutcome == Done or synthOutcome == SynthFailed or synthOutcome == Canceled,
    synthOutcome' = SynthIdle,
    synthAttempts' = 0,
    synthBackoffs' = 0,
    frameDaemon, frameResources, frameClient,
  }

  // ---- Step --------------------------------------------------------------

  action resourceStep = any {
    beginLoad,
    loadOk,
    loadFail,
    retryLoad,
  }

  action daemonStep = any {
    startDaemon,
    daemonBootOk,
    daemonBootFail,
    daemonLost,
    resetDaemon,
  }

  action clientStep = any {
    startConnect,
    connectOk,
    connectRetry,
    finalConnectOk,
    finalConnectFail,
    resetClientFailed,
  }

  action synthStep = any {
    enqueue,
    synthOk,
    synthFailRetry,
    synthFailFinal,
    synthFatalFail,
    backoffDone,
    cancelSynth,
    resetSynth,
  }

  action stutter = all {
    frameDaemon, frameResources, frameClient, frameSynth,
  }

  action step = any {
    resourceStep,
    daemonStep,
    clientStep,
    synthStep,
    stutter,
  }

  // ---- Properties --------------------------------------------------------

  val typeOK =
    retries.keys().forall(i => retries.get(i) >= 0 and retries.get(i) <= MAX_RESOURCE_RETRIES) and
    clientAttempt >= 0 and clientAttempt <= MAX_CONNECT_ATTEMPTS and
    synthAttempts >= 0 and synthAttempts <= MAX_SYNTH_ATTEMPTS and
    synthBackoffs >= 0 and synthBackoffs <= MAX_SYNTH_RETRIES

  val viewsAligned =
    clientDaemonState == (if (daemonState == DaemonReady) ViewDaemonReady else ViewDaemonDown)

  // The daemon binds its socket only after its state (catalog) is initialized,
  // so a connected client implies a ready daemon.
  val clientConnectedImpliesDaemonReady =
    clientState == Connected implies daemonState == DaemonReady

  // ---- Cross-layer startup guarantees ------------------------------------
  //
  // These couple the daemon/startup layer to the resource layer (no single
  // per-concern spec checks them at the integration point).

  // The daemon is only started once the resources it needs are ready.
  val daemonStartingRequiresResources =
    daemonState == DaemonStarting implies resourceReady

  // A ready daemon implies the resources are ready and the socket is bound
  // (resources are never unloaded once ready).
  val daemonReadyRequiresResources =
    daemonState == DaemonReady implies resourceReady

  val daemonReadyRequiresSocket =
    daemonState == DaemonReady implies resources.get(2) == Ready
}
