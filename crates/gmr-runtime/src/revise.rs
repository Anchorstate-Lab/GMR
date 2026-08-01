use chrono::Utc;
use gmr_core::{AnchorKey, Change, ContentHash, Entry, fold};
use gmr_store::Fence;

use crate::assembly::Runtime;
use crate::error::RuntimeError;
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
        let entries = self.journal.entries(key, 0).await?;
        let s = fold(&entries).ok_or_else(|| RuntimeError::NoSuchAnchor { key: key.clone() })?;

        if s.closed {
            return Err(RuntimeError::AnchorClosed { key: key.clone() });
        }

        let context = serde_json::json!({
            "at_entry": s.head,
            "state": s.state,
            "entered_at": s.entered_at,
            "closed": s.closed,
            "latest": s.latest,
            "probe_declaration": s.anchor.probe.declaration_hash(),
            "evaluator_version": gmr_expr::EVALUATOR_VERSION,
        });
        let context = self
            .bindings
            .seal(&serde_json::to_vec(&context).expect("the context always serialises"))
            .await?;
        let rationale = self.bindings.seal(rationale).await?;

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

        self.journal
            .append(
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
}
