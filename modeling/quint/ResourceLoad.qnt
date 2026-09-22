// One-shot resource load lifecycle.
//
// Models a startup resource that production loads with a single initialize call
// and **no retry**:
//   - ONNX Runtime (`onnxruntime::initialize()` in `infrastructure/onnxruntime.rs`),
//   - the OpenJTalk dictionary (`openjtalk::initialize()` in
//     `infrastructure/openjtalk.rs`).
//
// Retries belong to the installer (`Download.qnt` / `StartupResources.qnt`), not
// to the load step. This consolidates the two previously identical specs
// (`ONNXRuntime.qnt` / `Dictionary.qnt`), which were merged because their state
// machines and properties were duplicates.
//
// Properties: the load reaches a terminal state (`loadTerminates`), and once
// ready the resource is never unloaded (`loadedStaysReady`).

module ResourceLoad {
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
