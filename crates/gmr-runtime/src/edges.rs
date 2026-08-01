use chrono::{DateTime, Utc};
use gmr_core::{AnchorKey, AnchorState, Entry, ReasonClass, Seq, State, StatusId, Version, scan};
use serde::Serialize;

use crate::assembly::Runtime;
use crate::error::RuntimeError;

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
        reason: Stall,
        seq: Seq,
        at: DateTime<Utc>,
    },
    Rewritten {
        anchor: AnchorKey,
        reference: gmr_core::Ref,
        bound_version: Version,
        current_version: Option<Version>,
        retrievable: Option<bool>,
        at: DateTime<Utc>,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum Stall {
    Attempts {
        count: u32,
        last: ReasonClass,
    },
    Stale {
        last_sighting: Option<DateTime<Utc>>,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct Edges {
    pub edges: Vec<Edge>,
    pub cursor: Seq,
}

impl Runtime {
    pub async fn changed_since(
        &self,
        cursor: Seq,
        status: Option<&StatusId>,
    ) -> Result<Edges, RuntimeError> {
        let now = Utc::now();
        let mut edges = Vec::new();
        let mut head = cursor;

        for key in self.journal.anchors().await? {
            let entries = self.journal.entries(&key, 0).await?;
            let last = walk(&key, &entries, cursor, &self.policy, &mut edges);
            if let Some((seq, _)) = entries.last() {
                head = head.max(*seq);
                if let Some(s) = last
                    && !s.closed
                    && s.last_sighting.is_none_or(|t| {
                        (now - t).num_seconds() > self.policy.stalled_staleness_secs
                    })
                {
                    edges.push(Edge::Stalled {
                        anchor: key.clone(),
                        reason: Stall::Stale {
                            last_sighting: s.last_sighting,
                        },
                        seq: *seq,
                        at: now,
                    });
                }
            }

            for binding in self.bindings.bindings_on(&key).await? {
                let view = self.fetch_memory(binding).await;
                if view.rewritten {
                    edges.push(Edge::Rewritten {
                        anchor: key.clone(),
                        reference: view.reference,
                        bound_version: view.bound_version,
                        current_version: view.current_version,
                        retrievable: view.retrievable,
                        at: now,
                    });
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
            Edge::Transitioned { seq, .. }
            | Edge::Closed { seq, .. }
            | Edge::Stalled { seq, .. } => *seq,
            Edge::Rewritten { .. } => Seq::MAX,
        });

        Ok(Edges {
            edges,
            cursor: head,
        })
    }
}

/// 边沿从**同一份折叠**里派生出来，不另写一份投影。
///
/// 手写第二份的代价不是重复，是漂：两份对「什么算关了」的算法一旦分家，
/// 没有任何东西会发现，而边沿正是消费方唯一看得见的东西。
fn walk(
    key: &AnchorKey,
    entries: &[(Seq, Entry)],
    cursor: Seq,
    policy: &crate::policy::Policy,
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
                // 自己走进终结集合，还是作者伸手关的 —— 这两件事的处置不同。
                self_sealed: !matches!(entry, Entry::Close { .. }),
                seq,
                at: entry.at(),
            });
        }

        if fresh
            && let Entry::Attempt { reason, .. } = entry
            && now.attempts == policy.stalled_attempts
        {
            out.push(Edge::Stalled {
                anchor: key.clone(),
                reason: Stall::Attempts {
                    count: now.attempts,
                    last: *reason,
                },
                seq,
                at: entry.at(),
            });
        }

        was = now.state.clone();
        was_closed = now.closed;
    })
}
