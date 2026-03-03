use anyhow::Result;

use crate::infrastructure::download::{
    update_dictionary_only as run_update_dictionary_only,
    update_models_only as run_update_models_only,
    update_specific_model as run_update_specific_model, UpdateKind,
};
use crate::interface::{AppOutput, StdAppOutput};

fn print_update_outcome(kind: UpdateKind, used_fallback: bool, output: &dyn AppOutput) {
    if used_fallback {
        output.info("Requested update mode was unavailable. Fallback mode was used.");
    }

    match kind {
        UpdateKind::Models => output.info("Voice models updated successfully."),
        UpdateKind::Dictionary => output.info("Dictionary updated successfully."),
        UpdateKind::SpecificModel(model_id) => {
            output.info(&format!("Model {model_id} update completed."));
        }
    }
}

pub async fn update_models_only() -> Result<()> {
    let output = StdAppOutput;
    let outcome = run_update_models_only().await?;
    print_update_outcome(outcome.kind, outcome.used_fallback, &output);
    Ok(())
}

pub async fn update_dictionary_only() -> Result<()> {
    let output = StdAppOutput;
    let outcome = run_update_dictionary_only().await?;
    print_update_outcome(outcome.kind, outcome.used_fallback, &output);
    Ok(())
}

pub async fn update_specific_model(model_id: u32) -> Result<()> {
    let output = StdAppOutput;
    let outcome = run_update_specific_model(model_id).await?;
    print_update_outcome(outcome.kind, outcome.used_fallback, &output);
    Ok(())
}
