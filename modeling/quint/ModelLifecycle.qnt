// Daemon per-request model load/unload lifecycle.
//
// Mirrors `DaemonSynthesisExecutor::synthesize` and `ModelUnloadGuard` in
// `src/infrastructure/daemon/state/executor.rs`: a model is loaded for one
// request and unloaded by an RAII guard, including on error, panic, or task
// cancellation. Models are never cached in memory (see AGENTS.md).
//
// The key contract is that a loaded model is never leaked: under fairness of
// the progress actions, `loaded` eventually becomes false.

module ModelLifecycle {
  type Phase = Idle | Loading | Loaded | Synthesizing | Unloading | Failed

  var phase: Phase
  var loaded: bool

  action init = all {
    phase' = Idle,
    loaded' = false,
  }

  action load = all {
    phase == Idle,
    phase' = Loading,
    loaded' = false,
  }

  action loadOk = all {
    phase == Loading,
    phase' = Loaded,
    loaded' = true,
  }

  action loadFail = all {
    phase == Loading,
    phase' = Failed,
    loaded' = false,
  }

  action synthesize = all {
    phase == Loaded,
    loaded,
    phase' = Synthesizing,
    loaded' = loaded,
  }

  // The request ends (success or failure); the guard begins unloading.
  action synthOk = all {
    phase == Synthesizing,
    phase' = Unloading,
    loaded' = loaded,
  }

  action synthFail = all {
    phase == Synthesizing,
    phase' = Unloading,
    loaded' = loaded,
  }

  // The guard unloads. `unloadFail` models a failed unload: the model is still
  // released from the daemon's point of view (the guard is consumed).
  action unload = all {
    phase == Unloading,
    loaded,
    phase' = Idle,
    loaded' = false,
  }

  action unloadFail = all {
    phase == Unloading,
    loaded,
    phase' = Idle,
    loaded' = false,
  }

  action stutter = all {
    phase' = phase,
    loaded' = loaded,
  }

  action step = any {
    load,
    loadOk,
    loadFail,
    synthesize,
    synthOk,
    synthFail,
    unload,
    unloadFail,
    stutter,
  }

  val loadedImpliesPhase =
    loaded implies (phase == Loaded or phase == Synthesizing or phase == Unloading)

  // NOTE: a separate `noLeakedModel` invariant was removed because it was
  // implied by `loadedImpliesPhase`; the actual leak guarantee is the temporal
  // `eventuallyUnloaded` below.

  action advance = any {
    loadOk,
    loadFail,
    synthesize,
    synthOk,
    synthFail,
    unload,
    unloadFail,
  }

  val vars = (phase, loaded)

  temporal eventuallyUnloaded =
    weakFair(advance, vars) implies
      always(loaded implies eventually(not(loaded)))

  // ---- Fixed scenarios (executor MBT) -------------------------------------
  //
  // Terminal states of one daemon synthesis request, used by
  // `mbt/tests/model_lifecycle.rs`. The driver runs the production executor with
  // a recording fake runtime and observes that no model stays loaded.

  action hold = all {
    phase' = phase,
    loaded' = loaded,
  }

  // The request completed (success or synthesis failure); the guard unloaded.
  action scenarioUnloaded = all {
    phase' = Idle,
    loaded' = false,
  }

  // The model failed to load, so nothing is loaded.
  action scenarioLoadFailed = all {
    phase' = Failed,
    loaded' = false,
  }

  // The synthesis failed after loading; the guard still unloaded.
  action scenarioUnloadedAfterSynthFailure = all {
    phase' = Idle,
    loaded' = false,
  }
}
