use chrono::Utc;
use gmr_core::{AnchorKey, Entry, fold};
use gmr_store::Fence;

use crate::assembly::Runtime;
use crate::error::RuntimeError;

impl Runtime {
    pub async fn close(&self, key: &AnchorKey, rationale: &[u8]) -> Result<(), RuntimeError> {
        let entries = self.journal.entries(key, 0).await?;
        let state =
            fold(&entries).ok_or_else(|| RuntimeError::NoSuchAnchor { key: key.clone() })?;

        if state.closed {
            return Err(RuntimeError::AnchorClosed { key: key.clone() });
        }

        let context = serde_json::json!({
            "closed_by": "author",
            "at_entry": state.head,
            "state": state.state,
            "entered_at": state.entered_at,
        });
        let context = self
            .bindings
            .seal(&serde_json::to_vec(&context).expect("context 一定可序列化"))
            .await?;
        let rationale = self.bindings.seal(rationale).await?;

        self.journal
            .append(
                key,
                &Entry::Close {
                    context,
                    rationale,
                    at: Utc::now(),
                },
                Fence::NONE,
            )
            .await?;
        Ok(())
    }
}
