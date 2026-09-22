// Integrated daemon request path: admission x serialized synthesis.
//
// Composes two per-concern models:
//   - server admission (`DaemonServer.qnt`): up to `MAX_IN_FLIGHT` requests are
//     admitted and each holds a server permit until it is answered,
//   - the serialized synthesis worker (`DaemonSerialization.qnt`): one mutex, so
//     at most one synthesis runs at a time.
//
// It proves the cross-layer guarantees no single spec can: the worker stays
// exclusive even with many admitted requests, and the permit accounting stays
// exact while requests wait for the worker.

module DaemonSynthesisPath {
  pure val MAX_IN_FLIGHT = 32
  pure val REQ_IDS = Set(0, 1, 2)

  type ReqPhase = Idle | Admitted | Waiting | Synthesizing | Done
  type WorkerState = WorkerIdle | WorkerBusy

  var phase: int -> ReqPhase
  var inFlight: int
  var worker: WorkerState

  action init = all {
    phase' = Map(0 -> Idle, 1 -> Idle, 2 -> Idle),
    inFlight' = 0,
    worker' = WorkerIdle,
  }

  // Admit a request (acquire a server permit).
  action admit = all {
    nondet id = oneOf(REQ_IDS)
    all {
      phase.get(id) == Idle,
      inFlight < MAX_IN_FLIGHT,
      phase' = phase.set(id, Admitted),
      inFlight' = inFlight + 1,
      worker' = worker,
    }
  }

  // The admitted request asks the serialized worker to synthesize.
  action requestWorker = all {
    nondet id = oneOf(REQ_IDS)
    all {
      phase.get(id) == Admitted,
      phase' = phase.set(id, Waiting),
      inFlight' = inFlight,
      worker' = worker,
    }
  }

  action start0 = all {
    phase.get(0) == Waiting,
    worker == WorkerIdle,
    phase' = phase.set(0, Synthesizing),
    inFlight' = inFlight,
    worker' = WorkerBusy,
  }

  action start1 = all {
    phase.get(1) == Waiting,
    worker == WorkerIdle,
    phase' = phase.set(1, Synthesizing),
    inFlight' = inFlight,
    worker' = WorkerBusy,
  }

  action start2 = all {
    phase.get(2) == Waiting,
    worker == WorkerIdle,
    phase' = phase.set(2, Synthesizing),
    inFlight' = inFlight,
    worker' = WorkerBusy,
  }

  action finish0 = all {
    phase.get(0) == Synthesizing,
    worker == WorkerBusy,
    phase' = phase.set(0, Done),
    inFlight' = inFlight,
    worker' = WorkerIdle,
  }

  action finish1 = all {
    phase.get(1) == Synthesizing,
    worker == WorkerBusy,
    phase' = phase.set(1, Done),
    inFlight' = inFlight,
    worker' = WorkerIdle,
  }

  action finish2 = all {
    phase.get(2) == Synthesizing,
    worker == WorkerBusy,
    phase' = phase.set(2, Done),
    inFlight' = inFlight,
    worker' = WorkerIdle,
  }

  // The response is written and the server permit released.
  action release = all {
    nondet id = oneOf(REQ_IDS)
    all {
      phase.get(id) == Done,
      phase' = phase.set(id, Idle),
      inFlight' = inFlight - 1,
      worker' = worker,
    }
  }

  action stutter = all {
    phase' = phase,
    inFlight' = inFlight,
    worker' = worker,
  }

  action step = any {
    admit,
    requestWorker,
    start0,
    start1,
    start2,
    finish0,
    finish1,
    finish2,
    release,
    stutter,
  }

  // ---- Safety (cross-layer) ----------------------------------------------

  val typeOK = inFlight >= 0 and inFlight <= MAX_IN_FLIGHT

  // The permit counter matches the requests that hold a permit (admitted and
  // not yet released).
  val inFlightMatchesHolding =
    inFlight ==
      (if (phase.get(0) == Admitted or phase.get(0) == Waiting or phase.get(0) == Synthesizing or phase.get(0) == Done) 1 else 0) +
      (if (phase.get(1) == Admitted or phase.get(1) == Waiting or phase.get(1) == Synthesizing or phase.get(1) == Done) 1 else 0) +
      (if (phase.get(2) == Admitted or phase.get(2) == Waiting or phase.get(2) == Synthesizing or phase.get(2) == Done) 1 else 0)

  // The serialized worker admits at most one synthesis.
  val atMostOneSynthesizing =
    (if (phase.get(0) == Synthesizing) 1 else 0) +
    (if (phase.get(1) == Synthesizing) 1 else 0) +
    (if (phase.get(2) == Synthesizing) 1 else 0) <= 1

  val workerBusyMatchesSynthesizing =
    (worker == WorkerBusy) ==
      ((if (phase.get(0) == Synthesizing) 1 else 0) +
       (if (phase.get(1) == Synthesizing) 1 else 0) +
       (if (phase.get(2) == Synthesizing) 1 else 0) == 1)

  // ---- Liveness -----------------------------------------------------------

  val vars = (phase, inFlight, worker)

  // A busy worker always becomes idle again (no stuck synthesis). Per-request
  // weak fairness is required; a disjunctive fairness set would not force the
  // specific client's `finish` action.
  temporal workerEventuallyIdle =
    (weakFair(finish0, vars) and weakFair(finish1, vars) and weakFair(finish2, vars)) implies
      always(worker == WorkerBusy implies eventually(worker == WorkerIdle))
}
