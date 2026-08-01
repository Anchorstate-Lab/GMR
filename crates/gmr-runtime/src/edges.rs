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
        count: u32,
        last: ReasonClass,
        seq: Seq,
        at: DateTime<Utc>,
    },
}

/// 此刻仍然成立的**状况**，不是发生过的事。
///
/// 它们不来自日志：陈旧是拿当下的钟去比对最后一次看见，改写要向提供方问
/// 「现在是哪一版」。日志游标因此表达不了「这条我看过了」—— 硬塞进边沿里
/// 只会让每一次轮询都重新报一遍同样的东西，而消费方无从分辨。
///
/// 所以把它们摆在另一格：**按内容去重，不按游标。**
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
    /// 游标之后日志里真的发生过的事。问过就不再交第二次。
    pub edges: Vec<Edge>,
    /// 此刻的状况。每次问都重新算 —— 见 [`Standing`]。
    pub standing: Vec<Standing>,
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
        let mut standing = Vec::new();
        let mut head = cursor;

        for key in self.journal.anchors().await? {
            let entries = self.journal.entries(&key, 0).await?;
            let last = walk(&key, &entries, cursor, &self.policy, &mut edges);
            if let Some((seq, _)) = entries.last() {
                head = head.max(*seq);
            }
            if let Some(s) = last
                && !s.closed
                && s.last_sighting
                    .is_none_or(|t| (now - t).num_seconds() > self.policy.stalled_staleness_secs)
            {
                standing.push(Standing::Stale {
                    anchor: key.clone(),
                    last_sighting: s.last_sighting,
                });
            }

            for binding in self.bindings.bindings_on(&key).await? {
                let view = self.fetch_memory(binding).await;
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

        // 问了具体状态就是问了一个具体问题：别拿状况去搅它。
        if let Some(want) = status {
            edges.retain(|e| match e {
                Edge::Transitioned { status, .. } => status.as_ref() == Some(want),
                _ => false,
            });
            standing.clear();
        }

        edges.sort_by_key(|e| match e {
            Edge::Transitioned { seq, .. }
            | Edge::Closed { seq, .. }
            | Edge::Stalled { seq, .. } => *seq,
        });

        Ok(Edges {
            edges,
            standing,
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

        // 规则写错了重试一万次也不会好 —— 第一次就得响，而且别跟
        // 「世界这会儿够不着」共用一个计数器。
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
