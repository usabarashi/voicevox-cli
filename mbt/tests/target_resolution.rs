//! Model-based test for the target-resolution decision logic.
//!
//! The driver calls the **real** `voicevox_cli::...::resolve_target` function
//! with the same fixture catalog that `modeling/quint/TargetResolution.qnt`
//! models, and lets Quint Connect compare the observed outcome against the
//! spec state after every step.
//!
//! Keep the fixture below in sync with the spec.

use quint_connect::*;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use voicevox_cli::infrastructure::daemon::state::catalog::{TargetResolution, resolve_target};
use voicevox_cli::infrastructure::voicevox::AvailableModel;

/// style ID -> model ID (mirrors `STYLE_TO_MODEL` in the spec).
const STYLE_TO_MODEL: [(u32, u32); 2] = [(2, 1), (11, 1)];
/// model ID -> default style ID (mirrors `MODEL_DEFAULT_STYLE` in the spec).
const MODEL_DEFAULT_STYLE: [(u32, u32); 2] = [(1, 2), (2, 21)];
/// Known model IDs (mirrors the models the spec can resolve through).
const MODEL_IDS: [u32; 2] = [1, 2];

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

fn model_default_style_map() -> HashMap<u32, u32> {
    MODEL_DEFAULT_STYLE.into_iter().collect()
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

    /// Calls the production resolution function and records the observed result.
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
            resolveUnknown(id) => self.resolve(id),
            // Fixed scenarios enter through a scenario-specific init action.
            initCollision => self.resolve(2),
            initModelDefault => self.resolve(1),
            initUnknown => self.resolve(999),
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

/// Fixed regression scenario: a model-only ID resolves to its default style.
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
