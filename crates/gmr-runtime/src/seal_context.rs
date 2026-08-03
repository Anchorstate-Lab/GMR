use gmr_core::AnchorState;
use serde_json::{Value, json};

/// What every author-driven write seals. `revise`/`close` extend it; the two
/// are not the same shape, so only the common part lives here.
pub(crate) fn base(s: &AnchorState) -> Value {
    json!({
        "at_entry": s.head,
        "state": s.state,
        "entered_at": s.entered_at,
    })
}
