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
        current_version: Version,
        before: crate::read::Before,
    },
    Gone {
        anchor: AnchorKey,
        reference: gmr_core::Ref,
        bound_version: Version,
    },
    NoProvider {
        anchor: AnchorKey,
        reference: gmr_core::Ref,
        provider: gmr_core::ProviderId,
    },
    Unreachable {
        anchor: AnchorKey,
        reference: gmr_core::Ref,
        code: gmr_content::ContentErrorCode,
        why: String,
    },
}

impl Standing {
    pub fn of(anchor: AnchorKey, view: crate::read::MemoryView) -> Option<Self> {
        let crate::read::MemoryView {
            reference,
            bound_version,
            grounding,
            ..
        } = view;
        match grounding {
            crate::read::Grounding::Current { .. } => None,
            crate::read::Grounding::Rewritten {
                version, before, ..
            } => Some(Self::Rewritten {
                anchor,
                reference,
                bound_version,
                current_version: version,
                before,
            }),
            crate::read::Grounding::Gone => Some(Self::Gone {
                anchor,
                reference,
                bound_version,
            }),
            crate::read::Grounding::NoProvider { provider } => Some(Self::NoProvider {
                anchor,
                reference,
                provider,
            }),
            crate::read::Grounding::Unreachable { code, why } => Some(Self::Unreachable {
                anchor,
                reference,
                code,
                why,
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Edges {
    pub edges: Vec<Edge>,
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
    let total = policy.content_budget();
    let call = policy.content_call();
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
                let view = memory.fetch_memory(binding, &total.narrowed(call)).await?;
                standing.extend(Standing::of(key.clone(), view));
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
                self_sealed: !matches!(entry, Entry::Close { .. }),
                seq,
                at: entry.at(),
            });
        }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::read::{Before, Footing, Grounding, MemoryView};
    use gmr_content::ContentErrorCode;
    use gmr_core::{ProviderId, Ref};

    fn viewed(grounding: Grounding) -> MemoryView {
        MemoryView {
            reference: Ref::new("git", "m.md"),
            bound_version: Version::new("v1"),
            grounded: true,
            links: Vec::new(),
            bound_at_seq: None,
            stale: None,
            grounding,
        }
    }

    fn every_grounding() -> Vec<Grounding> {
        vec![
            Grounding::Current {
                version: Version::new("v1"),
                content: b"x".to_vec(),
            },
            Grounding::Rewritten {
                version: Version::new("v2"),
                content: b"y".to_vec(),
                before: Before::Retrieved {
                    content: b"x".to_vec(),
                },
            },
            Grounding::Rewritten {
                version: Version::new("v2"),
                content: b"y".to_vec(),
                before: Before::NotRetained,
            },
            Grounding::Rewritten {
                version: Version::new("v2"),
                content: b"y".to_vec(),
                before: Before::NoHistory,
            },
            Grounding::Gone,
            Grounding::NoProvider {
                provider: ProviderId::new("mem0"),
            },
            Grounding::Unreachable {
                code: ContentErrorCode::ProviderFailed,
                why: "the store said no".into(),
            },
            Grounding::Unreachable {
                code: ContentErrorCode::BudgetSpent,
                why: "nothing was asked".into(),
            },
        ]
    }

    #[test]
    fn the_two_corpus_walks_cannot_disagree_about_whether_a_record_is_fine() {
        for grounding in every_grounding() {
            let footing = grounding.footing();
            let raised = Standing::of(AnchorKey::new("a"), viewed(grounding.clone())).is_some();
            assert_eq!(
                raised,
                !footing.is_current(),
                "`edges` raises a standing and `doctor` counts a footing, over the same \
                 record. They were two matches on `Grounding` written apart, and they drifted: \
                 doctor walked only the open anchors, so a record the provider had deleted was \
                 `gone` under one verb and absent from the other — and the verb holding the \
                 exit code was the blind one. Whatever a new grounding shape means, both have \
                 to mean it: {grounding:?}"
            );
        }
    }

    #[test]
    fn every_footing_but_current_names_something_to_do_about_it() {
        let seen: std::collections::BTreeSet<Footing> =
            every_grounding().iter().map(Grounding::footing).collect();
        assert_eq!(
            seen.len(),
            7,
            "each footing has its own line in `doctor`, and a shape that stopped producing \
             one would take that line down with it silently"
        );
    }
}
