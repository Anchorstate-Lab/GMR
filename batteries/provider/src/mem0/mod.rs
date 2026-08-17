//! mem0 as a content provider, a history and a discovery source.
//!
//! The seam this module talks through offers `get` and nothing else, so
//! "GMR never writes into mem0" is not a rule anyone has to keep — there is
//! no method here that could.

mod http;

use async_trait::async_trait;
use gmr_content::{Claim, ContentError, ContentProvider, Fetched, History, MemorySource, Record};
use gmr_core::{ExternalId, ProviderId, Ref, Version, content_hash_of_bytes};
use gmr_probe::Budget;
use serde::Deserialize;

use http::{Answer, Http};

pub const DEFAULT_BASE: &str = "https://api.mem0.ai";

pub struct Scope {
    pub user_id: Option<String>,
    pub agent_id: Option<String>,
    pub app_id: Option<String>,
}

impl Scope {
    pub fn user(id: impl Into<String>) -> Self {
        Self {
            user_id: Some(id.into()),
            agent_id: None,
            app_id: None,
        }
    }

    fn query(&self) -> String {
        [
            ("user_id", &self.user_id),
            ("agent_id", &self.agent_id),
            ("app_id", &self.app_id),
        ]
        .iter()
        .filter_map(|(k, v)| v.as_ref().map(|v| format!("{k}={v}")))
        .collect::<Vec<_>>()
        .join("&")
    }
}

pub struct Mem0 {
    id: ProviderId,
    base: String,
    scope: Scope,
    http: Box<dyn Http>,
}

impl Mem0 {
    pub fn new(api_key: impl Into<String>, scope: Scope) -> Result<Self, ContentError> {
        Ok(Self {
            id: ProviderId::new("mem0"),
            base: DEFAULT_BASE.to_owned(),
            scope,
            http: Box::new(http::Reqwest::new(api_key.into())?),
        })
    }

    pub fn based_at(mut self, base: impl Into<String>) -> Self {
        self.base = base.into();
        self
    }

    pub fn named(mut self, id: impl Into<String>) -> Self {
        self.id = ProviderId::new(id);
        self
    }

    #[cfg(test)]
    fn faked(http: Box<dyn Http>, scope: Scope) -> Self {
        Self {
            id: ProviderId::new("mem0"),
            base: "https://mem0.test".to_owned(),
            scope,
            http,
        }
    }

    async fn scope_is_live(&self, budget: &Budget) -> Result<bool, ContentError> {
        let url = format!(
            "{}/v1/memories/?{}&page_size=1",
            self.base,
            self.scope.query()
        );
        Ok(self.http.get(&url, budget).await?.status == 200)
    }

    async fn absent(
        &self,
        id: &ExternalId,
        budget: &Budget,
    ) -> Result<Option<Fetched>, ContentError> {
        match self.scope_is_live(budget).await? {
            true => Ok(None),
            false => Err(ContentError::new(format!(
                "mem0 has no `{id}` for this key and scope, and asking it to list that scope \
                 failed too — so this is a credentials or scope problem, not a record that \
                 was deleted. Reporting it as gone would send you to delete a binding that \
                 is fine"
            ))),
        }
    }
}

fn refused(answer: &Answer, what: &str) -> ContentError {
    let body = answer.body.chars().take(200).collect::<String>();
    ContentError::new(format!(
        "mem0 answered {} for {what}: {body}",
        answer.status
    ))
}

#[derive(Debug, Deserialize)]
struct Memory {
    id: String,
    #[serde(default)]
    memory: String,
}

#[derive(Debug, Deserialize)]
struct Page {
    #[serde(default)]
    results: Vec<Memory>,
    #[serde(default)]
    next: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Change {
    #[serde(default)]
    new_memory: Option<String>,
}

fn parse<T: serde::de::DeserializeOwned>(body: &str, what: &str) -> Result<T, ContentError> {
    serde_json::from_str(body)
        .map_err(|e| ContentError::new(format!("mem0's {what} was not the shape we expect: {e}")))
}

fn page_of(body: &str) -> Result<Page, ContentError> {
    match parse::<Vec<Memory>>(body, "listing") {
        Ok(results) => Ok(Page {
            results,
            next: None,
        }),
        Err(_) => parse(body, "listing"),
    }
}

fn version_of(text: &str) -> Version {
    Version::new(content_hash_of_bytes(text.as_bytes()).into_inner())
}

#[async_trait]
impl ContentProvider for Mem0 {
    fn provider(&self) -> &ProviderId {
        &self.id
    }

    async fn fetch(
        &self,
        id: &ExternalId,
        budget: &Budget,
    ) -> Result<Option<Fetched>, ContentError> {
        let url = format!("{}/v1/memories/{id}/", self.base);
        let answer = self.http.get(&url, budget).await?;
        match answer.status {
            200 => {
                let memory: Memory = parse(&answer.body, "memory")?;
                Ok(Some(Fetched {
                    version: version_of(&memory.memory),
                    bytes: memory.memory.into_bytes(),
                }))
            }
            404 => self.absent(id, budget).await,
            _ => Err(refused(&answer, "a memory")),
        }
    }

    fn history(&self) -> Option<&dyn History> {
        Some(self)
    }
}

#[async_trait]
impl History for Mem0 {
    async fn fetch_at(
        &self,
        id: &ExternalId,
        version: &Version,
        budget: &Budget,
    ) -> Result<Option<Vec<u8>>, ContentError> {
        let url = format!("{}/v1/memories/{id}/history/", self.base);
        let answer = self.http.get(&url, budget).await?;
        match answer.status {
            200 => {
                let changes: Vec<Change> = parse(&answer.body, "history")?;
                Ok(changes
                    .into_iter()
                    .filter_map(|c| c.new_memory)
                    .find(|text| &version_of(text) == version)
                    .map(String::into_bytes))
            }
            404 => Ok(None),
            _ => Err(refused(&answer, "a memory's history")),
        }
    }
}

#[async_trait]
impl MemorySource for Mem0 {
    fn provider(&self) -> &ProviderId {
        &self.id
    }

    async fn list(&self, budget: &Budget) -> Result<Vec<Record>, ContentError> {
        let mut url = format!("{}/v1/memories/?{}", self.base, self.scope.query());
        let mut out = Vec::new();
        loop {
            if budget.remaining().is_none() {
                return Err(ContentError::spent(format!(
                    "the budget ran out after {} record(s); a partial listing is not a \
                     listing, and treating one as complete would read as records having \
                     disappeared",
                    out.len()
                )));
            }
            let answer = self.http.get(&url, budget).await?;
            if answer.status != 200 {
                return Err(refused(&answer, "a listing"));
            }
            let page = page_of(&answer.body)?;
            out.extend(page.results.into_iter().map(|m| Record {
                reference: Ref::new(self.id.as_str(), m.id),
                version: version_of(&m.memory),
                bytes: m.memory.into_bytes(),
                claim: Claim::Silent,
            }));
            match page.next {
                Some(next) => url = next,
                None => return Ok(out),
            }
        }
    }
}

#[cfg(test)]
mod tests;
