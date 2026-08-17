use std::time::Duration;

use chrono::{DateTime, Utc};
use gmr_content::ContentErrorCode;
use gmr_core::{
    Anchor, AnchorKey, Derivation, Facts, Link, Outcome, ProviderId, Ref, Seq, State, StatusId,
    Version, scan,
};
use gmr_probe::Budget;
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
    pub derivation: Option<Derivation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub facts: Option<Facts>,
    pub memories: Vec<MemoryView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemoryView {
    pub reference: Ref,
    pub bound_version: Version,
    pub grounded: bool,
    pub links: Vec<Link>,
    pub bound_at_seq: Option<Seq>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stale: Option<bool>,
    pub grounding: Grounding,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "grounding", rename_all = "snake_case")]
pub enum Grounding {
    Current {
        version: Version,
        #[serde(serialize_with = "as_text")]
        content: Vec<u8>,
    },
    Rewritten {
        version: Version,
        #[serde(serialize_with = "as_text")]
        content: Vec<u8>,
        before: Before,
    },
    Gone,
    NoProvider {
        provider: ProviderId,
    },
    Unreachable {
        code: ContentErrorCode,
        why: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "before", rename_all = "snake_case")]
pub enum Before {
    Retrieved {
        #[serde(serialize_with = "as_text")]
        content: Vec<u8>,
    },
    NotRetained,
    NoHistory,
    Unreachable {
        code: ContentErrorCode,
        why: String,
    },
}

impl MemoryView {
    pub fn content(&self) -> Option<&[u8]> {
        match &self.grounding {
            Grounding::Current { content, .. } | Grounding::Rewritten { content, .. } => {
                Some(content)
            }
            _ => None,
        }
    }

    pub fn rewritten(&self) -> bool {
        matches!(self.grounding, Grounding::Rewritten { .. })
    }
}

fn as_text<S: serde::Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&String::from_utf8_lossy(bytes))
}

impl Runtime {
    pub async fn read(&self, key: &AnchorKey) -> Result<AnchorView, RuntimeError> {
        let policy = self.scheduler.policy();
        read(
            &self.log,
            &self.memory,
            key,
            &policy.content_budget(),
            policy.content_call(),
        )
        .await
    }

    pub async fn current_version(&self, reference: &Ref) -> Result<Option<Version>, RuntimeError> {
        let policy = self.scheduler.policy();
        self.memory
            .current_version(
                reference,
                &policy.content_budget().narrowed(policy.content_call()),
            )
            .await
    }

    pub async fn cobound(&self, reference: &Ref) -> Result<Vec<Ref>, RuntimeError> {
        cobound(&self.memory, reference).await
    }

    pub async fn read_all(&self) -> Result<Vec<AnchorView>, RuntimeError> {
        let policy = self.scheduler.policy();
        read_all(
            &self.log,
            &self.memory,
            &policy.content_budget(),
            policy.content_call(),
        )
        .await
    }
}

async fn read(
    log: &AnchorLog,
    memory: &MemoryLens,
    key: &AnchorKey,
    total: &Budget,
    call: Duration,
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
        let mut view = memory.fetch_memory(binding, &total.narrowed(call)).await?;
        view.stale = view.bound_at_seq.map(|seq| seq < s.head);
        memories.push(view);
    }
    memory.carry_linked(&mut memories, total, call).await?;

    let sighting = match s.latest.as_ref().map(|o| &o.outcome) {
        Some(Outcome::Found { .. }) => Sighting::Found,
        _ => Sighting::Absent,
    };
    let derivation = s.latest.as_ref().map(|o| o.versions.derivation.clone());
    let facts = s.latest.as_ref().and_then(|o| o.facts().cloned());

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
        facts,
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

async fn read_all(
    log: &AnchorLog,
    memory: &MemoryLens,
    total: &Budget,
    call: Duration,
) -> Result<Vec<AnchorView>, RuntimeError> {
    let mut out = Vec::new();
    for key in log.anchors().await? {
        out.push(read(log, memory, &key, total, call).await?);
    }
    Ok(out)
}
