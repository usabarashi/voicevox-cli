// Daemon startup / duplicate-prevention model.
//
// Mirrors the client-side daemon bootstrap decision procedure
// (`src/infrastructure/daemon/process.rs` `handle_existing_socket` /
// `remove_stale_socket`, and `bootstrap.rs` `remove_stale_socket_if_requested`):
//
//   - if the socket is responsive, a daemon is already running: do not start a
//     second one and do not remove the socket,
//   - if the socket exists but is not responsive, re-check responsiveness
//     (TOCTOU guard) and look for a daemon process before treating it as stale,
//   - a stale socket is removed and only then is a new daemon started.
//
// The socket scenario is chosen non-deterministically to explore all three
// observations: absent, stale-present, live-present.

module DaemonStartup {
  type SocketScenario = Absent | StalePresent | LivePresent
  type Phase = Deciding | Started | AlreadyRunning

  var scenario: SocketScenario
  var removed: bool
  var started: bool
  var phase: Phase

  action init = all {
    nondet s = oneOf(Set(Absent, StalePresent, LivePresent))
    all {
      scenario' = s,
      removed' = false,
      started' = false,
      phase' = Deciding,
    }
  }

  // A live daemon is detected: neither remove its socket nor start another.
  action detectLive = all {
    phase == Deciding,
    scenario == LivePresent,
    phase' = AlreadyRunning,
    removed' = removed,
    started' = started,
    scenario' = scenario,
  }

  // A stale socket (not responsive, no daemon process) is removed.
  action removeStale = all {
    phase == Deciding,
    scenario == StalePresent,
    removed' = true,
    started' = started,
    phase' = phase,
    scenario' = scenario,
  }

  // A new daemon is started only when no live daemon exists, and a stale socket
  // is removed first.
  action startDaemon = all {
    phase == Deciding,
    scenario == Absent or (scenario == StalePresent and removed),
    started' = true,
    removed' = removed,
    phase' = Started,
    scenario' = scenario,
  }

  action stutter = all {
    scenario' = scenario,
    removed' = removed,
    started' = started,
    phase' = phase,
  }

  action step = any {
    detectLive,
    removeStale,
    startDaemon,
    stutter,
  }

  // ---- Safety -------------------------------------------------------------

  // A live daemon's socket is never removed.
  val liveNeverRemoved = scenario == LivePresent implies not(removed)

  // A second daemon is never started while a live one exists.
  val liveNeverStarted = scenario == LivePresent implies not(started)

  // Only a stale socket is ever removed.
  val removedOnlyStale = removed implies scenario == StalePresent

  // A stale socket is removed before a daemon is started.
  val staleRemovedBeforeStart =
    started and scenario == StalePresent implies removed

  // ---- Liveness -----------------------------------------------------------

  action advance = any {
    detectLive,
    removeStale,
    startDaemon,
  }

  val vars = (scenario, removed, started, phase)

  // The decision procedure terminates: it either detects a running daemon or
  // starts one.
  temporal decides =
    weakFair(advance, vars) implies eventually(phase != Deciding)
}
