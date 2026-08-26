use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use gmr_budget::{Budget, Spent};
use gmr_core::{
    Derivation, Facts, Kind, Openness, Outcome, ProbeName, ProbeVersion, ReasonClass,
    Verifiability, content_hash_of_bytes,
};
use gmr_probe::{ProbeCall, ProbeError, ProbeErrorCode, Transport};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Header {
    Given(String),
    FromEnv(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ask {
    pub url: String,
    pub select: Option<String>,
    pub headers: BTreeMap<String, Header>,
}

impl Ask {
    pub fn at(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            select: None,
            headers: BTreeMap::new(),
        }
    }

    pub fn selecting(mut self, path: impl Into<String>) -> Self {
        self.select = Some(path.into());
        self
    }

    pub fn with_header(mut self, name: impl Into<String>, value: Header) -> Self {
        self.headers.insert(name.into(), value);
        self
    }

    pub fn version(&self) -> ProbeVersion {
        let mut acc = Vec::new();
        acc.extend_from_slice(self.url.as_bytes());
        acc.push(0);
        acc.extend_from_slice(self.select.as_deref().unwrap_or("").as_bytes());
        acc.push(0);
        for (name, value) in &self.headers {
            acc.extend_from_slice(name.as_bytes());
            acc.push(0);
            match value {
                Header::Given(v) => {
                    acc.extend_from_slice(b"given");
                    acc.push(0);
                    acc.extend_from_slice(v.as_bytes());
                }
                Header::FromEnv(name) => {
                    acc.extend_from_slice(b"from-env");
                    acc.push(0);
                    acc.extend_from_slice(name.as_bytes());
                }
            }
            acc.push(0);
        }
        ProbeVersion::of(content_hash_of_bytes(&acc))
    }

    fn sent(&self) -> Result<Vec<(String, String)>, ProbeError> {
        self.headers
            .iter()
            .map(|(name, value)| match value {
                Header::Given(v) => Ok((name.clone(), v.clone())),
                Header::FromEnv(var) => match std::env::var(var) {
                    Ok(v) => Ok((name.clone(), v)),
                    Err(_) => Err(ProbeError::with_code(
                        ReasonClass::Unusable,
                        ProbeErrorCode::ArtifactInvalid,
                        format!(
                            "the header `{name}` is declared to come from the environment \
                             variable `{var}`, and it is not set"
                        ),
                    )),
                },
            })
            .collect()
    }
}

pub struct Reply {
    pub status: u16,
    pub body: String,
}

#[async_trait]
pub trait Fetch: Send + Sync {
    async fn get(
        &self,
        url: &str,
        headers: &[(String, String)],
        budget: &Budget,
    ) -> Result<Reply, ProbeError>;
}

pub struct Http {
    kind: Kind,
    asks: BTreeMap<ProbeName, Ask>,
    fetch: Arc<dyn Fetch>,
}

impl Http {
    pub fn new(asks: BTreeMap<ProbeName, Ask>) -> Result<Self, ProbeError> {
        Ok(Self::with(asks, Arc::new(Reqwest::new()?)))
    }

    pub fn with(asks: BTreeMap<ProbeName, Ask>, fetch: Arc<dyn Fetch>) -> Self {
        Self {
            kind: Kind::new("http"),
            asks,
            fetch,
        }
    }
}

#[async_trait]
impl Transport for Http {
    fn kind(&self) -> &Kind {
        &self.kind
    }

    fn resolve(&self, name: &ProbeName) -> Option<Derivation> {
        Some(Derivation {
            version: self.asks.get(name)?.version(),
            verifiability: Verifiability::open([Openness::Network, Openness::Clock]),
        })
    }

    async fn invoke(&self, call: &ProbeCall<'_>) -> Result<Outcome, ProbeError> {
        let name = &call.probe.name;
        let ask = self.asks.get(name).ok_or_else(|| {
            ProbeError::with_code(
                ReasonClass::Unusable,
                ProbeErrorCode::ArtifactInvalid,
                format!("no url is declared for the probe named `{name}`"),
            )
        })?;

        let reply = self.fetch.get(&ask.url, &ask.sent()?, call.budget).await?;

        match reply.status {
            200..=299 => {}
            404 | 410 => return Ok(Outcome::NotFound),
            401 | 403 => {
                return Err(ProbeError::with_code(
                    ReasonClass::Unusable,
                    ProbeErrorCode::ArtifactInvalid,
                    format!(
                        "{} refused this request ({}); that is our credentials, not its answer",
                        ask.url, reply.status
                    ),
                ));
            }
            500..=599 => {
                return Err(ProbeError::unreachable(format!(
                    "{} answered {}; the fact is not established either way",
                    ask.url, reply.status
                )));
            }
            other => {
                return Err(ProbeError::unusable(format!(
                    "{} answered {other}, which is neither an answer nor an outage",
                    ask.url
                )));
            }
        }

        if reply.body.len() > call.budget.output_cap() {
            return Err(ProbeError::too_large(
                reply.body.len(),
                call.budget.output_cap(),
            ));
        }

        let body: Value = serde_json::from_str(&reply.body).map_err(|e| {
            ProbeError::with_code(
                ReasonClass::Unusable,
                ProbeErrorCode::InvalidJson,
                format!(
                    "{} did not answer with JSON ({e}); received prefix: {}",
                    ask.url,
                    reply.body.chars().take(120).collect::<String>()
                ),
            )
        })?;

        let Some(select) = ask.select.as_deref() else {
            return Ok(found(body));
        };
        Ok(match body.pointer(&pointer(select)) {
            Some(picked) => found(picked.clone()),
            None => Outcome::NotFound,
        })
    }
}

fn found(value: Value) -> Outcome {
    match value.is_null() {
        true => Outcome::NotFound,
        false => Outcome::Found {
            facts: Facts::new(value),
        },
    }
}

pub fn pointer(select: &str) -> String {
    let path = select.trim_start_matches('$').trim_start_matches('.');
    match path.starts_with('/') {
        true => path.to_owned(),
        false => path
            .split('.')
            .filter(|s| !s.is_empty())
            .map(|s| format!("/{}", s.replace('~', "~0").replace('/', "~1")))
            .collect(),
    }
}

pub struct Reqwest {
    client: reqwest::Client,
}

impl Reqwest {
    pub fn new() -> Result<Self, ProbeError> {
        reqwest::Client::builder()
            .build()
            .map(|client| Self { client })
            .map_err(|e| {
                ProbeError::with_code(
                    ReasonClass::Unusable,
                    ProbeErrorCode::ArtifactInvalid,
                    format!("cannot build an HTTP client: {e}"),
                )
            })
    }
}

#[async_trait]
impl Fetch for Reqwest {
    async fn get(
        &self,
        url: &str,
        headers: &[(String, String)],
        budget: &Budget,
    ) -> Result<Reply, ProbeError> {
        let left = budget
            .remaining()
            .ok_or_else(|| ProbeError::spent(Spent::Deadline, budget))?;
        let mut request = self.client.get(url).timeout(left);
        for (name, value) in headers {
            request = request.header(name, value);
        }
        let response = request.send().await.map_err(|e| match e.is_timeout() {
            true => ProbeError::spent(Spent::Deadline, budget),
            false => ProbeError::unreachable(format!("cannot reach {url}: {e}")),
        })?;
        let status = response.status().as_u16();
        let body = response
            .text()
            .await
            .map_err(|e| ProbeError::unreachable(format!("cannot read {url}'s answer: {e}")))?;
        Ok(Reply { status, body })
    }
}
