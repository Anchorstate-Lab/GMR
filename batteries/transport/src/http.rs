use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use gmr_budget::{Budget, Spent};
use gmr_core::{
    Derivation, Kind, Openness, Outcome, ProbeName, ProbeVersion, ReasonClass, Verifiability,
    content_hash_of_bytes,
};
use gmr_probe::{ProbeCall, ProbeError, ProbeErrorCode, Transport};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use crate::select::VALUE;

pub const SCHEMA: &str = "gmr.probe-http.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Header {
    Given(String),
    FromEnv(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ask {
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub select: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
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

pub trait Asks: Send + Sync {
    fn ask(&self, name: &ProbeName) -> Option<Ask>;
}

impl Asks for BTreeMap<ProbeName, Ask> {
    fn ask(&self, name: &ProbeName) -> Option<Ask> {
        self.get(name).cloned()
    }
}

impl<T: Asks + ?Sized> Asks for Arc<T> {
    fn ask(&self, name: &ProbeName) -> Option<Ask> {
        (**self).ask(name)
    }
}

pub struct Http {
    kind: Kind,
    asks: Arc<dyn Asks>,
    fetch: Arc<dyn Fetch>,
}

impl Http {
    pub fn new(asks: impl Asks + 'static) -> Result<Self, ProbeError> {
        Ok(Self::with(asks, Arc::new(Reqwest::new()?)))
    }

    pub fn with(asks: impl Asks + 'static, fetch: Arc<dyn Fetch>) -> Self {
        Self {
            kind: Kind::new("http"),
            asks: Arc::new(asks),
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
            version: self.asks.ask(name)?.version(),
            verifiability: Verifiability::open([Openness::Network, Openness::Clock]),
        })
    }

    async fn invoke(&self, call: &ProbeCall<'_>) -> Result<Outcome, ProbeError> {
        let name = &call.probe.name;
        let ask = self.asks.ask(name).ok_or_else(|| {
            ProbeError::with_code(
                ReasonClass::Unusable,
                ProbeErrorCode::ArtifactInvalid,
                format!("no url is declared for the probe named `{name}`"),
            )
        })?;

        let url = crate::template::url(&ask.url, call.position)?;
        crate::given::without_credentials(&url)?;
        let sent = ask.sent().map_err(|e| e.about(name))?;
        let reply = self
            .fetch
            .get(&url, &sent, call.budget)
            .await
            .map_err(|e| e.about(name))?;
        let ask = &ask;

        match reply.status {
            200..=299 => {}
            404 | 410 => return Ok(Outcome::NotFound),
            401 | 403 => {
                return Err(ProbeError::with_code(
                    ReasonClass::Unusable,
                    ProbeErrorCode::ArtifactInvalid,
                    format!(
                        "the endpoint `{name}` reads refused this request ({}); that is \
                         something about our request -- a credential, or a header it \
                         requires -- and not its answer",
                        reply.status
                    ),
                ));
            }
            500..=599 => {
                return Err(ProbeError::unreachable(format!(
                    "the endpoint `{name}` reads answered {}; the fact is not established \
                     either way",
                    reply.status
                )));
            }
            other => {
                return Err(ProbeError::unusable(format!(
                    "the endpoint `{name}` reads answered {other}, which is neither an \
                     answer nor an outage"
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
                    "the endpoint `{name}` reads did not answer with JSON ({e}); received \
                     prefix: {}",
                    reply.body.chars().take(120).collect::<String>()
                ),
            )
        })?;

        Ok(crate::select::pick(&body, ask.select.as_deref()))
    }
}

pub struct Reqwest {
    client: reqwest::Client,
}

impl Reqwest {
    pub fn new() -> Result<Self, ProbeError> {
        reqwest::Client::builder()
            .user_agent(concat!("gmr/", env!("CARGO_PKG_VERSION")))
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
            false => {
                ProbeError::unreachable(format!("cannot reach the endpoint: {}", e.without_url()))
            }
        })?;
        let status = response.status().as_u16();
        let body = response.text().await.map_err(|e| {
            ProbeError::unreachable(format!(
                "cannot read the endpoint's answer: {}",
                e.without_url()
            ))
        })?;
        Ok(Reply { status, body })
    }
}
