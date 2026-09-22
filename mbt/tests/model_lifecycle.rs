//! Model-based test for the daemon's per-request model load/unload contract.
//!
//! The driver runs the **production** `DaemonSynthesisExecutor` /
//! `SerializedSynthesisPolicy` with a recording fake `ModelRuntime` and lets
//! Quint Connect compare the observed lifecycle against
//! `modeling/quint/ModelLifecycle.qnt`.
//!
//! The observed contract is that no model stays loaded after a request,
//! including after a synthesis failure (the RAII unload guard).

use anyhow::{Result as AnyResult, anyhow};
use quint_connect::*;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use voicevox_cli::infrastructure::core::ModelRuntime;
use voicevox_cli::infrastructure::daemon::state::catalog::ModelCatalog;
use voicevox_cli::infrastructure::daemon::state::executor::DaemonSynthesisExecutor;
use voicevox_cli::infrastructure::daemon::state::policy::SerializedSynthesisPolicy;
use voicevox_cli::infrastructure::voicevox::AvailableModel;

const STYLE_ID: u32 = 3;
const MODEL_ID: u32 = 0;

#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
#[serde(tag = "tag", content = "value")]
enum Phase {
    Idle,
    Loading,
    Loaded,
    Synthesizing,
    Unloading,
    Failed,
}

#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
struct LifecycleState {
    phase: Phase,
    loaded: bool,
}

#[derive(Default)]
struct FakeState {
    loaded: AtomicBool,
    load_calls: AtomicUsize,
    synth_calls: AtomicUsize,
    unload_calls: AtomicUsize,
    fail_load: bool,
    fail_synth: bool,
}

impl FakeState {
    fn is_loaded(&self) -> bool {
        self.loaded.load(Ordering::SeqCst)
    }
}

#[derive(Clone)]
struct FakeCore {
    state: Arc<FakeState>,
}

impl ModelRuntime for FakeCore {
    fn load_model(&self, _model_id: u32) -> AnyResult<()> {
        self.state.load_calls.fetch_add(1, Ordering::SeqCst);
        if self.state.fail_load {
            return Err(anyhow!("injected load failure"));
        }
        self.state.loaded.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn synthesize(&self, _text: &str, _style_id: u32, _rate: f32) -> AnyResult<Vec<u8>> {
        self.state.synth_calls.fetch_add(1, Ordering::SeqCst);
        if self.state.fail_synth {
            return Err(anyhow!("injected synthesis failure"));
        }
        Ok(vec![1, 2, 3])
    }

    fn unload_model_by_path(&self, _model_path: &Path) -> AnyResult<()> {
        self.state.unload_calls.fetch_add(1, Ordering::SeqCst);
        self.state.loaded.store(false, Ordering::SeqCst);
        Ok(())
    }
}

fn catalog() -> ModelCatalog {
    ModelCatalog::from_parts(
        HashMap::from([(STYLE_ID, MODEL_ID)]),
        HashMap::from([(MODEL_ID, STYLE_ID)]),
        Vec::new(),
        vec![AvailableModel {
            model_id: MODEL_ID,
            file_path: PathBuf::from("/nonexistent/0.vvm"),
            speakers: Default::default(),
        }],
    )
}

struct LifecycleDriver {
    runtime: tokio::runtime::Runtime,
    policy: SerializedSynthesisPolicy<FakeCore>,
    catalog: ModelCatalog,
    state: Arc<FakeState>,
    phase: Phase,
    loaded: bool,
}

impl Default for LifecycleDriver {
    fn default() -> Self {
        let state = Arc::new(FakeState::default());
        let factory_state = Arc::clone(&state);
        let policy = SerializedSynthesisPolicy::new(DaemonSynthesisExecutor::with_factory(
            Box::new(move || {
                Ok(FakeCore {
                    state: Arc::clone(&factory_state),
                })
            }),
        ));
        Self {
            runtime: tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime"),
            policy,
            catalog: catalog(),
            state,
            phase: Phase::Idle,
            loaded: false,
        }
    }
}

impl LifecycleDriver {
    /// Replaces the fake's failure configuration and records the observed state
    /// after running one production request.
    fn run(&mut self, fail_load: bool, fail_synth: bool) {
        // A fresh fake state per run; the factory shares it with the executor.
        self.state = Arc::new(FakeState {
            fail_load,
            fail_synth,
            ..FakeState::default()
        });
        let factory_state = Arc::clone(&self.state);
        self.policy = SerializedSynthesisPolicy::new(DaemonSynthesisExecutor::with_factory(
            Box::new(move || {
                Ok(FakeCore {
                    state: Arc::clone(&factory_state),
                })
            }),
        ));

        let text = "テスト".to_string();
        let _ = self
            .runtime
            .block_on(self.policy.synthesize(&self.catalog, text, STYLE_ID, 1.0));

        // Observed: the guard must have unloaded any loaded model.
        self.loaded = self.state.is_loaded();
        self.phase = if fail_load && self.state.synth_calls.load(Ordering::SeqCst) == 0 {
            Phase::Failed
        } else {
            Phase::Idle
        };
    }

    fn reset(&mut self) {
        self.phase = Phase::Idle;
        self.loaded = false;
    }
}

impl State<LifecycleDriver> for LifecycleState {
    fn from_driver(driver: &LifecycleDriver) -> AnyResult<Self> {
        Ok(Self {
            phase: driver.phase.clone(),
            loaded: driver.loaded,
        })
    }
}

impl Driver for LifecycleDriver {
    type State = LifecycleState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => self.reset(),
            step => self.reset(),
            hold => (),
            scenarioUnloaded => self.run(false, false),
            scenarioUnloadedAfterSynthFailure => self.run(false, true),
            scenarioLoadFailed => self.run(true, false),
            _ => (),
        })
    }
}

/// A successful request unloads the model.
#[quint_run(
    spec = "../modeling/quint/ModelLifecycle.qnt",
    init = "scenarioUnloaded",
    step = "hold",
    max_samples = 1,
    max_steps = 1
)]
fn model_lifecycle_success() -> impl Driver {
    LifecycleDriver::default()
}

/// A synthesis failure still unloads the model (RAII guard).
#[quint_run(
    spec = "../modeling/quint/ModelLifecycle.qnt",
    init = "scenarioUnloadedAfterSynthFailure",
    step = "hold",
    max_samples = 1,
    max_steps = 1
)]
fn model_lifecycle_synth_failure() -> impl Driver {
    LifecycleDriver::default()
}

/// A load failure leaves no model loaded.
#[quint_run(
    spec = "../modeling/quint/ModelLifecycle.qnt",
    init = "scenarioLoadFailed",
    step = "hold",
    max_samples = 1,
    max_steps = 1
)]
fn model_lifecycle_load_failure() -> impl Driver {
    LifecycleDriver::default()
}
