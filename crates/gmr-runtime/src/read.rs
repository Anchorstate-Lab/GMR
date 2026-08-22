use std::time::Duration;

use chrono::{DateTime, Utc};
use gmr_content::ContentErrorCode;
use gmr_core::{
    Anchor, AnchorKey, Derivation, Facts, Link, Outcome, ProviderId, Ref, Seq, Source, State,
    StatusId, Version, scan,
};
use gmr_probe::Budget;
use gmr_store::Seen;
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
}

#[derive(Debug, Clone, Serialize)]
pub struct Grounded {
    #[serde(flatten)]
    pub view: AnchorView,
    pub memories: Vec<MemoryView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemoryView {
    pub reference: Ref,
    pub bound_version: Version,
    pub grounded: bool,
    pub links: Vec<Link>,
    pub bound_at_seq: Option<Seq>,
    pub source: Source,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asserted_at: Option<DateTime<Utc>>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Footing {
    Current,
    Rewritten,
    NoBefore,
    Gone,
    NoProvider,
    Unreachable,
    NeverAsked,
}

impl Footing {
    pub fn is_current(self) -> bool {
        matches!(self, Self::Current)
    }
}

impl Grounding {
    pub fn footing(&self) -> Footing {
        match self {
            Self::Current { .. } => Footing::Current,
            Self::Rewritten { before, .. } => match before {
                Before::Retrieved { .. } => Footing::Rewritten,
                _ => Footing::NoBefore,
            },
            Self::Gone => Footing::Gone,
            Self::NoProvider { .. } => Footing::NoProvider,
            Self::Unreachable { code, .. } => match code {
                ContentErrorCode::BudgetSpent => Footing::NeverAsked,
                _ => Footing::Unreachable,
            },
        }
    }
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

    pub fn footing(&self) -> Footing {
        self.grounding.footing()
    }
}

fn as_text<S: serde::Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
    match std::str::from_utf8(bytes) {
        Ok(text) => s.serialize_some(text),
        Err(_) => s.serialize_none(),
    }
}

impl Runtime {
    pub async fn read(&self, key: &AnchorKey) -> Result<AnchorView, RuntimeError> {
        Ok(projected(&self.log, key, &self.scheduler.seen(key).await?)
            .await?
            .0)
    }

    pub async fn read_all(&self) -> Result<Vec<AnchorView>, RuntimeError> {
        let seen = self.scheduler.all_seen().await?;
        let mut out = Vec::new();
        for key in self.log.anchors().await? {
            let looks = seen.get(&key).copied().unwrap_or_default();
            out.push(projected(&self.log, &key, &looks).await?.0);
        }
        Ok(out)
    }

    pub async fn grounded(&self, key: &AnchorKey) -> Result<Grounded, RuntimeError> {
        let policy = self.scheduler.policy();
        let (view, head) = projected(&self.log, key, &self.scheduler.seen(key).await?).await?;
        ground(
            &self.memory,
            view,
            head,
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

    pub async fn grounded_all(&self) -> Result<Vec<Grounded>, RuntimeError> {
        let policy = self.scheduler.policy();
        let total = policy.content_budget();
        let call = policy.content_call();
        let seen = self.scheduler.all_seen().await?;
        let mut out = Vec::new();
        for key in self.log.anchors().await? {
            let looks = seen.get(&key).copied().unwrap_or_default();
            let (view, head) = projected(&self.log, &key, &looks).await?;
            out.push(ground(&self.memory, view, head, &total, call).await?);
        }
        Ok(out)
    }
}

async fn projected(
    log: &AnchorLog,
    key: &AnchorKey,
    looks: &Seen,
) -> Result<(AnchorView, Seq), RuntimeError> {
    let entries = log.entries(key, 0).await?;
    let mut logged: u64 = 0;
    let s = scan(&entries, |_, entry, _| {
        if entry.is_sighting() {
            logged += 1;
        }
    })
    .ok_or_else(|| RuntimeError::NoSuchAnchor { key: key.clone() })?;

    let (sightings, last_sighting) = match looks.sightings {
        0 => (logged, s.last_sighting),
        counted => (counted, looks.last_at.or(s.last_sighting)),
    };

    let sighting = match s.latest.as_ref().map(|o| &o.outcome) {
        Some(Outcome::Found { .. }) => Sighting::Found,
        _ => Sighting::Absent,
    };
    let derivation = s.latest.as_ref().map(|o| o.versions.derivation.clone());
    let facts = s.latest.as_ref().and_then(|o| o.facts().cloned());

    Ok((
        AnchorView {
            key: key.clone(),
            status: s.state.status(),
            state: s.state,
            anchor: s.anchor,
            sighting,
            closed: s.closed,
            attempts: s.attempts,
            entered_at: s.entered_at,
            last_sighting,
            sightings,
            derivation,
            facts,
        },
        s.head,
    ))
}

async fn ground(
    memory: &MemoryLens,
    view: AnchorView,
    head: Seq,
    total: &Budget,
    call: Duration,
) -> Result<Grounded, RuntimeError> {
    let mut memories = Vec::new();
    for binding in memory.bindings_on(&view.key).await? {
        let mut held = memory.fetch_memory(binding, &total.narrowed(call)).await?;
        held.stale = held.bound_at_seq.map(|seq| seq < head);
        memories.push(held);
    }
    memory.carry_linked(&mut memories, total, call).await?;
    Ok(Grounded { view, memories })
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
