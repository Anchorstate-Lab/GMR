use gmr_core::AnchorState;
use serde_json::{Value, json};

pub(crate) fn base(s: &AnchorState) -> Value {
    json!({
        "at_entry": s.head,
        "state": s.state,
        "entered_at": s.entered_at,
    })
}
