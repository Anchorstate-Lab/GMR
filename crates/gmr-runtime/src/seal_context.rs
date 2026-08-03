use gmr_core::AnchorState;
use serde_json::{Value, json};

/// The part of a sealed context every author-driven write shares: where the
/// log stood, and what the state was at that point. `revise`/`close` each add
/// their own fields on top — the two contexts are not the same shape (revise
/// also records the probe/evaluator identity in force; close records who
/// closed it), so this only factors out what is genuinely common, rather than
/// forcing both into one type.
pub(crate) fn base(s: &AnchorState) -> Value {
    json!({
        "at_entry": s.head,
        "state": s.state,
        "entered_at": s.entered_at,
    })
}
