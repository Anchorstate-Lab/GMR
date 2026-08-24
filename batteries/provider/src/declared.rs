use std::sync::Arc;

use async_trait::async_trait;
use gmr_content::{ContentError, ContentProvider, Fetched, MemorySource, MemoryStore, Record};
use gmr_core::{ExternalId, Outcome, ProbeRef, ProviderId, Ref, Version, content_hash_of_bytes};
use gmr_probe::{Budget, ProbeCall, ProbeError, ProbeErrorCode, Transport};
use serde_json::Value;

pub const CONTRACT: &str = "gmr.provider-script.v1";

pub struct Declared {
    id: ProviderId,
    fetch: ProbeRef,
    transport: Arc<dyn Transport>,
}

struct Listing {
    id: ProviderId,
    list: ProbeRef,
    transport: Arc<dyn Transport>,
}

impl Declared {
    pub fn new(id: impl Into<String>, fetch: ProbeRef, transport: Arc<dyn Transport>) -> Self {
        Self {
            id: ProviderId::new(id),
            fetch,
            transport,
        }
    }

    pub fn store(self) -> MemoryStore {
        MemoryStore::new(Arc::new(self))
    }

    pub fn listing(self, list: ProbeRef) -> MemoryStore {
        let source = Listing {
            id: self.id.clone(),
            list,
            transport: Arc::clone(&self.transport),
        };
        self.store().listing(Arc::new(source))
    }
}

#[async_trait]
impl ContentProvider for Declared {
    fn provider(&self) -> &ProviderId {
        &self.id
    }

    async fn fetch(
        &self,
        id: &ExternalId,
        budget: &Budget,
    ) -> Result<Option<Fetched>, ContentError> {
        let position = serde_json::json!({ "id": id.as_str() });
        let call = ProbeCall {
            probe: &self.fetch,
            position: &position,
            budget,
        };
        match self.transport.invoke(&call).await.map_err(refused)? {
            Outcome::NotFound => Ok(None),
            Outcome::Found { facts } => body(facts.as_value()).map(Some),
        }
    }
}

#[async_trait]
impl MemorySource for Listing {
    fn provider(&self) -> &ProviderId {
        &self.id
    }

    async fn list(&self, budget: &Budget) -> Result<Vec<Record>, ContentError> {
        let position = Value::Object(Default::default());
        let call = ProbeCall {
            probe: &self.list,
            position: &position,
            budget,
        };
        let facts = match self.transport.invoke(&call).await.map_err(refused)? {
            Outcome::NotFound => {
                return Err(ContentError::new(format!(
                    "the listing script for `{}` answered `null`, which this contract spells \
                     \"no such record\". A store with nothing in it lists `{{\"records\": []}}`, \
                     and a store that cannot list declares no listing script at all",
                    self.id
                )));
            }
            Outcome::Found { facts } => facts,
        };
        let records = facts
            .as_value()
            .get("records")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                ContentError::new(format!(
                    "the listing script for `{}` did not answer with a `records` array; \
                     {CONTRACT} has no other way to spell a listing",
                    self.id
                ))
            })?;
        records
            .iter()
            .map(|held| {
                let id = held.get("id").and_then(Value::as_str).ok_or_else(|| {
                    ContentError::new(format!(
                        "a record in `{}`'s listing has no `id`, so nothing could be bound \
                         to it even after it was listed",
                        self.id
                    ))
                })?;
                let Fetched { version, bytes } = body(held)?;
                Ok(Record {
                    reference: Ref::new(self.id.as_str(), id),
                    version,
                    bytes,
                })
            })
            .collect()
    }
}

fn body(held: &Value) -> Result<Fetched, ContentError> {
    let text = held.get("text").and_then(Value::as_str).ok_or_else(|| {
        ContentError::new(format!(
            "a record answered by a declared provider has no `text`; {CONTRACT} carries a \
             memory as one JSON string, so a store whose records are not text needs a \
             provider compiled into this binary"
        ))
    })?;
    let bytes = text.as_bytes().to_vec();
    Ok(Fetched {
        version: Version::new(content_hash_of_bytes(&bytes).into_inner()),
        bytes,
    })
}

fn refused(e: ProbeError) -> ContentError {
    match e.code {
        ProbeErrorCode::TimedOut => ContentError::spent(e.message),
        _ => ContentError::new(e.message),
    }
}
