use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use gmr_core::{AnchorKey, Change, ChangeKind, ContentHash, Entry, Ref, Seq, State, scan};
use serde::Serialize;

use crate::assembly::Runtime;
use crate::error::RuntimeError;
use crate::log::AnchorLog;
use crate::memory::MemoryLens;
use crate::read::{AnchorView, Footing, Grounded};

#[derive(Debug, Clone, Serialize)]
pub struct AnchorHealth {
    pub anchor: AnchorKey,
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
    let mut rationale_hashes: Vec<ContentHash> = Vec::new();
    let mut initial: Option<State> = None;
    let mut last_failure = None;

    let s = scan(&entries, |_, entry, _| match entry {
        Entry::Open { state, .. } => initial = Some(state.clone()),
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

async fn corpus_health(
    memory: &MemoryLens,
    grounded: &[Grounded],
) -> Result<CorpusHealth, RuntimeError> {
    let bindings = crate::memory::by_reference(memory.all().await?);
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
        .filter_map(|b| b.standing().map(|r| r.binding.reference.clone()))
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

    Ok(CorpusHealth {
        bound_refs: bindings.len(),
        active_anchors: open.len(),
        memories_per_anchor: per_anchor,
        barren_anchors: barren,
        unsupervised,
        footings,
    })
}
