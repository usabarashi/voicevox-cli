// Ported from `modeling/tla/SynthesisParallel.tla` (Phase 2).
//
// Two-job parallel synthesis model with queue/worker/cancel interactions.
// Preserved properties: `AtMostOneSynthesizing`, `WorkerMatchesSynthesis`,
// and the temporal `EventuallyLeavesBusyWorker` under the old `ProgressSpec`
// fairness assumption.
//
// `Idle` is shared by the job and worker state sets in TLA+; it is renamed to
// `JobIdle` / `WorkerIdle` to avoid a conflicting sum-type label.

module SynthesisParallel {
  pure val MAX_RETRY = 2

  type JobState = JobIdle | Queued | Synthesizing | Done | Failed | Canceled
  type WorkerState = WorkerIdle | BusyJ1 | BusyJ2

  var daemonReady: bool
  var workerState: WorkerState
  var j1State: JobState
  var j2State: JobState
  var j1Retry: int
  var j2Retry: int

  action init = all {
    daemonReady' = false,
    workerState' = WorkerIdle,
    j1State' = JobIdle,
    j2State' = JobIdle,
    j1Retry' = 0,
    j2Retry' = 0,
  }

  action daemonUp = all {
    not(daemonReady),
    daemonReady' = true,
    workerState' = workerState,
    j1State' = j1State,
    j2State' = j2State,
    j1Retry' = j1Retry,
    j2Retry' = j2Retry,
  }

  action daemonDown = all {
    daemonReady,
    daemonReady' = false,
    workerState' = WorkerIdle,
    j1State' = if (j1State == Queued or j1State == Synthesizing) JobIdle else j1State,
    j2State' = if (j2State == Queued or j2State == Synthesizing) JobIdle else j2State,
    j1Retry' = j1Retry,
    j2Retry' = j2Retry,
  }

  action enqueueJ1 = all {
    daemonReady,
    j1State == JobIdle,
    j1State' = Queued,
    daemonReady' = daemonReady,
    workerState' = workerState,
    j2State' = j2State,
    j1Retry' = j1Retry,
    j2Retry' = j2Retry,
  }

  action enqueueJ2 = all {
    daemonReady,
    j2State == JobIdle,
    j2State' = Queued,
    daemonReady' = daemonReady,
    workerState' = workerState,
    j1State' = j1State,
    j1Retry' = j1Retry,
    j2Retry' = j2Retry,
  }

  action startJ1 = all {
    daemonReady,
    workerState == WorkerIdle,
    j1State == Queued,
    j1State' = Synthesizing,
    workerState' = BusyJ1,
    daemonReady' = daemonReady,
    j2State' = j2State,
    j1Retry' = j1Retry,
    j2Retry' = j2Retry,
  }

  action startJ2 = all {
    daemonReady,
    workerState == WorkerIdle,
    j2State == Queued,
    j2State' = Synthesizing,
    workerState' = BusyJ2,
    daemonReady' = daemonReady,
    j1State' = j1State,
    j1Retry' = j1Retry,
    j2Retry' = j2Retry,
  }

  action finishJ1 = all {
    workerState == BusyJ1,
    j1State == Synthesizing,
    j1State' = Done,
    workerState' = WorkerIdle,
    daemonReady' = daemonReady,
    j2State' = j2State,
    j1Retry' = j1Retry,
    j2Retry' = j2Retry,
  }

  action finishJ2 = all {
    workerState == BusyJ2,
    j2State == Synthesizing,
    j2State' = Done,
    workerState' = WorkerIdle,
    daemonReady' = daemonReady,
    j1State' = j1State,
    j1Retry' = j1Retry,
    j2Retry' = j2Retry,
  }

  action failJ1 = all {
    workerState == BusyJ1,
    j1State == Synthesizing,
    j1State' = if (j1Retry < MAX_RETRY) Queued else Failed,
    j1Retry' = if (j1Retry < MAX_RETRY) j1Retry + 1 else j1Retry,
    workerState' = WorkerIdle,
    daemonReady' = daemonReady,
    j2State' = j2State,
    j2Retry' = j2Retry,
  }

  action failJ2 = all {
    workerState == BusyJ2,
    j2State == Synthesizing,
    j2State' = if (j2Retry < MAX_RETRY) Queued else Failed,
    j2Retry' = if (j2Retry < MAX_RETRY) j2Retry + 1 else j2Retry,
    workerState' = WorkerIdle,
    daemonReady' = daemonReady,
    j1State' = j1State,
    j1Retry' = j1Retry,
  }

  action cancelJ1 = all {
    j1State == Queued or j1State == Synthesizing,
    j1State' = Canceled,
    workerState' = if (workerState == BusyJ1) WorkerIdle else workerState,
    daemonReady' = daemonReady,
    j2State' = j2State,
    j1Retry' = j1Retry,
    j2Retry' = j2Retry,
  }

  action cancelJ2 = all {
    j2State == Queued or j2State == Synthesizing,
    j2State' = Canceled,
    workerState' = if (workerState == BusyJ2) WorkerIdle else workerState,
    daemonReady' = daemonReady,
    j1State' = j1State,
    j1Retry' = j1Retry,
    j2Retry' = j2Retry,
  }

  action resetJ1 = all {
    j1State == Done or j1State == Failed or j1State == Canceled,
    j1State' = JobIdle,
    j1Retry' = 0,
    daemonReady' = daemonReady,
    workerState' = workerState,
    j2State' = j2State,
    j2Retry' = j2Retry,
  }

  action resetJ2 = all {
    j2State == Done or j2State == Failed or j2State == Canceled,
    j2State' = JobIdle,
    j2Retry' = 0,
    daemonReady' = daemonReady,
    workerState' = workerState,
    j1State' = j1State,
    j1Retry' = j1Retry,
  }

  action stutter = all {
    daemonReady' = daemonReady,
    workerState' = workerState,
    j1State' = j1State,
    j2State' = j2State,
    j1Retry' = j1Retry,
    j2Retry' = j2Retry,
  }

  action step = any {
    daemonUp,
    daemonDown,
    enqueueJ1,
    enqueueJ2,
    startJ1,
    startJ2,
    finishJ1,
    finishJ2,
    failJ1,
    failJ2,
    cancelJ1,
    cancelJ2,
    resetJ1,
    resetJ2,
    stutter,
  }

  val typeOK = j1Retry >= 0 and j1Retry <= MAX_RETRY and j2Retry >= 0 and j2Retry <= MAX_RETRY

  val atMostOneSynthesizing =
    not(j1State == Synthesizing and j2State == Synthesizing)

  val workerMatchesSynthesis =
    (workerState == BusyJ1 implies j1State == Synthesizing) and
    (workerState == BusyJ2 implies j2State == Synthesizing) and
    (workerState == WorkerIdle implies (j1State != Synthesizing and j2State != Synthesizing))

  // The old `ProgressSpec` used per-job weak fairness. A single combined
  // fairness set would be satisfiable by progress on one job alone, so keep
  // them separate.
  action advanceJ1 = any {
    finishJ1,
    failJ1,
    cancelJ1,
    daemonDown,
  }

  action advanceJ2 = any {
    finishJ2,
    failJ2,
    cancelJ2,
    daemonDown,
  }

  val vars = (daemonReady, workerState, j1State, j2State, j1Retry, j2Retry)

  temporal eventuallyLeavesBusyWorker =
    (weakFair(advanceJ1, vars) and weakFair(advanceJ2, vars)) implies
      always(workerState != WorkerIdle implies eventually(workerState == WorkerIdle))
}
