use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use gmr_core::{AnchorKey, AnchorState, Entry, Seq, resume};
use gmr_store::{Expected, Fence, Journal};

use crate::error::RuntimeError;

const LOCK_POISONED: &str = "gmr-runtime: a prior fold panicked while holding the checkpoint";

#[derive(Clone)]
pub(crate) struct Stood {
    pub anchor: AnchorState,
    pub logged: u64,
}

pub struct AnchorLog {
    journal: Arc<dyn Journal>,
    checkpoint: Mutex<BTreeMap<AnchorKey, Stood>>,
}

impl AnchorLog {
    pub(crate) fn new(journal: Arc<dyn Journal>) -> Self {
        Self {
            journal,
            checkpoint: Mutex::new(BTreeMap::new()),
        }
    }

    pub async fn entries(
        &self,
        key: &AnchorKey,
        from: Seq,
    ) -> Result<Vec<(Seq, Entry)>, RuntimeError> {
        Ok(self.journal.entries(key, from).await?)
    }

    pub(crate) async fn stood(&self, key: &AnchorKey) -> Result<Option<Stood>, RuntimeError> {
        let held = self
            .checkpoint
            .lock()
            .expect(LOCK_POISONED)
            .get(key)
            .cloned();
        let from = held.as_ref().map_or(0, |h| h.anchor.head + 1);
        let entries = self.journal.entries(key, from).await?;

        let (seed, mut logged) = match held {
            Some(h) => (Some(h.anchor), h.logged),
            None => (None, 0),
        };
        let Some(anchor) = resume(seed, &entries, |_, entry, _| {
            if entry.is_sighting() {
                logged += 1;
            }
        }) else {
            return Ok(None);
        };

        let stood = Stood { anchor, logged };
        self.checkpoint
            .lock()
            .expect(LOCK_POISONED)
            .insert(key.clone(), stood.clone());
        Ok(Some(stood))
    }

    pub(crate) async fn state(&self, key: &AnchorKey) -> Result<Option<AnchorState>, RuntimeError> {
        Ok(self.stood(key).await?.map(|s| s.anchor))
    }

    pub async fn append(
        &self,
        key: &AnchorKey,
        entry: &Entry,
        fence: Fence,
        expected: Expected,
    ) -> Result<Seq, RuntimeError> {
        Ok(self.journal.append(key, entry, fence, expected).await?)
    }

    pub async fn anchors(&self) -> Result<Vec<AnchorKey>, RuntimeError> {
        Ok(self.journal.anchors().await?)
    }

    pub async fn head(&self) -> Result<Seq, RuntimeError> {
        Ok(self.journal.head().await?)
    }
}
