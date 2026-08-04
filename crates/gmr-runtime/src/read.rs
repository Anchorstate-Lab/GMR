use chrono::{DateTime, Utc};
use gmr_core::{
    Anchor, AnchorKey, Derivation, Link, Outcome, Ref, Seq, State, StatusId, Version, scan,
};
use serde::Serialize;

use crate::assembly::Runtime;
use crate::error::RuntimeError;
use crate::log::AnchorLog;
use crate::memory::MemoryLens;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Sighting {
    Found,
    Absent,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnchorView {
    pub key: AnchorKey,
    pub anchor: Anchor,
    pub state: State,
    pub status: Option<StatusId>,
    pub sighting: Sighting,
    pub closed: bool,
    pub attempts: u32,
    pub entered_at: Option<DateTime<Utc>>,
    pub last_sighting: Option<DateTime<Utc>>,
    pub sightings: u64,
    /// What derived the reading this state stands on. Compare it against what
    /// the probe resolves to now and you know whether the instrument changed.
    pub derivation: Option<Derivation>,
    pub memories: Vec<MemoryView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemoryView {
    pub reference: Ref,
    pub bound_version: Version,
    pub current_version: Option<Version>,
    pub rewritten: bool,
    pub content: Option<String>,
    pub content_at_bind: Option<String>,
    pub retrievable: Option<bool>,
    pub grounded: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unavailable: Option<String>,
    pub links: Vec<Link>,
    /// The bound anchor's head at bind time; `None` unless exactly one anchor.
    pub bound_at_seq: Option<Seq>,
    /// Whether the anchor moved since `bound_at_seq`. `None` when there is
    /// nothing to compare against, including records carried in via a link.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stale: Option<bool>,
}

impl Runtime {
    pub async fn read(&self, key: &AnchorKey) -> Result<AnchorView, RuntimeError> {
        read(&self.log, &self.memory, key).await
    }

    pub async fn cobound(&self, reference: &Ref) -> Result<Vec<Ref>, RuntimeError> {
        cobound(&self.memory, reference).await
    }

    pub async fn read_all(&self) -> Result<Vec<AnchorView>, RuntimeError> {
        read_all(&self.log, &self.memory).await
    }
}

async fn read(
    log: &AnchorLog,
    memory: &MemoryLens,
    key: &AnchorKey,
) -> Result<AnchorView, RuntimeError> {
    let entries = log.entries(key, 0).await?;
    let mut sightings: u64 = 0;
    let s = scan(&entries, |_, entry, _| {
        if entry.is_sighting() {
            sightings += 1;
        }
    })
    .ok_or_else(|| RuntimeError::NoSuchAnchor { key: key.clone() })?;

    let mut memories = Vec::new();
    for binding in memory.bindings_on(key).await? {
        let mut view = memory.fetch_memory(binding).await?;
        // Relative to this anchor's head; linked records are left None below.
        view.stale = view.bound_at_seq.map(|seq| seq < s.head);
        memories.push(view);
    }
    memory.carry_linked(&mut memories).await?;

    let sighting = match s.latest.as_ref().map(|o| &o.outcome) {
        Some(Outcome::Found { .. }) => Sighting::Found,
        _ => Sighting::Absent,
    };
    let derivation = s.latest.as_ref().map(|o| o.versions.derivation.clone());

    Ok(AnchorView {
        key: key.clone(),
        status: s.state.status(),
        state: s.state,
        anchor: s.anchor,
        sighting,
        closed: s.closed,
        attempts: s.attempts,
        entered_at: s.entered_at,
        last_sighting: s.last_sighting,
        sightings,
        derivation,
        memories,
    })
}

async fn cobound(memory: &MemoryLens, reference: &Ref) -> Result<Vec<Ref>, RuntimeError> {
    let Some(record) = memory.binding_of(reference).await? else {
        return Ok(Vec::new());
    };
    let mut out: Vec<Ref> = Vec::new();
    for anchor in &record.binding.anchors {
        for other in memory.bindings_on(anchor).await? {
            let other_reference = other.binding.reference;
            if &other_reference != reference && !out.contains(&other_reference) {
                out.push(other_reference);
            }
        }
    }
    out.sort();
    Ok(out)
}

async fn read_all(log: &AnchorLog, memory: &MemoryLens) -> Result<Vec<AnchorView>, RuntimeError> {
    let mut out = Vec::new();
    for key in log.anchors().await? {
        out.push(read(log, memory, &key).await?);
    }
    Ok(out)
}
