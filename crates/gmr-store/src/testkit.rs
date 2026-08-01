use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;
use gmr_core::{AnchorKey, Binding, ContentHash, Entry, Ref, Seq, content_hash_of_bytes};

use chrono::{DateTime, Duration, Utc};

use crate::bindings::BindingStore;
use crate::error::StoreError;
use crate::journal::{Fence, Journal};
use crate::queue::{Disposition, Queue, Ticket};

#[derive(Default)]
struct JournalInner {
    entries: Vec<(AnchorKey, Seq, Entry)>,
    fences: HashMap<AnchorKey, u64>,
    next: Seq,
}

#[derive(Default)]
pub struct MemoryJournal {
    inner: Mutex<JournalInner>,
}

#[async_trait]
impl Journal for MemoryJournal {
    async fn append(
        &self,
        anchor: &AnchorKey,
        entry: &Entry,
        fence: Fence,
    ) -> Result<Seq, StoreError> {
        let mut inner = self.inner.lock().unwrap();
        let seen = inner.fences.get(anchor).copied().unwrap_or(0);
        crate::journal::guard(fence, seen as i64, entry)?;
        inner
            .fences
            .insert(anchor.clone(), fence.epoch().unwrap_or(0).max(seen));
        inner.next += 1;
        let seq = inner.next;
        inner.entries.push((anchor.clone(), seq, entry.clone()));
        Ok(seq)
    }

    async fn entries(
        &self,
        anchor: &AnchorKey,
        from: Seq,
    ) -> Result<Vec<(Seq, Entry)>, StoreError> {
        let inner = self.inner.lock().unwrap();
        Ok(inner
            .entries
            .iter()
            .filter(|(a, s, _)| a == anchor && *s >= from)
            .map(|(_, s, e)| (*s, e.clone()))
            .collect())
    }

    async fn anchors(&self) -> Result<Vec<AnchorKey>, StoreError> {
        let inner = self.inner.lock().unwrap();
        let mut keys: Vec<AnchorKey> = inner.entries.iter().map(|(a, _, _)| a.clone()).collect();
        keys.sort();
        keys.dedup();
        Ok(keys)
    }
}

#[derive(Default)]
struct BindingInner {
    bindings: Vec<Binding>,
    sealed: HashMap<ContentHash, Vec<u8>>,
}

#[derive(Default)]
pub struct MemoryBindings {
    inner: Mutex<BindingInner>,
}

impl MemoryBindings {
    fn latest(&self) -> Vec<Binding> {
        let inner = self.inner.lock().unwrap();
        let mut seen: Vec<&Ref> = Vec::new();
        let mut out: Vec<Binding> = Vec::new();
        for b in inner.bindings.iter().rev() {
            if !seen.contains(&&b.reference) {
                seen.push(&b.reference);
                out.push(b.clone());
            }
        }
        out.reverse();
        out
    }
}

#[async_trait]
impl BindingStore for MemoryBindings {
    async fn bind(&self, binding: &Binding) -> Result<(), StoreError> {
        self.inner.lock().unwrap().bindings.push(binding.clone());
        Ok(())
    }

    async fn bindings_on(&self, anchor: &AnchorKey) -> Result<Vec<Binding>, StoreError> {
        Ok(self
            .latest()
            .into_iter()
            .filter(|b| b.anchors.contains(anchor))
            .collect())
    }

    async fn binding_of(&self, reference: &Ref) -> Result<Option<Binding>, StoreError> {
        let inner = self.inner.lock().unwrap();
        Ok(inner
            .bindings
            .iter()
            .rev()
            .find(|b| &b.reference == reference)
            .cloned())
    }

    async fn all(&self) -> Result<Vec<Binding>, StoreError> {
        Ok(self.latest())
    }

    async fn seal(&self, bytes: &[u8]) -> Result<ContentHash, StoreError> {
        let address = content_hash_of_bytes(bytes);
        self.inner
            .lock()
            .unwrap()
            .sealed
            .insert(address.clone(), bytes.to_vec());
        Ok(address)
    }

    async fn sealed(&self, address: &ContentHash) -> Result<Option<Vec<u8>>, StoreError> {
        Ok(self.inner.lock().unwrap().sealed.get(address).cloned())
    }
}

#[derive(Default)]
struct Slot {
    due: DateTime<Utc>,
    lease_until: Option<DateTime<Utc>>,
    epoch: u64,
    parked: bool,
}

#[derive(Default)]
pub struct MemoryQueue {
    inner: Mutex<HashMap<AnchorKey, Slot>>,
}

#[async_trait]
impl Queue for MemoryQueue {
    async fn enqueue(&self, anchor: &AnchorKey, due: DateTime<Utc>) -> Result<(), StoreError> {
        let mut inner = self.inner.lock().unwrap();
        let slot = inner.entry(anchor.clone()).or_default();
        slot.due = due;
        slot.lease_until = None;
        slot.parked = false;
        Ok(())
    }

    async fn due(
        &self,
        now: DateTime<Utc>,
        lease: Duration,
        limit: usize,
    ) -> Result<Vec<Ticket>, StoreError> {
        let mut inner = self.inner.lock().unwrap();
        let mut out = Vec::new();
        let mut keys: Vec<AnchorKey> = inner.keys().cloned().collect();
        keys.sort();
        for key in keys {
            if out.len() >= limit {
                break;
            }
            let slot = inner.get_mut(&key).unwrap();
            let leased = slot.lease_until.is_some_and(|u| u > now);
            if !slot.parked && slot.due <= now && !leased {
                slot.epoch += 1;
                slot.lease_until = Some(now + lease);
                out.push(Ticket {
                    anchor: key,
                    fence: Fence::Held(slot.epoch),
                    lease_until: now + lease,
                });
            }
        }
        Ok(out)
    }

    async fn lease(
        &self,
        anchor: &AnchorKey,
        now: DateTime<Utc>,
        lease: Duration,
    ) -> Result<Option<Ticket>, StoreError> {
        let mut inner = self.inner.lock().unwrap();
        let Some(slot) = inner.get_mut(anchor) else {
            return Ok(None);
        };
        if slot.lease_until.is_some_and(|u| u > now) {
            return Ok(None);
        }
        slot.epoch += 1;
        slot.lease_until = Some(now + lease);
        Ok(Some(Ticket {
            anchor: anchor.clone(),
            fence: Fence::Held(slot.epoch),
            lease_until: now + lease,
        }))
    }

    async fn settle(
        &self,
        ticket: &Ticket,
        disposition: Disposition,
        now: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        let mut inner = self.inner.lock().unwrap();
        match disposition {
            Disposition::Retire => {
                if let Some(slot) = inner.get_mut(&ticket.anchor) {
                    slot.parked = true;
                    slot.lease_until = None;
                }
            }
            Disposition::Reschedule { after_secs } | Disposition::Backoff { after_secs } => {
                if let Some(slot) = inner.get_mut(&ticket.anchor) {
                    slot.due = now + Duration::seconds(after_secs);
                    slot.lease_until = None;
                }
            }
        }
        Ok(())
    }
}
