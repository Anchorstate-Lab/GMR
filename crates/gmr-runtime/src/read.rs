use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use chrono::{DateTime, Utc};
use futures_util::{StreamExt, TryStreamExt, future::try_join};
use gmr_budget::Budget;
use gmr_content::ContentErrorCode;
use gmr_core::{
    Anchor, AnchorKey, AnchorState, Claim, Derivation, Entry, Expr, FactAddress, Facts,
    FailureCode, Faltering, LinkKind, Outcome, ProbeVersion, ProviderId, ReasonClass, Ref, SaidId,
    Seq, Source, State, StatusId, Verifiability, Version,
};
use gmr_store::Seen;
use serde::{Deserialize, Serialize};

use crate::assembly::Runtime;
use crate::error::RuntimeError;
use crate::log::AnchorLog;
use crate::memory::MemoryLens;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Instructions {
    #[serde(
        rename = "max_staleness_ms",
        default,
        skip_serializing_if = "Option::is_none",
        with = "millis"
    )]
    pub max_staleness: Option<Duration>,
    #[serde(
        rename = "budget_ms",
        default,
        skip_serializing_if = "Option::is_none",
        with = "millis"
    )]
    pub budget: Option<Duration>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reach: Option<usize>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub carry: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub lean: bool,
}

mod millis {
    use std::time::Duration;

    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(held: &Option<Duration>, s: S) -> Result<S::Ok, S::Error> {
        held.map(|span| u64::try_from(span.as_millis()).unwrap_or(u64::MAX))
            .serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Duration>, D::Error> {
        Ok(Option::<u64>::deserialize(d)?.map(Duration::from_millis))
    }
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Reading {
    pub key: AnchorKey,
    pub sighting: Sighting,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub facts: Option<Facts>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fact_address: Option<FactAddress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub derivation: Option<Derivation>,
    pub at: Option<DateTime<Utc>>,
    pub knowledge: Knowledge,
}

impl From<AnchorView> for Reading {
    fn from(view: AnchorView) -> Self {
        Self {
            knowledge: knowledge_of(&view),
            key: view.key,
            sighting: view.sighting,
            facts: view.facts,
            fact_address: view.fact_address,
            derivation: view.derivation,
            at: view.last_sighting,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Grounded {
    #[serde(flatten)]
    pub view: AnchorView,
    pub memories: Vec<MemoryView>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub said: Vec<SaidView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SaidView {
    pub id: SaidId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asserts: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depends: Option<Expr>,
    pub sources: std::collections::BTreeSet<Source>,
    pub bound_at_seq: Option<Seq>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asserted_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warrant: Option<Warrant>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Linked {
    pub to: Ref,
    pub kind: LinkKind,
    pub source: Source,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemoryView {
    pub reference: Ref,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<SaidId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bound_version: Option<Version>,
    pub grounded: bool,
    pub links: Vec<Linked>,
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
        #[serde(
            serialize_with = "as_text_held",
            skip_serializing_if = "Option::is_none"
        )]
        content: Option<Vec<u8>>,
    },
    Unverified {
        version: Version,
        #[serde(
            serialize_with = "as_text_held",
            skip_serializing_if = "Option::is_none"
        )]
        content: Option<Vec<u8>>,
    },
    Rewritten {
        version: Version,
        #[serde(
            serialize_with = "as_text_held",
            skip_serializing_if = "Option::is_none"
        )]
        content: Option<Vec<u8>>,
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
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub saw: BTreeSet<FactAddress>,
    #[serde(flatten)]
    pub shown: Shown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "shown", rename_all = "snake_case")]
pub enum Shown {
    Seen { at: Seq },
    Unseen,
    NotSaid,
}

impl Shown {
    pub fn is_seen(&self) -> bool {
        matches!(self, Self::Seen { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "anchored", rename_all = "snake_case")]
pub enum Anchored {
    On {
        key: AnchorKey,
        warrant: Box<Warrant>,
        evidence: Box<Evidence>,
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

    pub fn evidence(&self) -> Option<&Evidence> {
        match self {
            Self::On { evidence, .. } => Some(evidence),
            Self::Unopened { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asked {
    pub claim: Claim,
    pub anchors: Vec<AnchorKey>,
    pub saw: BTreeSet<FactAddress>,
    pub depends: Option<gmr_core::Expr>,
}

impl Asked {
    pub fn about(claim: Claim) -> Self {
        Self {
            claim,
            anchors: Vec::new(),
            saw: BTreeSet::new(),
            depends: None,
        }
    }

    pub fn on(mut self, anchors: impl IntoIterator<Item = AnchorKey>) -> Self {
        self.anchors = anchors.into_iter().collect();
        self
    }

    pub fn saw(mut self, addresses: impl IntoIterator<Item = FactAddress>) -> Self {
        self.saw = addresses.into_iter().collect();
        self
    }

    pub fn depending(mut self, source: impl Into<String>) -> Self {
        self.depends = Some(gmr_core::Expr::text(source));
        self
    }

    pub fn inline(&self) -> bool {
        !self.anchors.is_empty() || !self.saw.is_empty() || self.depends.is_some()
    }
}

enum Rests<'a> {
    Stored(&'a crate::memory::Bound),
    Inline(&'a Asked),
}

impl Rests<'_> {
    fn anchors(&self) -> &[AnchorKey] {
        match self {
            Self::Stored(bound) => bound.anchors(),
            Self::Inline(asked) => &asked.anchors,
        }
    }

    fn saw(&self) -> &BTreeSet<FactAddress> {
        static NONE: std::sync::LazyLock<BTreeSet<FactAddress>> =
            std::sync::LazyLock::new(BTreeSet::new);
        match self {
            Self::Stored(bound) => bound.saw(),
            Self::Inline(asked) => match asked.saw.is_empty() {
                true => &NONE,
                false => &asked.saw,
            },
        }
    }

    fn depends(&self) -> Option<&gmr_core::Expr> {
        match self {
            Self::Stored(bound) => bound.depends(),
            Self::Inline(asked) => asked.depends.as_ref(),
        }
    }

    fn origin(&self) -> Option<&SaidId> {
        match self {
            Self::Stored(bound) => bound.origin(),
            Self::Inline(_) => None,
        }
    }

    fn bound_at(&self) -> Option<Seq> {
        match self {
            Self::Stored(bound) => bound.dating().and_then(|r| r.bound_at_seq),
            Self::Inline(_) => None,
        }
    }

    fn bound_version(&self) -> Option<&Version> {
        match self {
            Self::Stored(bound) => bound.bound_version(),
            Self::Inline(_) => None,
        }
    }

    fn claim<'c>(&'c self, asked: &'c Claim) -> &'c Claim {
        match self {
            Self::Stored(bound) => bound.claim().unwrap_or(asked),
            Self::Inline(_) => asked,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Standing {
    pub claim: Claim,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record: Option<Grounding>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<SaidId>,
    #[serde(flatten)]
    pub depends: Depends,
    pub on: Vec<Anchored>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub reached: Vec<crate::link::Reached>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "depends", rename_all = "snake_case")]
pub enum Depends {
    Holds,
    Broken,
    Vacuous { wrote: String },
    Unevaluable { why: String },
    Unstated,
}

impl Depends {
    pub fn stated(&self) -> bool {
        !matches!(self, Self::Unstated)
    }
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
    NotAsked,
    Unreachable {
        code: ContentErrorCode,
        why: String,
    },
}

impl MemoryView {
    pub fn content(&self) -> Option<&[u8]> {
        match &self.grounding {
            Grounding::Current { content, .. } | Grounding::Rewritten { content, .. } => {
                content.as_deref()
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

fn as_text_held<S: serde::Serializer>(bytes: &Option<Vec<u8>>, s: S) -> Result<S::Ok, S::Error> {
    match bytes {
        Some(held) => as_text(held, s),
        None => s.serialize_none(),
    }
}

impl Runtime {
    pub async fn read(&self, key: &AnchorKey) -> Result<AnchorView, RuntimeError> {
        Ok(stand(&self.log, key, &self.scheduler.seen(key).await?)
            .await?
            .0)
    }

    pub async fn sample_all(&self) -> Result<Vec<Reading>, RuntimeError> {
        Ok(self
            .read_all()
            .await?
            .into_iter()
            .map(Reading::from)
            .collect())
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

    pub async fn sample(
        &self,
        key: &AnchorKey,
        how: &Instructions,
    ) -> Result<Reading, RuntimeError> {
        self.refresh(key, how).await?;
        Ok(stand(&self.log, key, &self.scheduler.seen(key).await?)
            .await?
            .0
            .into())
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
        let served = ground(
            &self.log,
            &self.memory,
            view,
            moved_at,
            &reaching(policy, how),
            policy.content_call(),
            how,
        )
        .await?;
        for held in &served.memories {
            if held.grounded {
                self.used(&Claim::Stored(held.reference.clone())).await?;
            }
        }
        for said in &served.said {
            self.used(&Claim::Said {
                id: said.id.clone(),
                asserts: None,
            })
            .await?;
        }
        Ok(served)
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
        asked: &[Asked],
        how: &Instructions,
    ) -> Result<Vec<Standing>, RuntimeError> {
        let policy = self.scheduler.policy();

        let mut bound = Vec::with_capacity(asked.len());
        for one in asked {
            let held = self.memory.binding_of(&one.claim).await?;
            if one.inline() && !held.is_empty() {
                return Err(RuntimeError::AlreadyAsserted {
                    claim: one.claim.clone(),
                });
            }
            bound.push(held);
        }
        let rests: Vec<Rests<'_>> = asked
            .iter()
            .zip(&bound)
            .map(|(one, held)| match held.is_empty() {
                true => Rests::Inline(one),
                false => Rests::Stored(held),
            })
            .collect();
        let keys: Vec<AnchorKey> = rests
            .iter()
            .flat_map(|r| r.anchors().iter().cloned())
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
            self.records_of(asked, &rests, &reading, policy.content_call(), how.lean),
        )
        .await?;

        let mut out = Vec::with_capacity(asked.len());
        for ((one, held), record) in asked.iter().zip(&rests).zip(records) {
            let claim = &one.claim;
            if matches!(held, Rests::Stored(_)) {
                self.used(claim).await?;
            }
            let mut on = Vec::with_capacity(held.anchors().len());
            for key in held.anchors() {
                on.push(anchored(&self.log, key, held, stood.get(key)).await?);
            }
            let reached = match (how.reach, claim.stored()) {
                (Some(depth), Some(from)) => {
                    crate::link::reaching(
                        &self.memory,
                        from,
                        depth,
                        &reading,
                        policy.content_call(),
                    )
                    .await?
                }
                _ => Vec::new(),
            };
            out.push(Standing {
                claim: held.claim(claim).clone(),
                record,
                origin: held.origin().cloned(),
                depends: depends(held, &stood),
                on,
                reached,
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

        let each = futures_util::stream::iter(keys.to_vec())
            .map(|key| {
                let looks = seen.get(&key).copied().unwrap_or_default();
                async move {
                    let Some((view, moved_at)) = standing_at(&self.log, &key, &looks).await? else {
                        return Ok::<_, RuntimeError>(None);
                    };
                    if view.closed || !how.stale(view.last_sighting, now) {
                        return Ok(Some((view, moved_at)));
                    }
                    match crate::observe::observe(
                        &self.log,
                        &self.observer,
                        &self.scheduler,
                        &key,
                        budget,
                    )
                    .await
                    {
                        Ok(_) | Err(RuntimeError::Leased { .. }) => {}
                        Err(e) => return Err(e),
                    }
                    standing_at(&self.log, &key, &self.scheduler.seen(&key).await?).await
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
        asked: &[Asked],
        rests: &[Rests<'_>],
        total: &Budget,
        call: Duration,
        lean: bool,
    ) -> Result<Vec<Option<Grounding>>, RuntimeError> {
        let mut out = Vec::with_capacity(asked.len());
        for (one, held) in asked.iter().zip(rests) {
            out.push(match one.claim.stored() {
                None => None,
                Some(reference) => Some(
                    self.memory
                        .grounding_of(reference, held.bound_version(), &total.narrowed(call), lean)
                        .await,
                ),
            });
        }
        Ok(out)
    }

    pub async fn cobound(&self, claim: &Claim) -> Result<Vec<Claim>, RuntimeError> {
        cobound(&self.log, &self.memory, claim).await
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
            out.push(ground(&self.log, &self.memory, view, moved_at, &total, call, how).await?);
        }
        Ok(out)
    }
}

async fn anchored(
    log: &AnchorLog,
    key: &AnchorKey,
    held: &Rests<'_>,
    stood: Option<&(AnchorView, Option<Seq>)>,
) -> Result<Anchored, RuntimeError> {
    let Some((view, moved_at)) = stood else {
        return Ok(Anchored::Unopened { key: key.clone() });
    };
    let bound_at = held.bound_at();
    let saw = held.saw().clone();
    let shown = shown_at(log, key, &saw).await?;
    Ok(Anchored::On {
        key: key.clone(),
        warrant: Box::new(warranted(log, key, bound_at, view, *moved_at).await?),
        evidence: Box::new(Evidence {
            reading: view.fact_address.clone(),
            instrument: view.derivation.as_ref().map(|d| d.version.clone()),
            bound_at,
            moved_at: *moved_at,
            saw,
            shown,
        }),
    })
}

fn depends(held: &Rests<'_>, stood: &BTreeMap<AnchorKey, (AnchorView, Option<Seq>)>) -> Depends {
    let Some(source) = held.depends() else {
        return Depends::Unstated;
    };
    let node = match gmr_expr::parse(&source.source) {
        Ok(node) => node,
        Err(e) => {
            return Depends::Unevaluable {
                why: format!("`{}`: {e}", source.source),
            };
        }
    };
    if !node.reads_anchors() {
        return Depends::Vacuous {
            wrote: source.source.clone(),
        };
    }
    let states: Vec<serde_json::Value> = held
        .anchors()
        .iter()
        .filter_map(|key| stood.get(key))
        .map(|(view, _)| view.state.as_value().clone())
        .collect();
    let nothing = serde_json::Value::Null;
    let ctx = gmr_expr::Ctx::new(&nothing, &nothing).over(&states);
    match gmr_expr::eval(&node, ctx) {
        gmr_expr::Evaluated::Value(serde_json::Value::Bool(true)) => Depends::Holds,
        gmr_expr::Evaluated::Value(serde_json::Value::Bool(false)) => Depends::Broken,
        gmr_expr::Evaluated::Value(other) => Depends::Unevaluable {
            why: format!("answered with {other}, which is not a yes or a no"),
        },
        gmr_expr::Evaluated::Absent => Depends::Unevaluable {
            why: "answered with nothing at all".to_owned(),
        },
        gmr_expr::Evaluated::Fault(f) => Depends::Unevaluable {
            why: format!("could not be settled: {}", f.class()),
        },
    }
}

async fn shown_at(
    log: &AnchorLog,
    key: &AnchorKey,
    saw: &BTreeSet<FactAddress>,
) -> Result<Shown, RuntimeError> {
    if saw.is_empty() {
        return Ok(Shown::NotSaid);
    }
    Ok(recorded_at(&log.entries(key, 0).await?, saw))
}

fn recorded_at(entries: &[(Seq, Entry)], saw: &BTreeSet<FactAddress>) -> Shown {
    let looked = entries.iter().find_map(|(seq, entry)| match entry {
        Entry::Open { observation, .. } | Entry::Transition { observation, .. } => {
            saw.contains(&observation.fact_address).then_some(*seq)
        }
        _ => None,
    });
    match looked {
        Some(at) => Shown::Seen { at },
        None => Shown::Unseen,
    }
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
        (serde_json::Value::Array(a), serde_json::Value::Array(b)) => {
            for at in 0..a.len().max(b.len()) {
                let next = match path.is_empty() {
                    true => at.to_string(),
                    false => format!("{path}.{at}"),
                };
                match (a.get(at), b.get(at)) {
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
    how: &Instructions,
) -> Result<Grounded, RuntimeError> {
    let mut memories = Vec::new();
    let mut said = Vec::new();
    for asserted in memory.bindings_on(log, &view.key).await? {
        match asserted.claim().cloned() {
            Some(Claim::Said { id, asserts }) => {
                let bound_at_seq = asserted.dating().and_then(|r| r.bound_at_seq);
                said.push(SaidView {
                    id,
                    asserts,
                    depends: asserted.depends().cloned(),
                    sources: asserted.sources(),
                    bound_at_seq,
                    asserted_at: asserted.first_asserted(),
                    warrant: Some(warranted(log, &view.key, bound_at_seq, &view, moved_at).await?),
                });
            }
            Some(Claim::Stored(_)) => {
                let Some(stored) = asserted.held() else {
                    continue;
                };
                let mut held = memory
                    .fetch_memory(stored, &total.narrowed(call), how.lean)
                    .await?;
                held.warrant =
                    Some(warranted(log, &view.key, held.bound_at_seq, &view, moved_at).await?);
                memories.push(held);
            }
            None => {}
        }
    }
    if how.carry {
        memory
            .carry_linked(&mut memories, total, call, how.lean)
            .await?;
    }
    Ok(Grounded {
        view,
        memories,
        said,
    })
}

async fn cobound(
    log: &AnchorLog,
    memory: &MemoryLens,
    claim: &Claim,
) -> Result<Vec<Claim>, RuntimeError> {
    let bound = memory.binding_of(claim).await?;
    let mut out: Vec<Claim> = Vec::new();
    for anchor in bound.anchors() {
        for other in memory.bindings_on(log, anchor).await? {
            let Some(beside) = other.claim().cloned() else {
                continue;
            };
            if !beside.same(claim) && !out.iter().any(|held| held.same(&beside)) {
                out.push(beside);
            }
        }
    }
    out.sort_by_key(Claim::to_string);
    Ok(out)
}
