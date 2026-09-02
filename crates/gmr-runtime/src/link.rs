use std::collections::{BTreeSet, VecDeque};

use gmr_budget::Budget;
use gmr_core::{LinkKind, Ref, Source};
use gmr_store::{LinkRecord, LinkRevocation};
use serde::Serialize;

use crate::assembly::Runtime;
use crate::error::RuntimeError;
use crate::memory::MemoryLens;
use crate::read::Footing;

pub const REACHED_AT_MOST: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Links {
    pub out: Vec<crate::read::Linked>,
    #[serde(rename = "in")]
    pub incoming: Vec<Inbound>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Inbound {
    pub from: Ref,
    pub kind: gmr_core::LinkKind,
    pub source: gmr_core::Source,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Reached {
    pub reference: Ref,
    pub via: Vec<LinkKind>,
    pub depth: usize,
    pub footing: Footing,
}

impl Runtime {
    pub async fn link(
        &self,
        from: &Ref,
        to: &Ref,
        kind: LinkKind,
        source: Source,
    ) -> Result<(), RuntimeError> {
        self.memory
            .link(from, to, kind, source, chrono::Utc::now())
            .await
    }

    pub async fn unlink(&self, revocation: &LinkRevocation) -> Result<u64, RuntimeError> {
        self.memory.unlink(revocation).await
    }

    pub async fn links_of(&self, reference: &Ref) -> Result<Vec<LinkRecord>, RuntimeError> {
        self.memory.links_of(reference).await
    }

    pub async fn all_links(&self) -> Result<Vec<(Ref, LinkRecord)>, RuntimeError> {
        self.memory.all_links().await
    }

    pub async fn links(&self, reference: &Ref) -> Result<Links, RuntimeError> {
        let out = self
            .memory
            .links_of(reference)
            .await?
            .into_iter()
            .map(|r| crate::read::Linked {
                to: r.to,
                kind: r.kind,
                source: r.source,
                at: r.at,
            })
            .collect();
        let incoming = self
            .memory
            .links_to(reference)
            .await?
            .into_iter()
            .map(|(from, r)| Inbound {
                from,
                kind: r.kind,
                source: r.source,
                at: r.at,
            })
            .collect();
        Ok(Links { out, incoming })
    }

    pub async fn reaching(&self, from: &Ref, depth: usize) -> Result<Vec<Reached>, RuntimeError> {
        let policy = self.scheduler.policy();
        reaching(
            &self.memory,
            from,
            depth,
            &policy.content_budget(),
            policy.content_call(),
        )
        .await
    }
}

pub(crate) async fn reaching(
    memory: &MemoryLens,
    from: &Ref,
    depth: usize,
    total: &Budget,
    call: std::time::Duration,
) -> Result<Vec<Reached>, RuntimeError> {
    if depth == 0 {
        return Ok(Vec::new());
    }
    let mut seen = BTreeSet::from([from.clone()]);
    let mut queue = VecDeque::from([(from.clone(), Vec::new())]);
    let mut out = Vec::new();

    while let Some((at, via)) = queue.pop_front() {
        if via.len() >= depth || out.len() >= REACHED_AT_MOST {
            continue;
        }
        for link in memory.links_of(&at).await? {
            if !seen.insert(link.to.clone()) {
                continue;
            }
            let mut path = via.clone();
            path.push(link.kind.clone());
            let bound = memory.binding_of(&link.to.clone().into()).await?;
            let footing = memory
                .grounding_of(
                    &link.to,
                    bound.bound_version(),
                    &total.narrowed(call),
                    false,
                )
                .await
                .footing();
            queue.push_back((link.to.clone(), path.clone()));
            if !footing.is_current() {
                out.push(Reached {
                    reference: link.to,
                    depth: path.len(),
                    via: path,
                    footing,
                });
            }
            if out.len() >= REACHED_AT_MOST {
                break;
            }
        }
    }
    Ok(out)
}
