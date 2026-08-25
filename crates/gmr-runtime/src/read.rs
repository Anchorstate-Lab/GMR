use std::time::Duration;

use chrono::{DateTime, Utc};
use gmr_content::ContentErrorCode;
use gmr_core::{
    Anchor, AnchorKey, Derivation, Entry, Facts, FailureCode, Faltering, Link, Outcome, ProviderId,
    ReasonClass, Ref, Seq, Source, State, StatusId, Verifiability, Version, scan,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub faltering: Option<Faltering>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bound_version: Option<Version>,
    pub grounded: bool,
    pub links: Vec<Link>,
    pub bound_at_seq: Option<Seq>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline_at: Option<Seq>,
    pub sources: std::collections::BTreeSet<Source>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asserted_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warrant: Option<Warrant>,
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
    Unverified {
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
    Unverified,
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
            Self::Unverified { .. } => Footing::Unverified,
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
pub struct Warrant {
    pub holding: Holding,
    pub knowledge: Knowledge,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "holding", rename_all = "snake_case")]
pub enum Holding {
    Holds,
    Moved { axes: Vec<String>, at: Seq },
    Absent,
    NeverEstablished,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "knowledge", rename_all = "snake_case")]
pub enum Knowledge {
    Seen {
        at: DateTime<Utc>,
        verifiability: Verifiability,
    },
    Blind {
        since: Option<DateTime<Utc>>,
        why: Blind,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "blind", rename_all = "snake_case")]
pub enum Blind {
    NeverAsked,
    Unreachable { code: Option<FailureCode> },
    Unusable { code: Option<FailureCode> },
    Unevaluable { code: Option<FailureCode> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Bearing {
    Holds,
    Moved,
    Absent,
    NeverEstablished,
    Blind,
    NeverAsked,
}

impl Blind {
    fn of(f: &Faltering) -> Self {
        match (f.code, f.reason) {
            (Some(FailureCode::TimedOut), _) => Self::NeverAsked,
            (code, ReasonClass::Unreachable) => Self::Unreachable { code },
            (code, ReasonClass::Unusable) => Self::Unusable { code },
            (code, ReasonClass::Unevaluable) => Self::Unevaluable { code },
        }
    }
}

impl Warrant {
    pub fn bearing(&self) -> Bearing {
        match (&self.holding, &self.knowledge) {
            (Holding::Moved { .. }, _) => Bearing::Moved,
            (Holding::Absent, _) => Bearing::Absent,
            (Holding::NeverEstablished, _) => Bearing::NeverEstablished,
            (Holding::Holds, Knowledge::Seen { .. }) => Bearing::Holds,
            (
                Holding::Holds,
                Knowledge::Blind {
                    why: Blind::NeverAsked,
                    ..
                },
            ) => Bearing::NeverAsked,
            (Holding::Holds, Knowledge::Blind { .. }) => Bearing::Blind,
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
        let entries = self.log.entries(key, 0).await?;
        Ok(project(&entries, key, &self.scheduler.seen(key).await?)?.0)
    }

    pub async fn read_all(&self) -> Result<Vec<AnchorView>, RuntimeError> {
        let seen = self.scheduler.all_seen().await?;
        let mut out = Vec::new();
        for key in self.log.anchors().await? {
            let looks = seen.get(&key).copied().unwrap_or_default();
            let entries = self.log.entries(&key, 0).await?;
            out.push(project(&entries, &key, &looks)?.0);
        }
        Ok(out)
    }

    pub async fn grounded(&self, key: &AnchorKey) -> Result<Grounded, RuntimeError> {
        let policy = self.scheduler.policy();
        let entries = self.log.entries(key, 0).await?;
        let (view, moved_at) = project(&entries, key, &self.scheduler.seen(key).await?)?;
        ground(
            &self.log,
            &self.memory,
            view,
            &entries,
            moved_at,
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
        cobound(&self.log, &self.memory, reference).await
    }

    pub async fn grounded_all(&self) -> Result<Vec<Grounded>, RuntimeError> {
        let policy = self.scheduler.policy();
        let total = policy.content_budget();
        let call = policy.content_call();
        let seen = self.scheduler.all_seen().await?;
        let mut out = Vec::new();
        for key in self.log.anchors().await? {
            let looks = seen.get(&key).copied().unwrap_or_default();
            let entries = self.log.entries(&key, 0).await?;
            let (view, moved_at) = project(&entries, &key, &looks)?;
            out.push(
                ground(
                    &self.log,
                    &self.memory,
                    view,
                    &entries,
                    moved_at,
                    &total,
                    call,
                )
                .await?,
            );
        }
        Ok(out)
    }
}

fn state_at(entries: &[(Seq, Entry)], at: Seq) -> Option<State> {
    let mut out = None;
    scan(entries, |seq, _, now| {
        if seq <= at {
            out = Some(now.state.clone());
        }
    });
    out
}

fn differing(
    before: &serde_json::Value,
    now: &serde_json::Value,
    path: &str,
    out: &mut Vec<String>,
) {
    match (before, now) {
        (serde_json::Value::Object(a), serde_json::Value::Object(b)) => {
            let mut keys: Vec<&String> = a.keys().chain(b.keys()).collect();
            keys.sort();
            keys.dedup();
            for k in keys {
                if path.is_empty() && (k == gmr_core::POSITION || k == gmr_core::STATUS) {
                    continue;
                }
                let next = match path.is_empty() {
                    true => k.clone(),
                    false => format!("{path}.{k}"),
                };
                match (a.get(k), b.get(k)) {
                    (Some(x), Some(y)) => differing(x, y, &next, out),
                    _ => out.push(next),
                }
            }
        }
        _ if before != now => out.push(path.to_owned()),
        _ => {}
    }
}

fn axes_between(before: &State, now: &State) -> Vec<String> {
    let mut out = Vec::new();
    differing(before.as_value(), now.as_value(), "", &mut out);
    out
}

fn warranted(
    held: &MemoryView,
    view: &AnchorView,
    entries: &[(Seq, Entry)],
    moved_at: Option<Seq>,
) -> Warrant {
    let knowledge = match (&view.faltering, view.last_sighting) {
        (Some(f), since) => Knowledge::Blind {
            since,
            why: Blind::of(f),
        },
        (None, Some(at)) => Knowledge::Seen {
            at,
            verifiability: view
                .derivation
                .as_ref()
                .map(|d| d.verifiability)
                .unwrap_or(Verifiability::Open),
        },
        (None, None) => Knowledge::Blind {
            since: None,
            why: Blind::NeverAsked,
        },
    };

    let holding = match (held.bound_at_seq, moved_at) {
        _ if view.sighting == Sighting::Absent => Holding::Absent,
        (Some(bound), Some(moved)) if bound < moved => Holding::Moved {
            axes: state_at(entries, bound)
                .map(|before| axes_between(&before, &view.state))
                .unwrap_or_default(),
            at: moved,
        },
        (Some(_), Some(_)) => Holding::Holds,
        _ => Holding::NeverEstablished,
    };

    Warrant { holding, knowledge }
}

fn project(
    entries: &[(Seq, Entry)],
    key: &AnchorKey,
    looks: &Seen,
) -> Result<(AnchorView, Option<Seq>), RuntimeError> {
    let mut logged: u64 = 0;
    let s = scan(entries, |_, entry, _| {
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
            faltering: s.faltering.clone(),
            entered_at: s.entered_at,
            last_sighting,
            sightings,
            derivation,
            facts,
        },
        s.moved_at,
    ))
}

async fn ground(
    log: &AnchorLog,
    memory: &MemoryLens,
    view: AnchorView,
    entries: &[(Seq, Entry)],
    moved_at: Option<Seq>,
    total: &Budget,
    call: Duration,
) -> Result<Grounded, RuntimeError> {
    let mut memories = Vec::new();
    for asserted in memory.bindings_on(log, &view.key).await? {
        let mut held = memory.fetch_memory(asserted, &total.narrowed(call)).await?;
        held.warrant = Some(warranted(&held, &view, entries, moved_at));
        memories.push(held);
    }
    memory.carry_linked(&mut memories, total, call).await?;
    Ok(Grounded { view, memories })
}

async fn cobound(
    log: &AnchorLog,
    memory: &MemoryLens,
    reference: &Ref,
) -> Result<Vec<Ref>, RuntimeError> {
    let bound = memory.binding_of(reference).await?;
    let mut out: Vec<Ref> = Vec::new();
    for anchor in bound.anchors() {
        for other in memory.bindings_on(log, anchor).await? {
            let Some(other_reference) = other.standing().map(|r| r.binding.reference.clone())
            else {
                continue;
            };
            if &other_reference != reference && !out.contains(&other_reference) {
                out.push(other_reference);
            }
        }
    }
    out.sort();
    Ok(out)
}
