// Ported from `modeling/tla/System.tla` (Phase 2).
//
// Top-level integrated model: startup resources + daemon + client + refined
// Synthesis, with the cross-view synchronization.
//
// Adaptations (see PORTING.md):
// - `Synthesis` is the refined `SynthesisRetry` contract: `synthOutcome`
//   (Idle/Running/Backoff/Done/Failed/Canceled), `synthAttempts`, `synthBackoffs`,
//   and no `daemonReady`/`cancelSource`/`errorKind`.
// - `SynthesisRunningImpliesDaemonReady` becomes: an attempt only starts while
//   the daemon is ready, and going not-ready cancels a Running/Backoff synthesis.
// - The four startup resources are encoded as a map indexed 0..3
//   (0=runtime, 1=dictionary, 2=socket, 3=model) with the same lifecycle; this
//   preserves `resourceReady` and the readiness-implies-socket invariant while
//   keeping the module compact.
// - Playback transitions are omitted: they are verified in `MCPServer.qnt` and
//   do not affect the invariants checked by `System.integration.cfg`.

module System {
  pure val MAX_RETRY = 2
  pure val MAX_ATTEMPTS = 3

  type DaemonPhase = Down | DaemonStarting | DaemonReady | DaemonFailed
  type LoadState = Missing | Loading | Ready | Failed
  type ViewDaemon = ViewDaemonDown | ViewDaemonReady
  type ClientState = ClientIdle | Connecting | Connected
  type ServiceMode = Normal | Degraded
  type SynthOutcome = SynthIdle | Running | Backoff | Done | SynthFailed | Canceled

  var daemonState: DaemonPhase
  var resources: int -> LoadState
  var retries: int -> int
  var clientDaemonState: ViewDaemon
  var clientState: ClientState
  var clientAttempt: int
  var clientServiceMode: ServiceMode
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
    clientServiceMode' = Normal,
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
    clientServiceMode' = clientServiceMode,
  }

  action frameSynth = all {
    synthOutcome' = synthOutcome,
    synthAttempts' = synthAttempts,
    synthBackoffs' = synthBackoffs,
  }

  /// Derives the client and synthesis views from a new daemon state. Follows
  /// `SyncViewsAndDependentStates`: a non-ready daemon resets a connected
  /// client to idle/degraded and cancels an in-flight synthesis.
  action viewsForDaemon(newDaemon) = all {
    clientDaemonState' = if (newDaemon == DaemonReady) ViewDaemonReady else ViewDaemonDown,
    clientState' = if (newDaemon != DaemonReady and clientState == Connected) ClientIdle else clientState,
    clientServiceMode' = if (newDaemon != DaemonReady and clientState == Connected) Degraded else clientServiceMode,
    synthOutcome' = if (newDaemon != DaemonReady and (synthOutcome == Running or synthOutcome == Backoff)) Canceled else synthOutcome,
    clientAttempt' = clientAttempt,
    synthAttempts' = synthAttempts,
    synthBackoffs' = synthBackoffs,
  }

  // ---- Startup resources (indices 0..3) ----------------------------------

  action beginLoad = all {
    nondet i = Set(0, 1, 2, 3).oneOf()
    all {
      resources.get(i) == Missing or resources.get(i) == Failed,
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
      retries' = retries.set(i, if (retries.get(i) < MAX_RETRY) retries.get(i) + 1 else retries.get(i)),
      frameDaemon, frameClient, frameSynth,
    },
  }

  action retryLoad = all {
    nondet i = Set(0, 1, 2, 3).oneOf()
    all {
      resources.get(i) == Failed,
      retries.get(i) < MAX_RETRY,
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

  action resetDaemon = all {
    daemonState == DaemonFailed,
    daemonState' = Down,
    resources' = resources,
    retries' = retries,
    viewsForDaemon(Down),
  }

  // ---- Client connect / degraded -----------------------------------------

  action startConnect = all {
    clientState == ClientIdle,
    clientState' = Connecting,
    clientDaemonState' = clientDaemonState,
    clientAttempt' = clientAttempt,
    clientServiceMode' = clientServiceMode,
    frameDaemon, frameResources, frameSynth,
  }

  action connectOk = all {
    clientState == Connecting,
    clientDaemonState == ViewDaemonReady,
    clientState' = Connected,
    clientServiceMode' = Normal,
    clientDaemonState' = clientDaemonState,
    clientAttempt' = clientAttempt,
    frameDaemon, frameResources, frameSynth,
  }

  action connectRetry = all {
    clientState == Connecting,
    clientDaemonState == ViewDaemonDown,
    clientAttempt < MAX_ATTEMPTS,
    clientState' = ClientIdle,
    clientAttempt' = clientAttempt + 1,
    clientDaemonState' = clientDaemonState,
    clientServiceMode' = clientServiceMode,
    frameDaemon, frameResources, frameSynth,
  }

  action enterDegraded = all {
    clientState == Connecting,
    clientDaemonState == ViewDaemonDown,
    clientAttempt == MAX_ATTEMPTS,
    clientState' = ClientIdle,
    clientServiceMode' = Degraded,
    clientDaemonState' = clientDaemonState,
    clientAttempt' = clientAttempt,
    frameDaemon, frameResources, frameSynth,
  }

  action leaveDegraded = all {
    clientServiceMode == Degraded,
    clientState == ClientIdle,
    clientServiceMode' = Normal,
    clientAttempt' = 0,
    clientDaemonState' = clientDaemonState,
    clientState' = clientState,
    frameDaemon, frameResources, frameSynth,
  }

  // ---- Synthesis (refined contract) --------------------------------------

  action enqueue = all {
    synthOutcome == SynthIdle,
    clientDaemonState == ViewDaemonReady,
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

  action synthFailRetry = all {
    synthOutcome == Running,
    synthAttempts < MAX_ATTEMPTS,
    synthOutcome' = Backoff,
    synthAttempts' = synthAttempts,
    synthBackoffs' = synthBackoffs,
    frameDaemon, frameResources, frameClient,
  }

  action synthFailFinal = all {
    synthOutcome == Running,
    synthAttempts == MAX_ATTEMPTS,
    synthOutcome' = SynthFailed,
    synthAttempts' = synthAttempts,
    synthBackoffs' = synthBackoffs,
    frameDaemon, frameResources, frameClient,
  }

  action backoffDone = all {
    synthOutcome == Backoff,
    clientDaemonState == ViewDaemonReady,
    synthOutcome' = Running,
    synthAttempts' = synthAttempts + 1,
    synthBackoffs' = synthBackoffs + 1,
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
    resetDaemon,
  }

  action clientStep = any {
    startConnect,
    connectOk,
    connectRetry,
    enterDegraded,
    leaveDegraded,
  }

  action synthStep = any {
    enqueue,
    synthOk,
    synthFailRetry,
    synthFailFinal,
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
    retries.keys().forall(i => retries.get(i) >= 0 and retries.get(i) <= MAX_RETRY) and
    clientAttempt >= 0 and clientAttempt <= MAX_ATTEMPTS and
    synthAttempts >= 0 and synthAttempts <= MAX_ATTEMPTS

  val viewsAligned =
    clientDaemonState == (if (daemonState == DaemonReady) ViewDaemonReady else ViewDaemonDown)

  val clientConnectedImpliesDaemonReady =
    clientState == Connected implies daemonState == DaemonReady

  val synthRunningImpliesDaemonReady =
    synthOutcome == Running implies daemonState == DaemonReady
}
