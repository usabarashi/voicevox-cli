// OpenJTalk dictionary resource lifecycle model.
//
// Production loads the dictionary with a single `openjtalk::initialize()` call
// (`src/infrastructure/openjtalk.rs`, reached from `core.rs`): there is **no
// retry**. Retries belong to the installer (`Download.qnt` /
// `StartupResources.qnt`), not to the load step.
//
// Properties: the load reaches a terminal state (`loadTerminates`), and once
// ready it is never unloaded (`loadedStaysReady`).

module Dictionary {
  type LoadState = Missing | Loading | Ready | Failed

  var state: LoadState

  action init = all {
    state' = Missing,
  }

  action beginLoad = all {
    state == Missing,
    state' = Loading,
  }

  action loadOk = all {
    state == Loading,
    state' = Ready,
  }

  action loadFail = all {
    state == Loading,
    state' = Failed,
  }

  action stutter = all {
    state' = state,
  }

  action step = any {
    beginLoad,
    loadOk,
    loadFail,
    stutter,
  }

  action advance = any {
    beginLoad,
    loadOk,
    loadFail,
  }

  val vars = (state, state)

  temporal loadTerminates =
    weakFair(advance, vars) implies eventually(state == Ready or state == Failed)

  temporal loadedStaysReady =
    always(state == Ready implies always(state == Ready))
}
