mod http;

#[cfg(feature = "testkit")]
pub mod testkit;

use async_trait::async_trait;
use gmr_content::{
    ContentError, ContentProvider, Fetched, History, MemorySource, MemoryStore, Record,
};
use gmr_core::{ExternalId, ProviderId, Ref, Version, content_hash_of_bytes};
use gmr_probe::Budget;
use serde::Deserialize;

use http::{Answer, Credential, Http};

pub const DEFAULT_BASE: &str = "https://api.mem0.ai";

const SELF_HOSTED_CEILING: usize = 1000;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Deployment {
    Platform,
    SelfHosted,
}

enum Absence {
    NotSaid,
    Certain,
    Unconfirmed { probe: String },
}

impl Deployment {
    fn memory(&self, base: &str, id: &ExternalId) -> String {
        match self {
            Self::Platform => format!("{base}/v1/memories/{id}/"),
            Self::SelfHosted => format!("{base}/memories/{id}"),
        }
    }

    fn history(&self, base: &str, id: &ExternalId) -> String {
        match self {
            Self::Platform => format!("{base}/v1/memories/{id}/history/"),
            Self::SelfHosted => format!("{base}/memories/{id}/history"),
        }
    }

    fn listing(&self, base: &str, scope: &str) -> String {
        match self {
            Self::Platform => format!("{base}/v1/memories/?{scope}"),
            Self::SelfHosted => format!("{base}/memories?{scope}&top_k={SELF_HOSTED_CEILING}"),
        }
    }

    fn credential(&self, key: String) -> Credential {
        match self {
            Self::Platform => Credential {
                header: "Authorization",
                value: format!("Token {key}"),
            },
            Self::SelfHosted => Credential {
                header: "X-API-Key",
                value: key,
            },
        }
    }

    fn absence(&self, answer: &Answer, base: &str, scope: &str) -> Absence {
        match self {
            Self::Platform if answer.status == 404 => Absence::Unconfirmed {
                probe: format!("{}&page_size=1", self.listing(base, scope)),
            },
            Self::SelfHosted if answer.status == 200 && answer.body.trim() == "null" => {
                Absence::Certain
            }
            _ => Absence::NotSaid,
        }
    }

    fn reads_as_no_history(&self, answer: &Answer) -> bool {
        match self {
            Self::Platform => answer.status == 404,
            Self::SelfHosted => false,
        }
    }

    fn whole(&self, listed: Vec<Record>) -> Result<Vec<Record>, ContentError> {
        match self {
            Self::Platform => Ok(listed),
            Self::SelfHosted if listed.len() < SELF_HOSTED_CEILING => Ok(listed),
            Self::SelfHosted => Err(ContentError::new(format!(
                "this scope holds at least {SELF_HOSTED_CEILING} memories, which is the most a \
                 self-hosted mem0 returns, and its listing route carries no cursor and no total \
                 — so a complete listing and a truncated one arrive identical. Narrow the scope \
                 rather than trust a count that sits exactly on the ceiling"
            ))),
        }
    }
}

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

    fn names_nothing(&self) -> bool {
        self.user_id.is_none() && self.agent_id.is_none() && self.app_id.is_none()
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
    deployment: Deployment,
    scope: Scope,
    http: Box<dyn Http>,
}

impl Mem0 {
    pub fn platform(api_key: impl Into<String>, scope: Scope) -> Result<Self, ContentError> {
        Self::assembled(
            Deployment::Platform,
            DEFAULT_BASE.to_owned(),
            Some(api_key.into()),
            scope,
        )
    }

    pub fn self_hosted(
        base: impl Into<String>,
        api_key: Option<String>,
        scope: Scope,
    ) -> Result<Self, ContentError> {
        if scope.app_id.is_some() {
            return Err(ContentError::new(
                "a self-hosted mem0 filters on user_id, agent_id and run_id, and silently \
                 ignores app_id. A scope named only by app_id therefore names nothing, and its \
                 listing route answers a scope that names nothing with every memory in the \
                 store — everybody's, not yours. Name a user_id or an agent_id instead",
            ));
        }
        Self::assembled(Deployment::SelfHosted, base.into(), api_key, scope)
    }

    fn assembled(
        deployment: Deployment,
        base: String,
        api_key: Option<String>,
        scope: Scope,
    ) -> Result<Self, ContentError> {
        if scope.names_nothing() {
            return Err(ContentError::new(
                "this mem0 provider was given no user_id, agent_id or app_id, so it never says \
                 whose memories it reads. A scope that names nothing is not a wider listing of \
                 yours, it is somebody else's",
            ));
        }
        Ok(Self {
            id: ProviderId::new("mem0"),
            base,
            deployment,
            http: Box::new(http::Reqwest::new(
                api_key.map(|key| deployment.credential(key)),
            )?),
            scope,
        })
    }

    pub fn named(mut self, id: impl Into<String>) -> Self {
        self.id = ProviderId::new(id);
        self
    }

    pub fn store(self) -> MemoryStore {
        let shared = std::sync::Arc::new(self);
        MemoryStore::new(shared.clone()).listing(shared)
    }

    #[cfg(any(test, feature = "testkit"))]
    fn faked(http: Box<dyn Http>, scope: Scope, deployment: Deployment) -> Self {
        Self {
            id: ProviderId::new("mem0"),
            base: "https://mem0.test".to_owned(),
            deployment,
            scope,
            http,
        }
    }

    async fn absent(
        &self,
        id: &ExternalId,
        probe: &str,
        budget: &Budget,
    ) -> Result<Option<Fetched>, ContentError> {
        match self.http.get(probe, budget).await?.status == 200 {
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
    memory: String,
}

#[derive(Debug, Deserialize)]
struct Page {
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
        let url = self.deployment.memory(&self.base, id);
        let answer = self.http.get(&url, budget).await?;
        match self
            .deployment
            .absence(&answer, &self.base, &self.scope.query())
        {
            Absence::Certain => return Ok(None),
            Absence::Unconfirmed { probe } => return self.absent(id, &probe, budget).await,
            Absence::NotSaid => {}
        }
        match answer.status {
            200 => {
                let memory: Memory = parse(&answer.body, "memory")?;
                Ok(Some(Fetched {
                    version: version_of(&memory.memory),
                    bytes: memory.memory.into_bytes(),
                }))
            }
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
        let url = self.deployment.history(&self.base, id);
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
            _ if self.deployment.reads_as_no_history(&answer) => Ok(None),
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
        let mut url = self.deployment.listing(&self.base, &self.scope.query());
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
            }));
            match page.next {
                Some(next) => url = next,
                None => return self.deployment.whole(out),
            }
        }
    }
}

#[cfg(test)]
mod tests;
