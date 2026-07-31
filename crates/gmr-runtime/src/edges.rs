use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use gmr_core::{AnchorKey, Change, Entry, ReasonClass, Seq, State, StatusId, Version};
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
            walk(&key, &entries, cursor, now, &self.policy, &mut edges);
            if let Some((seq, _)) = entries.last() {
                head = head.max(*seq);
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

fn walk(
    key: &AnchorKey,
    entries: &[(Seq, Entry)],
    cursor: Seq,
    now: DateTime<Utc>,
    policy: &crate::policy::Policy,
    out: &mut Vec<Edge>,
) {
    let mut state = State::default();
    let mut terminal: BTreeSet<StatusId> = BTreeSet::new();
    let mut attempts: u32 = 0;
    let mut last_sighting: Option<DateTime<Utc>> = None;
    let mut closed = false;

    let is_terminal =
        |s: &State, t: &BTreeSet<StatusId>| s.status().is_some_and(|x| t.contains(&x));

    for (seq, entry) in entries {
        let fresh = *seq > cursor;
        match entry {
            Entry::Open {
                anchor,
                state: next,
                at,
                ..
            } => {
                terminal = anchor.terminal.clone();
                state = next.clone();
                closed = is_terminal(&state, &terminal);
                attempts = 0;
                last_sighting = Some(*at);
            }
            Entry::Transition {
                state: next, at, ..
            } => {
                if fresh && state != *next {
                    out.push(Edge::Transitioned {
                        anchor: key.clone(),
                        from: state.clone(),
                        to: next.clone(),
                        status: next.status(),
                        seq: *seq,
                        at: *at,
                    });
                    if is_terminal(next, &terminal) {
                        out.push(Edge::Closed {
                            anchor: key.clone(),
                            self_sealed: true,
                            seq: *seq,
                            at: *at,
                        });
                    }
                }
                state = next.clone();
                closed = is_terminal(&state, &terminal);
                attempts = 0;
                last_sighting = Some(*at);
            }
            Entry::Still { at, .. } => {
                attempts = 0;
                last_sighting = Some(*at);
            }
            Entry::Attempt { at, reason, .. } => {
                attempts += 1;
                if fresh && attempts == policy.stalled_attempts {
                    out.push(Edge::Stalled {
                        anchor: key.clone(),
                        reason: Stall::Attempts {
                            count: attempts,
                            last: *reason,
                        },
                        seq: *seq,
                        at: *at,
                    });
                }
            }
            Entry::Revise { change, .. } => {
                match change {
                    Change::Reterminal { terminal: t } => terminal = t.clone(),
                    Change::Restate { state: s } => state = s.clone(),
                    _ => {}
                }
                closed = is_terminal(&state, &terminal);
            }
            Entry::Close { at, .. } => {
                closed = true;
                if fresh {
                    out.push(Edge::Closed {
                        anchor: key.clone(),
                        self_sealed: false,
                        seq: *seq,
                        at: *at,
                    });
                }
            }
        }
    }

    if !closed
        && let Some((seq, _)) = entries.last()
        && last_sighting.is_none_or(|t| (now - t).num_seconds() > policy.stalled_staleness_secs)
    {
        out.push(Edge::Stalled {
            anchor: key.clone(),
            reason: Stall::Stale { last_sighting },
            seq: *seq,
            at: now,
        });
    }
}
