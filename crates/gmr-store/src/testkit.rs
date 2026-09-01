use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;
use gmr_core::{
    AnchorKey, Binding, Claim, ContentHash, Entry, LinkKind, Ref, RunSettings, Seq,
    content_hash_of_bytes,
};

use chrono::{DateTime, Duration, Utc};

use crate::bindings::{Asserted, BindingRecord, BindingStore, Revocation};
use crate::error::StoreError;
use crate::journal::{Expected, Fence, Journal};
use crate::links::{LinkRecord, LinkRevocation, LinkStore};
use crate::queue::{Disposition, Queue, Ticket};
use crate::sealer::Sealer;

#[derive(Default)]
struct JournalInner {
    entries: Vec<(AnchorKey, Seq, Entry)>,
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
        _fence: Fence,
        expected: Expected,
    ) -> Result<Seq, StoreError> {
        let mut inner = self.inner.lock().unwrap();
        let head = inner
            .entries
            .iter()
            .filter(|(a, _, _)| a == anchor)
            .map(|(_, s, _)| *s)
            .next_back()
            .unwrap_or(0);
        crate::journal::guard(anchor, expected, head)?;
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

    async fn head(&self) -> Result<Seq, StoreError> {
        Ok(self.inner.lock().unwrap().next)
    }
}

#[derive(Default)]
struct BindingInner {
    bindings: Vec<BindingRecord>,
    revocations: Vec<Revocation>,
    sealed: HashMap<ContentHash, Vec<u8>>,
    links: Vec<(Ref, LinkRecord, bool)>,
}

#[derive(Default)]
pub struct MemoryBindings {
    inner: Mutex<BindingInner>,
}

impl MemoryBindings {
    fn live(&self, within: Option<&[AnchorKey]>) -> Vec<BindingRecord> {
        let inner = self.inner.lock().unwrap();
        let killed = |seq: Seq, anchor: &AnchorKey| {
            inner.revocations.iter().any(|rev| {
                within.is_none_or(|chain| chain.contains(&rev.at))
                    && rev
                        .tags
                        .iter()
                        .any(|t| t.binding == seq && &t.anchor == anchor)
            })
        };
        inner
            .bindings
            .iter()
            .filter_map(|r| {
                let anchors: Vec<AnchorKey> = r
                    .binding
                    .anchors
                    .iter()
                    .filter(|a| within.is_none_or(|chain| chain.contains(a)))
                    .filter(|a| !killed(r.seq, a))
                    .cloned()
                    .collect();
                if within.is_some() && anchors.is_empty() {
                    return None;
                }
                Some(BindingRecord {
                    binding: Binding {
                        claim: r.binding.claim.clone(),
                        anchors,
                        depends: r.binding.depends.clone(),
                    },
                    ..r.clone()
                })
            })
            .collect()
    }
}

#[async_trait]
impl BindingStore for MemoryBindings {
    async fn bind(&self, asserted: &Asserted) -> Result<(), StoreError> {
        let mut inner = self.inner.lock().unwrap();
        let seq = inner.bindings.len() as Seq + 1;
        inner.bindings.push(BindingRecord {
            seq,
            binding: asserted.binding.clone(),
            bound_version: asserted.bound_version.clone(),
            bound_at_seq: asserted.bound_at_seq,
            saw: asserted.saw.clone(),
            source: asserted.source,
            asserted_at: Some(asserted.at),
        });
        Ok(())
    }

    async fn revoke(&self, revocation: &Revocation) -> Result<(), StoreError> {
        self.inner
            .lock()
            .unwrap()
            .revocations
            .push(revocation.clone());
        Ok(())
    }

    async fn bindings_on(&self, anchors: &[AnchorKey]) -> Result<Vec<BindingRecord>, StoreError> {
        Ok(self.live(Some(anchors)))
    }

    async fn binding_of(&self, claim: &Claim) -> Result<Vec<BindingRecord>, StoreError> {
        Ok(self
            .live(None)
            .into_iter()
            .filter(|r| r.binding.claim.same(claim))
            .collect())
    }

    async fn all(&self) -> Result<Vec<BindingRecord>, StoreError> {
        Ok(self.live(None))
    }
}

#[async_trait]
impl Sealer for MemoryBindings {
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

#[async_trait]
impl LinkStore for MemoryBindings {
    async fn link(
        &self,
        from: &Ref,
        to: &Ref,
        kind: LinkKind,
        source: gmr_core::Source,
    ) -> Result<(), StoreError> {
        self.inner.lock().unwrap().links.push((
            from.clone(),
            LinkRecord {
                to: to.clone(),
                kind,
                source,
            },
            false,
        ));
        Ok(())
    }

    async fn unlink(&self, revocation: &LinkRevocation) -> Result<u64, StoreError> {
        let mut held = self.inner.lock().unwrap();
        let mut revoked = 0u64;
        for (from, record, dead) in held.links.iter_mut() {
            if *dead
                || from != &revocation.from
                || record.to != revocation.to
                || record.kind != revocation.kind
            {
                continue;
            }
            if revocation.asserted_as.is_some_and(|of| of != record.source) {
                continue;
            }
            *dead = true;
            revoked += 1;
        }
        Ok(revoked)
    }

    async fn links_of(&self, reference: &Ref) -> Result<Vec<LinkRecord>, StoreError> {
        Ok(self
            .inner
            .lock()
            .unwrap()
            .links
            .iter()
            .filter(|(from, _, dead)| from == reference && !dead)
            .map(|(_, record, _)| record.clone())
            .collect())
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
    settings: Mutex<HashMap<AnchorKey, RunSettings>>,
    sightings: Mutex<HashMap<AnchorKey, crate::Seen>>,
}

#[async_trait]
impl crate::Sightings for MemoryQueue {
    async fn sighted(&self, anchor: &AnchorKey, at: DateTime<Utc>) -> Result<(), StoreError> {
        let mut held = self.sightings.lock().unwrap();
        let seen = held.entry(anchor.clone()).or_default();
        seen.sightings += 1;
        seen.last_at = Some(at);
        Ok(())
    }

    async fn seen(&self, anchor: &AnchorKey) -> Result<crate::Seen, StoreError> {
        Ok(self
            .sightings
            .lock()
            .unwrap()
            .get(anchor)
            .copied()
            .unwrap_or_default())
    }

    async fn all_seen(
        &self,
    ) -> Result<std::collections::BTreeMap<AnchorKey, crate::Seen>, StoreError> {
        Ok(self
            .sightings
            .lock()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect())
    }
}

#[async_trait]
impl crate::Settings for MemoryQueue {
    async fn put(&self, anchor: &AnchorKey, settings: &RunSettings) -> Result<(), StoreError> {
        self.settings
            .lock()
            .unwrap()
            .insert(anchor.clone(), *settings);
        Ok(())
    }

    async fn get(&self, anchor: &AnchorKey) -> Result<Option<RunSettings>, StoreError> {
        Ok(self.settings.lock().unwrap().get(anchor).copied())
    }
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

    async fn ensure_enqueued(
        &self,
        anchor: &AnchorKey,
        due: DateTime<Utc>,
    ) -> Result<bool, StoreError> {
        let mut inner = self.inner.lock().unwrap();
        if inner.contains_key(anchor) {
            return Ok(false);
        }
        let slot = inner.entry(anchor.clone()).or_default();
        slot.due = due;
        slot.lease_until = None;
        slot.parked = false;
        Ok(true)
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
