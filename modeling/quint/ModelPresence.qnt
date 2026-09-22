// Voice-model presence detection (startup resource readiness).
//
// Mirrors the production predicate that decides whether the voice model
// resource is installed:
//
//   * `has_available_models()` (`infrastructure::voicevox`) is true iff at
//     least one `.vvm` file is discoverable under the models directory,
//   * `missing_startup_resources()` (`infrastructure::download`) reports
//     `"models"` iff that predicate is false.
//
// The regression this pins down: a models *directory* that merely exists but
// holds no `.vvm` file must map to `Missing`, not `Ready`. The previous
// `find_models_dir().is_err()` predicate mapped that concrete state to `Ready`,
// which let the daemon start with an empty catalog and reject every style/model
// ID (`Unknown style/model ID 3`).
//
// The concrete filesystem state -> `modelState` mapping is the executable
// refinement evidence in `mbt/tests/model_presence.rs`; keep the scenario
// fixtures there in sync.

module ModelPresence {
  type Presence = Missing | Ready

  var modelState: Presence
  var reportedMissing: bool

  // The two production observations are one contract: the resource is reported
  // missing exactly when it is not ready. The MBT driver derives both from the
  // production functions, so a disagreement fails the comparison.
  val reportMatchesPresence = reportedMissing == (modelState == Missing)

  action init = all {
    modelState' = Missing,
    reportedMissing' = true,
  }

  // Installing at least one `.vvm` flips the resource to `Ready`. A directory
  // that exists is not enough (see the fixed `scenarioMissing`).
  action installModels = all {
    modelState == Missing,
    modelState' = Ready,
    reportedMissing' = false,
  }

  action stutter = all {
    modelState' = modelState,
    reportedMissing' = reportedMissing,
  }

  action step = any {
    installModels,
    stutter,
  }

  // ---- Fixed regression scenarios (init/step variants) --------------------
  //
  // Driven by `#[quint_run(init = ..., step = "hold")]`. Each scenario maps to a
  // concrete directory fixture in `mbt/tests/model_presence.rs`.

  // Holds the current state; used as the `step` of a one-shot scenario.
  action hold = all {
    modelState' = modelState,
    reportedMissing' = reportedMissing,
  }

  // The models directory exists but contains no `.vvm` file.
  action scenarioMissing = all {
    modelState' = Missing,
    reportedMissing' = true,
  }

  // A `.vvm` file sits directly in the models directory.
  action scenarioReady = all {
    modelState' = Ready,
    reportedMissing' = false,
  }

  // A `.vvm` file sits in the production `vvms/` subdirectory.
  action scenarioReadyNested = all {
    modelState' = Ready,
    reportedMissing' = false,
  }
}
