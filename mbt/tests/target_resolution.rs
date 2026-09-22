//! Model-based test for the target-resolution decision logic.
//!
//! The driver calls the **real** `voicevox_cli` functions with the same fixture
//! catalog that `modeling/quint/TargetResolution.qnt` models, and lets Quint
//! Connect compare the observed outcome against the spec state after every
//! step.
//!
//! The model->default-style map is built by the **production**
//! `build_model_default_style_map`, so this test also covers the "smallest
//! style ID" rule: changing `.min` to `.max` changes `resolve_target`'s output
//! and fails the comparison.
//!
//! Keep the fixture below in sync with the spec.

// The `voicevox_cli` string/list types are feature-dependent (`String`/`Vec`
// by default, `CompactString`/`SmallVec` with `fast-strings`/`small-vectors`),
// so the `.into()` conversions are intentional even when they are no-ops.
#![allow(clippy::useless_conversion)]

use quint_connect::*;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use voicevox_cli::infrastructure::daemon::state::catalog::{
    TargetResolution, build_model_default_style_map, resolve_target,
};
use voicevox_cli::infrastructure::voicevox::{AvailableModel, Speaker, Style};

/// style ID -> model ID (mirrors `STYLE_TO_MODEL` in the spec).
const STYLE_TO_MODEL: [(u32, u32); 3] = [(2, 1), (11, 1), (21, 2)];
/// Available model IDs (mirrors the spec fixture); model 3 has no style.
const MODEL_IDS: [u32; 3] = [1, 2, 3];
/// Known model with no style endpoint (mirrors `NO_STYLE_MODEL_IDS`).
const NO_STYLE_MODEL_ID: u32 = 3;
/// Unknown target (mirrors `UNKNOWN_IDS`).
const UNKNOWN_ID: u32 = 999;

/// Mirrors the spec's `Outcome` sum type.
#[derive(Clone, PartialEq, Eq, Debug, Default, Deserialize)]
#[serde(tag = "tag", content = "value")]
enum Outcome {
    StyleTarget {
        #[serde(rename = "styleId")]
        style_id: i64,
        #[serde(rename = "modelId")]
        model_id: i64,
    },
    ModelTarget {
        #[serde(rename = "styleId")]
        style_id: i64,
        #[serde(rename = "modelId")]
        model_id: i64,
    },
    #[default]
    Missing,
}

impl Outcome {
    fn style(style_id: u32, model_id: u32) -> Self {
        Self::StyleTarget {
            style_id: i64::from(style_id),
            model_id: i64::from(model_id),
        }
    }

    fn model(model_id: u32, style_id: u32) -> Self {
        Self::ModelTarget {
            style_id: i64::from(style_id),
            model_id: i64::from(model_id),
        }
    }
}

fn style_to_model_map() -> HashMap<u32, u32> {
    STYLE_TO_MODEL.into_iter().collect()
}

/// Mirrors the `SPEAKERS` fixture in the spec: model 1 has styles {11, 2}
/// (unsorted so the minimum is meaningful), model 2 has style 21, model 3 has no
/// styles.
fn speakers() -> Vec<Speaker> {
    fn style(id: u32) -> Style {
        Style {
            name: format!("style-{id}").into(),
            id,
            style_type: None,
        }
    }

    fn speaker(name: &str, style_ids: &[u32]) -> Speaker {
        Speaker {
            name: name.to_string().into(),
            speaker_uuid: Default::default(),
            styles: style_ids
                .iter()
                .copied()
                .map(style)
                .collect::<Vec<_>>()
                .into(),
            version: Default::default(),
        }
    }

    vec![
        speaker("model-1", &[11, 2]),
        speaker("model-2", &[21]),
        speaker("model-3", &[]),
    ]
}

/// Derived with the production builder (must equal the spec's
/// `MODEL_DEFAULT_STYLE`).
fn model_default_style_map() -> HashMap<u32, u32> {
    build_model_default_style_map(&speakers(), &style_to_model_map())
}

fn available_models() -> Vec<AvailableModel> {
    MODEL_IDS
        .iter()
        .map(|&model_id| AvailableModel {
            model_id,
            file_path: PathBuf::from(format!("/nonexistent/{model_id}.vvm")),
            speakers: Default::default(),
        })
        .collect()
}

#[derive(Default)]
struct TargetResolutionDriver {
    /// Observed outcome, always derived from the production function.
    outcome: Outcome,
}

impl TargetResolutionDriver {
    fn reset(&mut self) {
        self.outcome = Outcome::Missing;
    }

    /// Calls the production resolution functions and records the observed result.
    fn resolve(&mut self, requested_id: i64) {
        let requested = u32::try_from(requested_id).expect("spec IDs fit in u32");
        let style_to_model = style_to_model_map();
        let model_default_style = model_default_style_map();
        let models = available_models();

        self.outcome =
            match resolve_target(&style_to_model, &model_default_style, &models, requested) {
                TargetResolution::Exists { style_id, model_id } => {
                    if style_to_model.contains_key(&requested) {
                        Outcome::style(style_id, model_id)
                    } else {
                        Outcome::model(model_id, style_id)
                    }
                }
                TargetResolution::Missing { .. } => Outcome::Missing,
            };
    }
}

impl State<TargetResolutionDriver> for Outcome {
    fn from_driver(driver: &TargetResolutionDriver) -> Result<Self> {
        Ok(driver.outcome.clone())
    }
}

impl Driver for TargetResolutionDriver {
    type State = Outcome;

    fn config() -> Config {
        Config {
            state: &["outcome"],
            ..Config::default()
        }
    }

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => self.reset(),
            resolveStyle(id) => self.resolve(id),
            resolveModel(id) => self.resolve(id),
            resolveNoStyleModel(id) => self.resolve(id),
            resolveUnknown(id) => self.resolve(id),
            // Fixed scenarios enter through a scenario-specific init action.
            initCollision => self.resolve(2),
            initModelDefault => self.resolve(1),
            initNoStyleModel => self.resolve(i64::from(NO_STYLE_MODEL_ID)),
            initUnknown => self.resolve(i64::from(UNKNOWN_ID)),
            hold => (),
            _ => (),
        })
    }
}

/// Random exploration of the resolution contract.
#[quint_run(
    spec = "../modeling/quint/TargetResolution.qnt",
    max_samples = 200,
    max_steps = 6
)]
fn target_resolution_simulation() -> impl Driver {
    TargetResolutionDriver::default()
}

/// Fixed regression scenario: id 2 is both a style and a model ID; the style
/// must win.
#[quint_run(
    spec = "../modeling/quint/TargetResolution.qnt",
    init = "initCollision",
    step = "hold",
    max_samples = 1,
    max_steps = 1
)]
fn target_resolution_id_collision() -> impl Driver {
    TargetResolutionDriver::default()
}

/// Fixed regression scenario: a model-only ID resolves to its default style,
/// which is the **minimum** of its style IDs (production builder).
#[quint_run(
    spec = "../modeling/quint/TargetResolution.qnt",
    init = "initModelDefault",
    step = "hold",
    max_samples = 1,
    max_steps = 1
)]
fn target_resolution_model_default_style() -> impl Driver {
    TargetResolutionDriver::default()
}

/// Fixed regression scenario: a known model with no style endpoint is missing.
#[quint_run(
    spec = "../modeling/quint/TargetResolution.qnt",
    init = "initNoStyleModel",
    step = "hold",
    max_samples = 1,
    max_steps = 1
)]
fn target_resolution_no_style_model() -> impl Driver {
    TargetResolutionDriver::default()
}

/// Fixed regression scenario: an unknown ID is missing.
#[quint_run(
    spec = "../modeling/quint/TargetResolution.qnt",
    init = "initUnknown",
    step = "hold",
    max_samples = 1,
    max_steps = 1
)]
fn target_resolution_unknown() -> impl Driver {
    TargetResolutionDriver::default()
}
