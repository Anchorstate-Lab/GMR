use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use gmr_core::{AnchorKey, Change, ChangeKind, ContentHash, Entry, Ref, Seq, State, scan};
use serde::Serialize;

use crate::assembly::Runtime;
use crate::error::RuntimeError;
use crate::log::AnchorLog;
use crate::memory::MemoryLens;
use crate::read::{AnchorView, Footing, Grounded, HoldingKind, KnowledgeKind, knowledge_of};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct Aim {
    pub readings: u32,
    pub answered: u32,
    pub moved_a_memory: u32,
}

impl Aim {
    pub fn never_fired(&self) -> bool {
        self.answered == 0 && self.readings > 0
    }

    pub fn fired_and_changed_nothing(&self) -> bool {
        self.answered > 0 && self.moved_a_memory == 0
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AnchorHealth {
    pub anchor: AnchorKey,
    pub aim: Aim,
    pub revisions: BTreeMap<ChangeKind, u32>,
    pub restate_count: u32,
    pub restate_interval_secs: Vec<i64>,
    pub state_drifted: bool,
    pub rationale_sizes: Vec<usize>,
    pub stall_ratio: f64,
    pub last_failure: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CorpusHealth {
    pub bound_refs: usize,
    pub active_anchors: usize,
    pub memories_per_anchor: BTreeMap<String, usize>,
    pub barren_anchors: Vec<AnchorKey>,
    pub unsupervised: Vec<Ref>,
    pub footings: BTreeMap<Footing, Vec<Ref>>,
    pub holdings: BTreeMap<HoldingKind, BTreeMap<AnchorKey, Vec<Ref>>>,
    pub knowings: BTreeMap<KnowledgeKind, Vec<AnchorKey>>,
}

impl CorpusHealth {
    pub fn on(&self, footing: Footing) -> &[Ref] {
        self.footings.get(&footing).map_or(&[], Vec::as_slice)
    }

    pub fn grounded_records(&self) -> usize {
        self.footings.values().map(Vec::len).sum()
    }
}

pub struct Corpus {
    grounded: Vec<Grounded>,
    health: CorpusHealth,
}

impl Corpus {
    pub fn len(&self) -> usize {
        self.grounded.len()
    }

    pub fn is_empty(&self) -> bool {
        self.grounded.is_empty()
    }

    pub fn anchors(&self) -> impl Iterator<Item = &AnchorView> {
        self.grounded.iter().map(|g| &g.view)
    }

    pub fn live(&self) -> Vec<&AnchorView> {
        self.anchors().filter(|v| !v.closed).collect()
    }

    pub fn health(&self) -> &CorpusHealth {
        &self.health
    }
}

const RECENT: usize = 50;

impl Runtime {
    pub async fn health(&self, key: &AnchorKey) -> Result<AnchorHealth, RuntimeError> {
        health(&self.log, &self.memory, key).await
    }

    pub async fn corpus(&self) -> Result<Corpus, RuntimeError> {
        let grounded = self.grounded_all().await?;
        let health = corpus_health(&self.memory, &grounded).await?;
        Ok(Corpus { grounded, health })
    }
}

async fn health(
    log: &AnchorLog,
    memory: &MemoryLens,
    key: &AnchorKey,
) -> Result<AnchorHealth, RuntimeError> {
    let entries = log.entries(key, 0).await?;

    let mut restate_at: Vec<DateTime<Utc>> = Vec::new();
    let mut restate_seq: Vec<Seq> = Vec::new();
    let mut readings: u32 = 0;
    let mut rationale_hashes: Vec<ContentHash> = Vec::new();
    let mut initial: Option<State> = None;
    let mut last_failure = None;

    let s = scan(&entries, |seq, entry, _| match entry {
        Entry::Open { state, .. } => {
            readings += 1;
            initial = Some(state.clone());
        }
        Entry::Transition { .. } | Entry::Still { .. } => readings += 1,
        Entry::Attempt {
            reason,
            code,
            message,
            ..
        } => {
            last_failure = Some(match code {
                Some(c) => format!("{c:?}: {message}"),
                None => format!("{reason:?}: {message}"),
            });
        }
        Entry::Revise {
            change,
            rationale,
            at,
            ..
        } => {
            if matches!(change, Change::Restate { .. }) {
                restate_at.push(*at);
                restate_seq.push(seq);
            }
            rationale_hashes.push(rationale.clone());
        }
        _ => {}
    })
    .ok_or_else(|| RuntimeError::NoSuchAnchor { key: key.clone() })?;

    let mut rationale_sizes = Vec::new();
    for rationale in &rationale_hashes {
        if let Some(bytes) = memory.sealed(rationale).await? {
            rationale_sizes.push(bytes.len());
        }
    }

    let recent: Vec<&(Seq, Entry)> = entries.iter().rev().take(RECENT).collect();
    let failed = recent
        .iter()
        .filter(|(_, e)| matches!(e, Entry::Attempt { .. }))
        .count();

    Ok(AnchorHealth {
        anchor: key.clone(),
        aim: aimed(log, memory, key, readings, &restate_seq).await?,
        restate_count: *s.revisions.get(&ChangeKind::Restate).unwrap_or(&0),
        restate_interval_secs: restate_at
            .windows(2)
            .map(|w| (w[1] - w[0]).num_seconds())
            .collect(),
        state_drifted: initial.is_some_and(|start| start != s.state),
        revisions: s.revisions,
        rationale_sizes,
        stall_ratio: if recent.is_empty() {
            0.0
        } else {
            failed as f64 / recent.len() as f64
        },
        last_failure,
    })
}

async fn aimed(
    log: &AnchorLog,
    memory: &MemoryLens,
    key: &AnchorKey,
    readings: u32,
    restates: &[Seq],
) -> Result<Aim, RuntimeError> {
    let mut aim = Aim {
        readings,
        answered: restates.len() as u32,
        moved_a_memory: 0,
    };
    if restates.is_empty() {
        return Ok(aim);
    }
    let mut stamped: Vec<Vec<(Seq, gmr_core::Version)>> = Vec::new();
    for bound in memory.bindings_on(log, key).await? {
        let mut each: Vec<(Seq, gmr_core::Version)> = bound
            .assertions()
            .iter()
            .filter_map(|r| Some((r.bound_at_seq?, r.bound_version.clone()?)))
            .collect();
        each.sort_by_key(|(seq, _)| *seq);
        stamped.push(each);
    }

    for (at, after) in restates.iter().zip(
        restates
            .iter()
            .skip(1)
            .copied()
            .chain(std::iter::once(Seq::MAX)),
    ) {
        let moved = stamped.iter().any(|each| {
            let before = each.iter().rev().find(|(seq, _)| seq <= at);
            let later = each.iter().rev().find(|(seq, _)| *seq < after);
            match (before, later) {
                (Some((_, a)), Some((_, b))) => a != b,
                (None, Some(_)) => true,
                _ => false,
            }
        });
        aim.moved_a_memory += u32::from(moved);
    }
    Ok(aim)
}

async fn corpus_health(
    memory: &MemoryLens,
    grounded: &[Grounded],
) -> Result<CorpusHealth, RuntimeError> {
    let bindings = crate::memory::by_claim(memory.all().await?);
    let views = || grounded.iter().map(|g| &g.view);
    let open: BTreeSet<&AnchorKey> = views().filter(|v| !v.closed).map(|v| &v.key).collect();

    let mut per_anchor: BTreeMap<String, usize> = BTreeMap::new();
    let mut barren = Vec::new();
    for held in grounded {
        let n = held.memories.len();
        per_anchor.insert(held.view.key.to_string(), n);
        if !held.view.closed && n == 0 {
            barren.push(held.view.key.clone());
        }
    }

    let delivered: BTreeSet<&Ref> = grounded
        .iter()
        .filter(|g| !g.view.closed)
        .flat_map(|g| g.memories.iter().map(|m| &m.reference))
        .collect();
    let unsupervised: Vec<Ref> = bindings
        .iter()
        .filter(|b| !b.anchors().is_empty())
        .filter_map(|b| b.stored().cloned())
        .filter(|reference| !delivered.contains(reference))
        .collect();

    let mut footings: BTreeMap<Footing, Vec<Ref>> = BTreeMap::new();
    for m in grounded.iter().flat_map(|g| &g.memories) {
        footings
            .entry(m.footing())
            .or_default()
            .push(m.reference.clone());
    }
    for refs in footings.values_mut() {
        refs.sort();
        refs.dedup();
    }

    let mut holdings: BTreeMap<HoldingKind, BTreeMap<AnchorKey, Vec<Ref>>> = BTreeMap::new();
    for held in grounded {
        for m in &held.memories {
            let Some(warrant) = m.warrant.as_ref() else {
                continue;
            };
            holdings
                .entry(warrant.holding.kind())
                .or_default()
                .entry(held.view.key.clone())
                .or_default()
                .push(m.reference.clone());
        }
    }
    for anchors in holdings.values_mut() {
        for refs in anchors.values_mut() {
            refs.sort();
            refs.dedup();
        }
    }

    let mut knowings: BTreeMap<KnowledgeKind, Vec<AnchorKey>> = BTreeMap::new();
    for view in views().filter(|v| !v.closed) {
        knowings
            .entry(knowledge_of(view).kind())
            .or_default()
            .push(view.key.clone());
    }
    for keys in knowings.values_mut() {
        keys.sort();
        keys.dedup();
    }

    Ok(CorpusHealth {
        bound_refs: bindings.len(),
        active_anchors: open.len(),
        memories_per_anchor: per_anchor,
        barren_anchors: barren,
        unsupervised,
        footings,
        holdings,
        knowings,
    })
}
