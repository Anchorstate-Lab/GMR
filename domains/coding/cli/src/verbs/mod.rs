pub mod bind;
pub mod close;
pub mod cobound;
pub mod doctor;
pub mod edges;
pub mod health;
pub mod link;
pub mod observe;
pub mod open;
pub mod pass;
pub mod publish;
pub mod read;
pub mod reaffirm;
pub mod reprobe;
pub mod requeue;
pub mod restate;
pub mod reterminal;
pub mod retransition;
pub mod sync;

use gmr::{AnchorKey, ContentHash, Runtime};

use crate::error::CliError;

pub(crate) async fn memories_on(rt: &Runtime, key: &AnchorKey) -> Result<Vec<String>, CliError> {
    Ok(rt
        .memory()
        .bindings_on(key)
        .await?
        .into_iter()
        .map(|b| b.binding.reference.external_id.into_inner())
        .collect())
}

pub(crate) fn sealed(context: &ContentHash, rationale: &ContentHash) {
    println!(
        "  context   {} (captured by substrate, cannot be forged)",
        &context.as_str()[..12]
    );
    println!(
        "  rationale {} (written by you; substrate only preserves tamper evidence)",
        &rationale.as_str()[..12]
    );
}
