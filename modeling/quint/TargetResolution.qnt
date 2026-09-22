// Target-resolution model (Phase 1 walking skeleton).
//
// The fixture catalog below mirrors `mbt/tests/target_resolution.rs`. Both must
// stay in sync; the driver calls the real `voicevox_cli` function
// `resolve_target` with the same data, so a divergence shows up as a state
// mismatch rather than a silent pass.
//
// Resolution contract (see CONTRACT.md):
//   1. a style ID always wins over a model ID (id 2 is intentionally both),
//   2. otherwise a known model ID resolves to its smallest matching style ID,
//   3. otherwise the target is missing.
//
// Scenario variants are expressed as separate `init`/`step` action pairs, as
// planned in CONTRACT.md. They are driven via `#[quint_run(init = ..., step =
// ...)]` because `quint test` has no `--mbt` support in quint 0.32.0, which
// makes `#[quint_test]` unusable with this toolchain.

module TargetResolution {
  pure val STYLE_IDS = Set(2, 11)
  // Model IDs that are not also style IDs. Model 2 is excluded on purpose:
  // it collides with style 2 and must resolve through the style branch.
  pure val MODEL_ONLY_IDS = Set(1)
  pure val UNKNOWN_IDS = Set(999)

  pure val STYLE_TO_MODEL = Map(2 -> 1, 11 -> 1)
  pure val MODEL_DEFAULT_STYLE = Map(1 -> 2, 2 -> 21)

  type Outcome =
    | StyleTarget({ styleId: int, modelId: int })
    | ModelTarget({ styleId: int, modelId: int })
    | Missing

  var outcome: Outcome

  // ---- Simulation scenario (used by the default `#[quint_run]`) -----------

  action init = all {
    outcome' = Missing,
  }

  action resolveStyle = all {
    nondet id = oneOf(STYLE_IDS)
    outcome' = StyleTarget({ styleId: id, modelId: STYLE_TO_MODEL.get(id) }),
  }

  action resolveModel = all {
    nondet id = oneOf(MODEL_ONLY_IDS)
    outcome' = ModelTarget({
      styleId: MODEL_DEFAULT_STYLE.get(id),
      modelId: id,
    }),
  }

  action resolveUnknown = all {
    nondet id = oneOf(UNKNOWN_IDS)
    outcome' = Missing,
  }

  action step = any {
    resolveStyle,
    resolveModel,
    resolveUnknown,
  }

  // ---- Fixed regression scenarios (init/step variants) --------------------

  // Holds the current outcome; used as the `step` of a one-shot scenario.
  action hold = all {
    outcome' = outcome,
  }

  // id 2 is both a style and a model ID; the style branch must win.
  action initCollision = all {
    outcome' = StyleTarget({ styleId: 2, modelId: STYLE_TO_MODEL.get(2) }),
  }

  // Model-only id 1 resolves to its default style (2).
  action initModelDefault = all {
    outcome' = ModelTarget({
      styleId: MODEL_DEFAULT_STYLE.get(1),
      modelId: 1,
    }),
  }

  action initUnknown = all {
    outcome' = Missing,
  }
}
