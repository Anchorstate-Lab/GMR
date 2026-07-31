use chrono::{DateTime, Utc};
use gmr_core::{
    Anchor, AnchorKey, Binding, Entry, Link, Outcome, Ref, Seq, State, StatusId, Version, fold,
};
use serde::Serialize;

use crate::assembly::Runtime;
use crate::error::RuntimeError;

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
    pub attempts: u32,
    pub entered_at: Option<DateTime<Utc>>,
    pub last_sighting: Option<DateTime<Utc>>,
    pub sightings: u64,
    pub memories: Vec<MemoryView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemoryView {
    pub reference: Ref,
    pub bound_version: Version,
    pub current_version: Option<Version>,
    pub rewritten: bool,
    pub content: Option<String>,
    pub content_at_bind: Option<String>,
    pub retrievable: Option<bool>,
    pub grounded: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unavailable: Option<String>,
    pub links: Vec<Link>,
}

impl Runtime {
    pub async fn read(&self, key: &AnchorKey) -> Result<AnchorView, RuntimeError> {
        let entries = self.journal.entries(key, 0).await?;
        let s = fold(&entries).ok_or_else(|| RuntimeError::NoSuchAnchor { key: key.clone() })?;

        let mut memories = Vec::new();
        for binding in self.bindings.bindings_on(key).await? {
            memories.push(self.fetch_memory(binding).await);
        }

        let sighting = match s.latest.as_ref().map(|o| &o.outcome) {
            Some(Outcome::Found { .. }) => Sighting::Found,
            _ => Sighting::Absent,
        };

        Ok(AnchorView {
            key: key.clone(),
            status: s.state.status(),
            state: s.state,
            anchor: s.anchor,
            sighting,
            closed: s.closed,
            attempts: s.attempts,
            entered_at: s.entered_at,
            last_sighting: s.last_sighting,
            sightings: count_sightings(&entries),
            memories,
        })
    }

    pub async fn cobound(&self, reference: &Ref) -> Result<Vec<Ref>, RuntimeError> {
        let Some(binding) = self.bindings.binding_of(reference).await? else {
            return Ok(Vec::new());
        };
        let mut out: Vec<Ref> = Vec::new();
        for anchor in &binding.anchors {
            for other in self.bindings.bindings_on(anchor).await? {
                if &other.reference != reference && !out.contains(&other.reference) {
                    out.push(other.reference);
                }
            }
        }
        out.sort();
        Ok(out)
    }

    pub async fn read_all(&self) -> Result<Vec<AnchorView>, RuntimeError> {
        let mut out = Vec::new();
        for key in self.journal.anchors().await? {
            out.push(self.read(&key).await?);
        }
        Ok(out)
    }
}

impl Runtime {
    pub(crate) async fn fetch_memory(&self, binding: Binding) -> MemoryView {
        let mut view = MemoryView {
            reference: binding.reference.clone(),
            bound_version: binding.bound_version.clone(),
            current_version: None,
            rewritten: false,
            content: None,
            content_at_bind: None,
            retrievable: None,
            grounded: !binding.anchors.is_empty(),
            unavailable: None,
            links: binding.links,
        };

        let Some(provider) = self
            .providers
            .iter()
            .find(|p| p.provider() == &binding.reference.provider)
        else {
            view.unavailable = Some(format!(
                "没有提供方认得 `{}` 这种引用",
                binding.reference.provider
            ));
            return view;
        };

        match provider.fetch(&binding.reference.external_id).await {
            Err(e) => view.unavailable = Some(e.message),
            Ok(None) => view.unavailable = Some("提供方说这条记录不存在了".to_owned()),
            Ok(Some(fetched)) => {
                view.rewritten = fetched.version != binding.bound_version;
                view.current_version = Some(fetched.version);
                match String::from_utf8(fetched.bytes) {
                    Ok(text) => view.content = Some(text),
                    Err(_) => view.unavailable = Some("记录不是 UTF-8 文本".to_owned()),
                }

                if view.rewritten {
                    match provider
                        .fetch_at(&binding.reference.external_id, &binding.bound_version)
                        .await
                    {
                        Ok(Some(bytes)) => {
                            view.retrievable = Some(true);
                            view.content_at_bind = String::from_utf8(bytes).ok();
                        }
                        Ok(None) => view.retrievable = Some(false),
                        Err(e) => view.unavailable = Some(e.message),
                    }
                } else {
                    view.retrievable = Some(true);
                }
            }
        }
        view
    }
}

fn count_sightings(entries: &[(Seq, Entry)]) -> u64 {
    entries.iter().filter(|(_, e)| e.is_sighting()).count() as u64
}
