use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use gmr_core::{AnchorKey, Change, ChangeKind, ContentHash, Entry, Seq, State, fold, scan};
use serde::Serialize;

use crate::assembly::Runtime;
use crate::error::RuntimeError;
use crate::log::AnchorLog;
use crate::memory::MemoryLens;

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
}

const RECENT: usize = 50;

impl Runtime {
    pub async fn health(&self, key: &AnchorKey) -> Result<AnchorHealth, RuntimeError> {
        health(&self.log, &self.memory, key).await
    }

    pub async fn corpus_health(&self) -> Result<CorpusHealth, RuntimeError> {
        corpus_health(&self.log, &self.memory).await
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

    // One walk decides both the canonical state (`s`) and everything below that
    // needs a per-event view (restate timestamps, rationale hashes, the last
    // failure) — a second hand-rolled loop over the same entries risked
    // disagreeing with `s.revisions` about what counts as a restate.
    let s = scan(&entries, |_, entry, _| match entry {
        Entry::Open { state, .. } => initial = Some(state.clone()),
        Entry::Attempt {
            reason, message, ..
        } => {
            last_failure = Some(format!("{reason:?}: {message}"));
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

    // Fetching sealed bytes is the one part of this that has to be async I/O,
    // so it stays a separate pass — but only over the handful of rationales
    // the scan above already identified, not the whole entry list again.
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

async fn corpus_health(log: &AnchorLog, memory: &MemoryLens) -> Result<CorpusHealth, RuntimeError> {
    let bindings = memory.all().await?;
    let anchors = log.anchors().await?;

    let mut per_anchor: BTreeMap<String, usize> = BTreeMap::new();
    let mut active = 0;
    let mut barren = Vec::new();
    for key in &anchors {
        let n = bindings
            .iter()
            .filter(|r| r.binding.anchors.contains(key))
            .count();
        per_anchor.insert(key.to_string(), n);
        let entries = log.entries(key, 0).await?;
        if fold(&entries).is_some_and(|s| !s.closed) {
            active += 1;
            if n == 0 {
                barren.push(key.clone());
            }
        }
    }

    Ok(CorpusHealth {
        bound_refs: bindings.len(),
        active_anchors: active,
        memories_per_anchor: per_anchor,
        barren_anchors: barren,
    })
}
