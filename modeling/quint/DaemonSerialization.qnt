// Daemon-side synthesis serialization (replaces `SynthesisParallel.qnt`).
//
// Mirrors `SerializedSynthesisPolicy` in
// `src/infrastructure/daemon/state/policy.rs`: a single async mutex serializes
// model load + synthesis + unload, so at most one synthesis runs at a time.
//
// The previous `SynthesisParallel.qnt` (ported from `SynthesisParallel.tla`)
// modelled cancellation releasing a busy worker and daemon-side retry with
// requeue. Neither exists in production: there is no cancel request on the
// daemon IPC (`DaemonRequest` has no cancel variant), the server finishes
// `handle_request` before attempting to write a response, and retries are
// owned by the client. This module therefore models the *actual* contract:
// one worker, queued jobs, no cancel, no daemon-side retry.
//
// Preserved property: at most one job is Synthesizing at any time. The old
// `WorkerMatchesSynthesis` is preserved as `workerMatchesSynthesis`.

module DaemonSerialization {
  type JobState = JobWaiting | Synthesizing | Done | Failed
  type WorkerState = WorkerIdle | Busy

  var worker: WorkerState
  var j1: JobState
  var j2: JobState

  action init = all {
    worker' = WorkerIdle,
    j1' = JobWaiting,
    j2' = JobWaiting,
  }

  action startJ1 = all {
    worker == WorkerIdle,
    j1 == JobWaiting,
    worker' = Busy,
    j1' = Synthesizing,
    j2' = j2,
  }

  action startJ2 = all {
    worker == WorkerIdle,
    j2 == JobWaiting,
    worker' = Busy,
    j1' = j1,
    j2' = Synthesizing,
  }

  action finishJ1 = all {
    worker == Busy,
    j1 == Synthesizing,
    worker' = WorkerIdle,
    j1' = Done,
    j2' = j2,
  }

  action finishJ2 = all {
    worker == Busy,
    j2 == Synthesizing,
    worker' = WorkerIdle,
    j1' = j1,
    j2' = Done,
  }

  action failJ1 = all {
    worker == Busy,
    j1 == Synthesizing,
    worker' = WorkerIdle,
    j1' = Failed,
    j2' = j2,
  }

  action failJ2 = all {
    worker == Busy,
    j2 == Synthesizing,
    worker' = WorkerIdle,
    j1' = j1,
    j2' = Failed,
  }

  action resetJ1 = all {
    j1 == Done or j1 == Failed,
    j1' = JobWaiting,
    worker' = worker,
    j2' = j2,
  }

  action resetJ2 = all {
    j2 == Done or j2 == Failed,
    j2' = JobWaiting,
    worker' = worker,
    j1' = j1,
  }

  action stutter = all {
    worker' = worker,
    j1' = j1,
    j2' = j2,
  }

  action step = any {
    startJ1,
    startJ2,
    finishJ1,
    finishJ2,
    failJ1,
    failJ2,
    resetJ1,
    resetJ2,
    stutter,
  }

  // The mutex admits one synthesis at a time; the client owns retries, so a
  // failed job returns to `JobWaiting` only through an explicit reset (a new
  // request), never by daemon-side requeue.
  val atMostOneSynthesizing =
    not(j1 == Synthesizing and j2 == Synthesizing)

  val workerMatchesSynthesis =
    (worker == Busy) == (j1 == Synthesizing or j2 == Synthesizing)

  // Per-job weak fairness of the completion/failure actions. A single combined
  // fairness set would be satisfiable by progress on one job alone.
  action advanceJ1 = any {
    finishJ1,
    failJ1,
  }

  action advanceJ2 = any {
    finishJ2,
    failJ2,
  }

  val vars = (worker, j1, j2)

  temporal eventuallyLeavesBusyWorker =
    (weakFair(advanceJ1, vars) and weakFair(advanceJ2, vars)) implies
      always(worker == Busy implies eventually(worker == WorkerIdle))

  // ---- Fixed scenario (real-daemon concurrency MBT) -----------------------
  //
  // Two concurrent client syntheses both complete through the serialized daemon
  // worker, used by `mbt/tests/daemon_serialization.rs`.

  action hold = all {
    worker' = worker,
    j1' = j1,
    j2' = j2,
  }

  action scenarioBothDone = all {
    worker' = WorkerIdle,
    j1' = Done,
    j2' = Done,
  }
}
