use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use chrono::{DateTime, Utc};
use futures_util::{StreamExt, TryStreamExt, future::try_join};
use gmr_budget::Budget;
use gmr_content::ContentErrorCode;
use gmr_core::{
    Anchor, AnchorKey, AnchorState, Derivation, Entry, FactAddress, Facts, FailureCode, Faltering,
    Link, Outcome, ProbeVersion, ProviderId, ReasonClass, Ref, Seq, Source, State, StatusId,
    Verifiability, Version,
};
use gmr_store::Seen;
use serde::Serialize;

use crate::assembly::Runtime;
use crate::error::RuntimeError;
use crate::log::AnchorLog;
use crate::memory::MemoryLens;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Instructions {
    pub max_staleness: Option<Duration>,
    pub budget: Option<Duration>,
}

impl Instructions {
    pub fn fresher_than(span: Duration) -> Self {
        Self {
            max_staleness: Some(span),
            ..Self::default()
        }
    }

    fn stale(&self, last_sighting: Option<DateTime<Utc>>, now: DateTime<Utc>) -> bool {
        let Some(max) = self.max_staleness else {
            return false;
        };
        match last_sighting {
            None => true,
            Some(at) => now
                .signed_duration_since(at)
                .to_std()
                .is_ok_and(|since| since > max),
        }
    }
}

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
    pub fact_address: Option<FactAddress>,
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
pub struct Evidence {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reading: Option<FactAddress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instrument: Option<ProbeVersion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bound_at: Option<Seq>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub moved_at: Option<Seq>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "anchored", rename_all = "snake_case")]
pub enum Anchored {
    On {
        key: AnchorKey,
        warrant: Warrant,
        evidence: Evidence,
    },
    Unopened {
        key: AnchorKey,
    },
}

impl Anchored {
    pub fn key(&self) -> &AnchorKey {
        match self {
            Self::On { key, .. } | Self::Unopened { key } => key,
        }
    }

    pub fn warrant(&self) -> Option<&Warrant> {
        match self {
            Self::On { warrant, .. } => Some(warrant),
            Self::Unopened { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Standing {
    pub reference: Ref,
    pub record: Grounding,
    pub on: Vec<Anchored>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "holding", rename_all = "snake_case")]
pub enum Holding {
    Holds,
    Moved {
        axes: Vec<String>,
        at: Seq,
    },
    Incomparable {
        took: ProbeVersion,
        reads: ProbeVersion,
    },
    Absent,
    NeverEstablished,
    Undated,
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
pub enum HoldingKind {
    Holds,
    Moved,
    Incomparable,
    Absent,
    NeverEstablished,
    Undated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeKind {
    Seen,
    NeverAsked,
    Unreachable,
    Unusable,
    Unevaluable,
}

impl Holding {
    pub fn kind(&self) -> HoldingKind {
        match self {
            Self::Holds => HoldingKind::Holds,
            Self::Moved { .. } => HoldingKind::Moved,
            Self::Incomparable { .. } => HoldingKind::Incomparable,
            Self::Absent => HoldingKind::Absent,
            Self::NeverEstablished => HoldingKind::NeverEstablished,
            Self::Undated => HoldingKind::Undated,
        }
    }
}

impl Knowledge {
    pub fn kind(&self) -> KnowledgeKind {
        match self {
            Self::Seen { .. } => KnowledgeKind::Seen,
            Self::Blind { why, .. } => match why {
                Blind::NeverAsked => KnowledgeKind::NeverAsked,
                Blind::Unreachable { .. } => KnowledgeKind::Unreachable,
                Blind::Unusable { .. } => KnowledgeKind::Unusable,
                Blind::Unevaluable { .. } => KnowledgeKind::Unevaluable,
            },
        }
    }
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
        Ok(stand(&self.log, key, &self.scheduler.seen(key).await?)
            .await?
            .0)
    }

    pub async fn read_all(&self) -> Result<Vec<AnchorView>, RuntimeError> {
        let seen = self.scheduler.all_seen().await?;
        let mut out = Vec::new();
        for key in self.log.anchors().await? {
            let looks = seen.get(&key).copied().unwrap_or_default();
            out.push(stand(&self.log, &key, &looks).await?.0);
        }
        Ok(out)
    }

    pub async fn grounded(&self, key: &AnchorKey) -> Result<Grounded, RuntimeError> {
        self.grounded_within(key, &Instructions::default()).await
    }

    pub async fn grounded_within(
        &self,
        key: &AnchorKey,
        how: &Instructions,
    ) -> Result<Grounded, RuntimeError> {
        let policy = self.scheduler.policy();
        self.refresh(key, how).await?;
        let (view, moved_at) = stand(&self.log, key, &self.scheduler.seen(key).await?).await?;
        ground(
            &self.log,
            &self.memory,
            view,
            moved_at,
            &reaching(policy, how),
            policy.content_call(),
        )
        .await
    }

    async fn refresh(&self, key: &AnchorKey, how: &Instructions) -> Result<(), RuntimeError> {
        if how.max_staleness.is_none() {
            return Ok(());
        }
        let looks = self.scheduler.seen(key).await?;
        let (view, _) = stand(&self.log, key, &looks).await?;
        if view.closed || !how.stale(view.last_sighting, Utc::now()) {
            return Ok(());
        }
        self.observe_within(key, how).await?;
        Ok(())
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

    pub async fn ground(
        &self,
        refs: &[Ref],
        how: &Instructions,
    ) -> Result<Vec<Standing>, RuntimeError> {
        let policy = self.scheduler.policy();

        let mut bound = Vec::with_capacity(refs.len());
        for reference in refs {
            bound.push(self.memory.binding_of(reference).await?);
        }
        let keys: Vec<AnchorKey> = bound
            .iter()
            .flat_map(|b| b.anchors().iter().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();

        let call = Budget::within(span_of(policy, how), usize::MAX);
        let probing = call.narrowed_to(
            Duration::from_millis(policy.probe_budget_ms),
            policy.probe_output_cap,
        );
        let reading = call.narrowed_to(Duration::from_millis(policy.content_total_ms), usize::MAX);

        let (stood, records) = try_join(
            self.stood_all(&keys, how, &probing),
            self.records_of(refs, &bound, &reading, policy.content_call()),
        )
        .await?;

        let mut out = Vec::with_capacity(refs.len());
        for ((reference, held), record) in refs.iter().zip(&bound).zip(records) {
            let mut on = Vec::with_capacity(held.anchors().len());
            for key in held.anchors() {
                on.push(anchored(&self.log, key, held, stood.get(key)).await?);
            }
            out.push(Standing {
                reference: reference.clone(),
                record,
                on,
            });
        }
        Ok(out)
    }

    async fn stood_all(
        &self,
        keys: &[AnchorKey],
        how: &Instructions,
        budget: &Budget,
    ) -> Result<BTreeMap<AnchorKey, (AnchorView, Option<Seq>)>, RuntimeError> {
        let at_once = self.scheduler.policy().observe_at_once.max(1);
        let seen = self.scheduler.all_seen().await?;
        let now = Utc::now();

        let each = futures_util::stream::iter(keys)
            .map(|key| {
                let looks = seen.get(key).copied().unwrap_or_default();
                async move {
                    let Some((view, moved_at)) = standing_at(&self.log, key, &looks).await? else {
                        return Ok::<_, RuntimeError>(None);
                    };
                    if view.closed || !how.stale(view.last_sighting, now) {
                        return Ok(Some((view, moved_at)));
                    }
                    match crate::observe::observe(
                        &self.log,
                        &self.observer,
                        &self.scheduler,
                        key,
                        budget,
                    )
                    .await
                    {
                        Ok(_) | Err(RuntimeError::Leased { .. }) => {}
                        Err(e) => return Err(e),
                    }
                    standing_at(&self.log, key, &self.scheduler.seen(key).await?).await
                }
            })
            .buffered(at_once)
            .try_collect::<Vec<_>>()
            .await?;

        Ok(keys
            .iter()
            .cloned()
            .zip(each)
            .filter_map(|(key, view)| view.map(|v| (key, v)))
            .collect())
    }

    async fn records_of(
        &self,
        refs: &[Ref],
        bound: &[crate::memory::Bound],
        total: &Budget,
        call: Duration,
    ) -> Result<Vec<Grounding>, RuntimeError> {
        let mut out = Vec::with_capacity(refs.len());
        for (reference, held) in refs.iter().zip(bound) {
            out.push(
                self.memory
                    .grounding_of(reference, held.bound_version(), &total.narrowed(call))
                    .await,
            );
        }
        Ok(out)
    }

    pub async fn cobound(&self, reference: &Ref) -> Result<Vec<Ref>, RuntimeError> {
        cobound(&self.log, &self.memory, reference).await
    }

    pub async fn grounded_all(&self) -> Result<Vec<Grounded>, RuntimeError> {
        self.grounded_all_within(&Instructions::default()).await
    }

    pub async fn grounded_all_within(
        &self,
        how: &Instructions,
    ) -> Result<Vec<Grounded>, RuntimeError> {
        let policy = self.scheduler.policy();
        let total = reaching(policy, how);
        let call = policy.content_call();
        let keys = self.log.anchors().await?;
        for key in &keys {
            self.refresh(key, how).await?;
        }
        let seen = self.scheduler.all_seen().await?;
        let mut out = Vec::new();
        for key in keys {
            let looks = seen.get(&key).copied().unwrap_or_default();
            let (view, moved_at) = stand(&self.log, &key, &looks).await?;
            out.push(ground(&self.log, &self.memory, view, moved_at, &total, call).await?);
        }
        Ok(out)
    }
}

async fn anchored(
    log: &AnchorLog,
    key: &AnchorKey,
    held: &crate::memory::Bound,
    stood: Option<&(AnchorView, Option<Seq>)>,
) -> Result<Anchored, RuntimeError> {
    let Some((view, moved_at)) = stood else {
        return Ok(Anchored::Unopened { key: key.clone() });
    };
    let bound_at = held.dating().and_then(|r| r.bound_at_seq);
    Ok(Anchored::On {
        key: key.clone(),
        warrant: warranted(log, key, bound_at, view, *moved_at).await?,
        evidence: Evidence {
            reading: view.fact_address.clone(),
            instrument: view.derivation.as_ref().map(|d| d.version.clone()),
            bound_at,
            moved_at: *moved_at,
        },
    })
}

fn span_of(policy: &crate::Policy, how: &Instructions) -> Duration {
    how.budget.unwrap_or_else(|| {
        Duration::from_millis(policy.probe_budget_ms.max(policy.content_total_ms))
    })
}

fn reaching(policy: &crate::Policy, how: &Instructions) -> Budget {
    match how.budget {
        Some(span) => policy.content_budget().narrowed(span),
        None => policy.content_budget(),
    }
}

fn folded_at(entries: &[(Seq, Entry)], at: Seq) -> Option<AnchorState> {
    let upto = entries.partition_point(|(seq, _)| *seq <= at);
    gmr_core::fold(&entries[..upto])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Divergence {
    Added,
    Removed,
    Differing,
}

fn differing(
    before: &serde_json::Value,
    now: &serde_json::Value,
    path: &str,
    out: &mut Vec<(String, Divergence)>,
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
                    (None, Some(_)) => out.push((next, Divergence::Added)),
                    (Some(_), None) => out.push((next, Divergence::Removed)),
                    (None, None) => {}
                }
            }
        }
        _ if before != now => out.push((path.to_owned(), Divergence::Differing)),
        _ => {}
    }
}

fn axes_between(before: &State, now: &State) -> Vec<(String, Divergence)> {
    let mut out = Vec::new();
    differing(before.as_value(), now.as_value(), "", &mut out);
    out
}

fn measured_by_both(axes: &[(String, Divergence)]) -> bool {
    axes.iter().any(|(_, d)| *d != Divergence::Added)
}

fn named(axes: Vec<(String, Divergence)>) -> Vec<String> {
    axes.into_iter().map(|(path, _)| path).collect()
}

async fn warranted(
    log: &AnchorLog,
    key: &AnchorKey,
    bound_at_seq: Option<Seq>,
    view: &AnchorView,
    moved_at: Option<Seq>,
) -> Result<Warrant, RuntimeError> {
    Ok(Warrant {
        holding: holding(log, key, bound_at_seq, view, moved_at).await?,
        knowledge: knowledge_of(view),
    })
}

pub(crate) fn knowledge_of(view: &AnchorView) -> Knowledge {
    match (
        &view.faltering,
        view.last_sighting,
        view.derivation.as_ref(),
    ) {
        (Some(f), since, _) => Knowledge::Blind {
            since,
            why: Blind::of(f),
        },
        (None, Some(at), Some(d)) => Knowledge::Seen {
            at,
            verifiability: d.verifiability.clone(),
        },
        (None, since, _) => Knowledge::Blind {
            since,
            why: Blind::NeverAsked,
        },
    }
}

async fn holding(
    log: &AnchorLog,
    key: &AnchorKey,
    bound_at_seq: Option<Seq>,
    view: &AnchorView,
    moved_at: Option<Seq>,
) -> Result<Holding, RuntimeError> {
    if view.sighting == Sighting::Absent {
        return Ok(Holding::Absent);
    }
    let (Some(bound), Some(moved)) = (bound_at_seq, moved_at) else {
        return Ok(Holding::Undated);
    };
    if bound >= moved {
        return Ok(Holding::Holds);
    }
    Ok(folded(&log.entries(key, 0).await?, bound, view, moved))
}

fn folded(entries: &[(Seq, Entry)], bound: Seq, view: &AnchorView, moved: Seq) -> Holding {
    let Some(before) = folded_at(entries, bound) else {
        return Holding::NeverEstablished;
    };
    let axes = axes_between(&before.state, &view.state);
    if axes.is_empty() {
        return Holding::Holds;
    }
    let took = before
        .latest
        .as_ref()
        .map(|o| &o.versions.derivation.version);
    let reads = view.derivation.as_ref().map(|d| &d.version);
    match (took, reads) {
        (Some(took), Some(reads)) if took != reads => match measured_by_both(&axes) {
            true => Holding::Incomparable {
                took: took.clone(),
                reads: reads.clone(),
            },
            false => Holding::Holds,
        },
        _ => Holding::Moved {
            axes: named(axes),
            at: moved,
        },
    }
}

pub(crate) async fn stand(
    log: &AnchorLog,
    key: &AnchorKey,
    looks: &Seen,
) -> Result<(AnchorView, Option<Seq>), RuntimeError> {
    standing_at(log, key, looks)
        .await?
        .ok_or_else(|| RuntimeError::NoSuchAnchor { key: key.clone() })
}

pub(crate) async fn standing_at(
    log: &AnchorLog,
    key: &AnchorKey,
    looks: &Seen,
) -> Result<Option<(AnchorView, Option<Seq>)>, RuntimeError> {
    Ok(log
        .stood(key)
        .await?
        .map(|stood| viewed(stood.anchor, key, looks, stood.logged)))
}

pub(crate) fn viewed(
    s: AnchorState,
    key: &AnchorKey,
    looks: &Seen,
    logged: u64,
) -> (AnchorView, Option<Seq>) {
    let (sightings, last_sighting) = match looks.sightings {
        0 => (logged, s.last_sighting),
        counted => (counted, looks.last_at.or(s.last_sighting)),
    };

    let sighting = match s.latest.as_ref().map(|o| &o.outcome) {
        Some(Outcome::Found { .. }) => Sighting::Found,
        _ => Sighting::Absent,
    };
    let derivation = s.latest.as_ref().map(|o| o.versions.derivation.clone());
    let fact_address = s.latest.as_ref().map(|o| o.fact_address.clone());
    let facts = s.latest.as_ref().and_then(|o| o.facts().cloned());
    let moved_at = s.moved_at;

    (
        AnchorView {
            key: key.clone(),
            status: s.state.status(),
            state: s.state,
            anchor: s.anchor,
            sighting,
            closed: s.closed,
            faltering: s.faltering,
            entered_at: s.entered_at,
            last_sighting,
            sightings,
            derivation,
            fact_address,
            facts,
        },
        moved_at,
    )
}

async fn ground(
    log: &AnchorLog,
    memory: &MemoryLens,
    view: AnchorView,
    moved_at: Option<Seq>,
    total: &Budget,
    call: Duration,
) -> Result<Grounded, RuntimeError> {
    let mut memories = Vec::new();
    for asserted in memory.bindings_on(log, &view.key).await? {
        let mut held = memory.fetch_memory(asserted, &total.narrowed(call)).await?;
        held.warrant = Some(warranted(log, &view.key, held.bound_at_seq, &view, moved_at).await?);
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
