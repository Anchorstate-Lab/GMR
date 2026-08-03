use chrono::{DateTime, Utc};
use gmr_core::{AnchorKey, AnchorState, Entry, ReasonClass, Seq, State, StatusId, Version, scan};
use serde::Serialize;

use crate::assembly::Runtime;
use crate::error::RuntimeError;
use crate::log::AnchorLog;
use crate::memory::MemoryLens;
use crate::policy::Policy;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "edge", rename_all = "snake_case")]
pub enum Edge {
    Transitioned {
        anchor: AnchorKey,
        from: State,
        to: State,
        status: Option<StatusId>,
        seq: Seq,
        at: DateTime<Utc>,
    },
    Closed {
        anchor: AnchorKey,
        self_sealed: bool,
        seq: Seq,
        at: DateTime<Utc>,
    },
    Stalled {
        anchor: AnchorKey,
        count: u32,
        last: ReasonClass,
        seq: Seq,
        at: DateTime<Utc>,
    },
}

/// A **condition** that holds right now, not something that happened.
///
/// These do not come from the log: staleness compares the current clock against
/// the last sighting, and a rewrite asks the provider which version it holds now.
/// A log cursor therefore cannot express "I have seen this one" — forcing them
/// into edges just re-reports the same thing on every poll, and the consumer has
/// no way to tell.
///
/// So they sit in their own slot: **deduplicate by content, not by cursor.**
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "standing", rename_all = "snake_case")]
pub enum Standing {
    Stale {
        anchor: AnchorKey,
        last_sighting: Option<DateTime<Utc>>,
    },
    Rewritten {
        anchor: AnchorKey,
        reference: gmr_core::Ref,
        bound_version: Version,
        current_version: Option<Version>,
        retrievable: Option<bool>,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct Edges {
    /// What actually happened in the log after the cursor. Handed over once.
    pub edges: Vec<Edge>,
    /// Conditions as of now — see [`Standing`]. `None` means not computed
    /// (a `status` filter was asked for), which is not the same as empty.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub standing: Option<Vec<Standing>>,
    pub cursor: Seq,
}

impl Runtime {
    pub async fn changed_since(
        &self,
        cursor: Seq,
        status: Option<&StatusId>,
    ) -> Result<Edges, RuntimeError> {
        changed_since(
            &self.log,
            &self.memory,
            self.scheduler.policy(),
            cursor,
            status,
        )
        .await
    }
}

async fn changed_since(
    log: &AnchorLog,
    memory: &MemoryLens,
    policy: &Policy,
    cursor: Seq,
    status: Option<&StatusId>,
) -> Result<Edges, RuntimeError> {
    let now = Utc::now();
    let mut edges = Vec::new();
    let mut standing = status.is_none().then(Vec::new);
    let mut head = cursor;

    for key in log.anchors().await? {
        let entries = log.entries(&key, 0).await?;
        let last = walk(&key, &entries, cursor, policy, &mut edges);
        if let Some((seq, _)) = entries.last() {
            head = head.max(*seq);
        }

        if let Some(standing) = standing.as_mut() {
            if let Some(s) = &last
                && !s.closed
                && s.last_sighting
                    .is_none_or(|t| (now - t).num_seconds() > policy.stalled_staleness_secs)
            {
                standing.push(Standing::Stale {
                    anchor: key.clone(),
                    last_sighting: s.last_sighting,
                });
            }

            for binding in memory.bindings_on(&key).await? {
                let view = memory.fetch_memory(binding).await?;
                if view.rewritten {
                    standing.push(Standing::Rewritten {
                        anchor: key.clone(),
                        reference: view.reference,
                        bound_version: view.bound_version,
                        current_version: view.current_version,
                        retrievable: view.retrievable,
                    });
                }
            }
        }
    }

    if let Some(want) = status {
        edges.retain(|e| match e {
            Edge::Transitioned { status, .. } => status.as_ref() == Some(want),
            _ => false,
        });
    }

    edges.sort_by_key(|e| match e {
        Edge::Transitioned { seq, .. } | Edge::Closed { seq, .. } | Edge::Stalled { seq, .. } => {
            *seq
        }
    });

    Ok(Edges {
        edges,
        standing,
        cursor: head,
    })
}

/// Edges are derived from **the same fold**, never a second projection.
///
/// The cost of a hand-written second copy is not duplication but drift: once the
/// two disagree on what counts as closed, nothing notices — and edges are the
/// only thing the consumer ever sees.
fn walk(
    key: &AnchorKey,
    entries: &[(Seq, Entry)],
    cursor: Seq,
    policy: &Policy,
    out: &mut Vec<Edge>,
) -> Option<AnchorState> {
    let mut was = State::default();
    let mut was_closed = false;

    scan(entries, |seq, entry, now| {
        let fresh = seq > cursor;

        if fresh && matches!(entry, Entry::Transition { .. }) && now.state != was {
            out.push(Edge::Transitioned {
                anchor: key.clone(),
                from: was.clone(),
                to: now.state.clone(),
                status: now.state.status(),
                seq,
                at: entry.at(),
            });
        }

        if fresh && now.closed && !was_closed {
            out.push(Edge::Closed {
                anchor: key.clone(),
                // Walked into the terminal set, or closed by the author — these
                // two are handled differently downstream.
                self_sealed: !matches!(entry, Entry::Close { .. }),
                seq,
                at: entry.at(),
            });
        }

        // A broken rule will not get better after ten thousand retries: it has to
        // be loud the first time, and must not share a counter with "the world is
        // out of reach right now".
        if fresh
            && let Entry::Attempt { reason, .. } = entry
            && (*reason == ReasonClass::Unevaluable || now.attempts == policy.stalled_attempts)
        {
            out.push(Edge::Stalled {
                anchor: key.clone(),
                count: now.attempts,
                last: *reason,
                seq,
                at: entry.at(),
            });
        }

        was = now.state.clone();
        was_closed = now.closed;
    })
}
