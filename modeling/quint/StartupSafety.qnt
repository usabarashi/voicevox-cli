// Integrated startup safety model.
//
// Composes the pieces that are modelled separately elsewhere:
//   - resource readiness (`StartupResources.qnt` / `Download.qnt`),
//   - the socket scenario and duplicate prevention (`DaemonStartup.qnt`),
//   - the ordering "prepare resources -> (remove stale socket) -> start -> ready".
//
// It proves the cross-layer guarantees that no single per-concern spec can:
// the daemon never becomes ready without resources, a live daemon is never
// doubled, and a stale socket is removed before starting.
//
// The full `System.qnt` additionally models the client view and synthesis; this
// model isolates the startup path so its ordering properties are explicit.

module StartupSafety {
  type SocketScenario = Absent | StalePresent | LivePresent
  type Phase = Init | Preparing | Ready | AlreadyRunning | Failed

  var socket: SocketScenario
  var resourcesReady: bool
  var resourcesFailed: bool
  var staleRemoved: bool
  var startedByUs: bool
  var phase: Phase

  action init = all {
    nondet s = oneOf(Set(Absent, StalePresent, LivePresent))
    all {
      socket' = s,
      resourcesReady' = false,
      resourcesFailed' = false,
      staleRemoved' = false,
      startedByUs' = false,
      phase' = Init,
    }
  }

  // Startup resources are prepared before any daemon start.
  action prepareOk = all {
    phase == Init,
    resourcesReady' = true,
    resourcesFailed' = false,
    socket' = socket,
    staleRemoved' = staleRemoved,
    startedByUs' = startedByUs,
    phase' = Preparing,
  }

  action prepareFail = all {
    phase == Init,
    resourcesReady' = false,
    resourcesFailed' = true,
    socket' = socket,
    staleRemoved' = staleRemoved,
    startedByUs' = startedByUs,
    phase' = Failed,
  }

  // A live daemon is detected: neither remove its socket nor start another.
  action detectLive = all {
    phase == Init or phase == Preparing,
    socket == LivePresent,
    phase' = AlreadyRunning,
    socket' = socket,
    resourcesReady' = resourcesReady,
    resourcesFailed' = resourcesFailed,
    staleRemoved' = staleRemoved,
    startedByUs' = startedByUs,
  }

  // A stale socket must be removed before starting.
  action removeStale = all {
    phase == Init or phase == Preparing,
    socket == StalePresent,
    staleRemoved' = true,
    socket' = socket,
    resourcesReady' = resourcesReady,
    resourcesFailed' = resourcesFailed,
    startedByUs' = startedByUs,
    phase' = phase,
  }

  // Start only with resources ready, no live daemon, and any stale socket gone.
  action start = all {
    phase == Preparing,
    resourcesReady,
    socket != LivePresent,
    socket == Absent or staleRemoved,
    startedByUs' = true,
    phase' = Ready,
    socket' = socket,
    resourcesReady' = resourcesReady,
    resourcesFailed' = resourcesFailed,
    staleRemoved' = staleRemoved,
  }

  action stutter = all {
    socket' = socket,
    resourcesReady' = resourcesReady,
    resourcesFailed' = resourcesFailed,
    staleRemoved' = staleRemoved,
    startedByUs' = startedByUs,
    phase' = phase,
  }

  action step = any {
    prepareOk,
    prepareFail,
    detectLive,
    removeStale,
    start,
    stutter,
  }

  // ---- Safety (cross-layer) ----------------------------------------------

  val readyRequiresResources = phase == Ready implies resourcesReady

  // Narrowed name: this checks the socket *scenario* is startable (absent, or
  // stale-and-removed) and not live. Bind success and the TOCTOU re-probe are
  // not modelled here (see STATE_TRACEABILITY.md).
  val readyRequiresStartableSocket =
    phase == Ready implies socket != LivePresent and (socket == Absent or staleRemoved)

  val startedImpliesNoLive = startedByUs implies socket != LivePresent

  // A live daemon's socket is never removed and a second daemon is never
  // started (subsumes DaemonStartup.qnt, which was retired into this model).
  val liveNeverRemoved = socket == LivePresent implies not(staleRemoved)
  val liveNeverStarted = socket == LivePresent implies not(startedByUs)

  val staleRemovedBeforeStart =
    startedByUs and socket == StalePresent implies staleRemoved

  val removedOnlyStale = staleRemoved implies socket == StalePresent

  val alreadyRunningOnlyLive = phase == AlreadyRunning implies socket == LivePresent

  val failedImpliesResourceFailure = phase == Failed implies resourcesFailed

  // ---- Liveness -----------------------------------------------------------

  action advance = any {
    prepareOk,
    prepareFail,
    detectLive,
    removeStale,
    start,
  }

  val vars = (socket, resourcesReady, resourcesFailed, staleRemoved, startedByUs, phase)

  // Startup reaches a terminal outcome.
  temporal terminates =
    weakFair(advance, vars) implies
      eventually(phase == Ready or phase == AlreadyRunning or phase == Failed)
}
