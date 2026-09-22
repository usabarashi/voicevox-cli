// ONNX Runtime resource lifecycle model.
//
// Production loads ONNX Runtime with a single `onnxruntime::initialize()` call
// (`src/infrastructure/onnxruntime.rs`, reached from `core.rs`): there is **no
// retry**. Retries belong to the installer (`Download.qnt` /
// `StartupResources.qnt`), not to the load step.
//
// Properties: the load reaches a terminal state (`loadTerminates`), and once
// ready it is never unloaded (`readyIsStable`).

module ONNXRuntime {
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

  temporal readyIsStable =
    always(state == Ready implies always(state == Ready))
}
