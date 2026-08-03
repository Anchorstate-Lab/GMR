use chrono::Utc;
use gmr_core::{AnchorKey, Change, ContentHash, Entry, fold};
use gmr_store::Fence;

use crate::assembly::Runtime;
use crate::error::RuntimeError;
use crate::log::AnchorLog;
use crate::memory::MemoryLens;
use crate::seal_context;
use crate::translate::bind_warnings;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Revised {
    pub context: ContentHash,
    pub rationale: ContentHash,
    pub warnings: Vec<String>,
    pub incomparable_state: bool,
}

impl Runtime {
    pub async fn revise(
        &self,
        key: &AnchorKey,
        change: Change,
        rationale: &[u8],
    ) -> Result<Revised, RuntimeError> {
        revise(&self.log, &self.memory, key, change, rationale).await
    }
}

async fn revise(
    log: &AnchorLog,
    memory: &MemoryLens,
    key: &AnchorKey,
    change: Change,
    rationale: &[u8],
) -> Result<Revised, RuntimeError> {
    let entries = log.entries(key, 0).await?;
    let s = fold(&entries).ok_or_else(|| RuntimeError::NoSuchAnchor { key: key.clone() })?;

    if s.closed {
        return Err(RuntimeError::AnchorClosed { key: key.clone() });
    }

    let mut context = seal_context::base(&s);
    context["closed"] = serde_json::json!(s.closed);
    context["latest"] = serde_json::json!(s.latest);
    context["probe_declaration"] = serde_json::json!(s.anchor.probe.declaration_hash());
    context["evaluator_version"] = serde_json::json!(gmr_expr::EVALUATOR_VERSION);
    let context = memory
        .seal(&serde_json::to_vec(&context).expect("the context always serialises"))
        .await?;
    let rationale = memory.seal(rationale).await?;

    let warnings = match (&change, s.latest.as_ref()) {
        (Change::Retransition { transitions }, Some(latest)) => {
            let mut preview = s.anchor.clone();
            preview.transitions = transitions.clone();
            bind_warnings(&preview, latest)
        }
        _ => Vec::new(),
    };

    let incomparable_state =
        matches!(change, Change::Reprobe { .. }) && !s.state.as_value().is_null();

    log.append(
        key,
        &Entry::Revise {
            change,
            context: context.clone(),
            rationale: rationale.clone(),
            at: Utc::now(),
        },
        Fence::Unleased,
    )
    .await?;

    Ok(Revised {
        context,
        rationale,
        warnings,
        incomparable_state,
    })
}
