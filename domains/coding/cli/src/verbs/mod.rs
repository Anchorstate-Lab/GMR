pub mod accept;
pub mod anchor;
pub mod bind;
pub mod check;
pub mod close;
pub mod cobound;
pub mod doctor;
pub mod edges;
pub mod export;
pub mod health;
pub mod import;
pub mod init;
pub mod link;
pub mod observe;
pub mod open;
pub mod pass;
pub mod probes;
pub mod publish;
pub mod read;
pub mod reaffirm;
pub mod rebase;
pub mod reprobe;
pub mod requeue;
pub mod restate;
pub mod reterminal;
pub mod retransition;
pub mod status;
pub mod sync;

use gmr::{AnchorKey, Change, ContentHash, Revised, Runtime, State};

use crate::error::CliError;

/// Clear the state back to the position alone, then look. The shape's own
/// capture rule writes the baseline, so what gets pinned is a reading taken
/// now — not whatever the last pass happened to leave in the state. If the
/// target is no longer there the capture rule says so instead of pinning it.
pub(crate) async fn recapture(
    rt: &Runtime,
    key: &AnchorKey,
    why: &[u8],
) -> Result<Revised, CliError> {
    let view = rt.read(key).await?;
    let blank = State::new(serde_json::json!({ "position": view.state.position() }));
    let revised = rt.revise(key, Change::Restate { state: blank }, why).await?;
    rt.observe(key).await?;
    Ok(revised)
}

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
