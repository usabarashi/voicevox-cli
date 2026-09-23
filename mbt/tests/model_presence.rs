//! Model-based test for startup voice-model presence detection.
//!
//! The driver points a fresh `model_presence_probe` process at a controlled
//! temporary directory (`VOICEVOX_MODELS_DIR`) and lets the **production**
//! predicates decide the abstract state:
//!
//! * `has_available_models()` drives `modelState` (`Ready` iff at least one
//!   `.vvm` file is discoverable),
//! * `missing_startup_resources()` drives `reportedMissing` (it contains
//!   `"models"` iff the resource is not ready).
//!
//! The predicates run out of process so this driver never calls
//! `std::env::set_var`: that is unsafe under concurrency, and a test harness
//! cannot serialise every reader (including native libraries) in its process.
//!
//! Regression: a models directory that exists but contains no `.vvm` files must
//! be `Missing` and reported missing. The previous `find_models_dir().is_err()`
//! predicate mapped that concrete state to `Ready`, which allowed the daemon to
//! start with an empty catalog and reject every style/model ID.
//!
//! Keep the fixtures below in sync with `modeling/quint/ModelPresence.qnt`.

use quint_connect::*;
use serde::Deserialize;
use std::path::Path;
use std::process::Command;

/// Mirrors the spec's `Presence` sum type.
#[derive(Clone, PartialEq, Eq, Debug, Default, Deserialize)]
#[serde(tag = "tag", content = "value")]
enum Presence {
    #[default]
    Missing,
    Ready,
}

/// Mirrors the spec's state variables `modelState` / `reportedMissing`.
#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
struct ModelPresenceState {
    #[serde(rename = "modelState")]
    model_state: Presence,
    #[serde(rename = "reportedMissing")]
    reported_missing: bool,
}

/// Runs both production predicates in a fresh `model_presence_probe` process
/// with `VOICEVOX_MODELS_DIR` set to `dir`, returning
/// `(available, reported_missing)`.
fn observe_models_dir(dir: &Path) -> (bool, bool) {
    let output = Command::new(env!("CARGO_BIN_EXE_model_presence_probe"))
        .env("VOICEVOX_MODELS_DIR", dir)
        .output()
        .expect("run model_presence_probe");

    assert!(
        output.status.success(),
        "model_presence_probe failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("probe stdout is UTF-8");
    let mut fields = stdout.split_whitespace();
    let available = fields.next() == Some("true");
    let reported_missing = fields.next() == Some("true");
    (available, reported_missing)
}

fn write_vvm(dir: &Path) {
    std::fs::write(dir.join("3.vvm"), b"model").expect("write vvm");
}

fn write_nested_vvm(dir: &Path) {
    let vvms = dir.join("vvms");
    std::fs::create_dir_all(&vvms).expect("create vvms dir");
    std::fs::write(vvms.join("0.vvm"), b"model").expect("write nested vvm");
}

#[derive(Default)]
struct ModelPresenceDriver {
    /// Keeps the fixture directory alive for the duration of the step.
    models_dir: Option<tempfile::TempDir>,
    model_state: Presence,
    reported_missing: bool,
}

impl ModelPresenceDriver {
    /// Builds a fresh fixture directory, applies `prepare`, runs the production
    /// predicates against it, and records the observed state.
    fn observe(&mut self, prepare: impl FnOnce(&Path)) {
        let dir = tempfile::tempdir().expect("tempdir");
        prepare(dir.path());

        let (available, reported_missing) = observe_models_dir(dir.path());
        assert_eq!(
            reported_missing, !available,
            "missing_startup_resources and has_available_models disagree"
        );

        self.model_state = if available {
            Presence::Ready
        } else {
            Presence::Missing
        };
        self.reported_missing = reported_missing;
        self.models_dir = Some(dir);
    }
}

impl State<ModelPresenceDriver> for ModelPresenceState {
    fn from_driver(driver: &ModelPresenceDriver) -> Result<Self> {
        Ok(Self {
            model_state: driver.model_state.clone(),
            reported_missing: driver.reported_missing,
        })
    }
}

impl Driver for ModelPresenceDriver {
    type State = ModelPresenceState;

    // clippy 1.98 flags quint-connect's `switch!` expansion as `no_effect`.
    #[allow(clippy::no_effect)]
    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => self.observe(|_| {}),
            installModels => self.observe(write_vvm),
            stutter => (),
            hold => (),
            scenarioMissing => self.observe(|_| {}),
            scenarioReady => self.observe(write_vvm),
            scenarioReadyNested => self.observe(write_nested_vvm),
            _ => (),
        })
    }
}

/// Random exploration: the resource becomes ready only once a `.vvm` exists.
#[quint_run(
    spec = "../modeling/quint/ModelPresence.qnt",
    max_samples = 50,
    max_steps = 4
)]
fn model_presence_simulation() -> impl Driver {
    ModelPresenceDriver::default()
}

/// Regression: a models directory that exists but holds no `.vvm` is `Missing`.
#[quint_run(
    spec = "../modeling/quint/ModelPresence.qnt",
    init = "scenarioMissing",
    step = "hold",
    max_samples = 1,
    max_steps = 1
)]
fn model_presence_missing_when_directory_has_no_vvm() -> impl Driver {
    ModelPresenceDriver::default()
}

/// A `.vvm` directly in the models directory is `Ready`.
#[quint_run(
    spec = "../modeling/quint/ModelPresence.qnt",
    init = "scenarioReady",
    step = "hold",
    max_samples = 1,
    max_steps = 1
)]
fn model_presence_ready_when_vvm_present() -> impl Driver {
    ModelPresenceDriver::default()
}

/// A `.vvm` in the production `vvms/` subdirectory is `Ready`.
#[quint_run(
    spec = "../modeling/quint/ModelPresence.qnt",
    init = "scenarioReadyNested",
    step = "hold",
    max_samples = 1,
    max_steps = 1
)]
fn model_presence_ready_when_nested_vvm_present() -> impl Driver {
    ModelPresenceDriver::default()
}
