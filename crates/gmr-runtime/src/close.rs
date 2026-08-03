use chrono::Utc;
use gmr_core::{AnchorKey, Entry, fold};
use gmr_store::Fence;

use crate::assembly::Runtime;
use crate::error::RuntimeError;
use crate::log::AnchorLog;
use crate::memory::MemoryLens;
use crate::seal_context;

impl Runtime {
    pub async fn close(&self, key: &AnchorKey, rationale: &[u8]) -> Result<(), RuntimeError> {
        close(&self.log, &self.memory, key, rationale).await
    }
}

async fn close(
    log: &AnchorLog,
    memory: &MemoryLens,
    key: &AnchorKey,
    rationale: &[u8],
) -> Result<(), RuntimeError> {
    let entries = log.entries(key, 0).await?;
    let state = fold(&entries).ok_or_else(|| RuntimeError::NoSuchAnchor { key: key.clone() })?;

    if state.closed {
        return Err(RuntimeError::AnchorClosed { key: key.clone() });
    }

    let mut context = seal_context::base(&state);
    context["closed_by"] = serde_json::json!("author");
    let context = memory
        .seal(&serde_json::to_vec(&context).expect("the context always serialises"))
        .await?;
    let rationale = memory.seal(rationale).await?;

    log.append(
        key,
        &Entry::Close {
            context,
            rationale,
            at: Utc::now(),
        },
        Fence::Unleased,
    )
    .await?;
    Ok(())
}
