use anyhow::Result;

use crate::infrastructure::download::{
    update_dictionary_only as run_update_dictionary_only,
    update_models_only as run_update_models_only, UpdateKind,
};
use crate::interface::{AppOutput, StdAppOutput};

fn print_update_outcome(kind: UpdateKind, used_fallback: bool, output: &dyn AppOutput) {
    if used_fallback {
        output.info("Requested update mode was unavailable. Fallback mode was used.");
    }

    match kind {
        UpdateKind::Models => output.info("Voice models updated successfully."),
        UpdateKind::Dictionary => output.info("Dictionary updated successfully."),
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
