use std::collections::BTreeMap;
use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use gmr_budget::Spent;
use gmr_core::{
    Derivation, Kind, Openness, Outcome, ProbeName, ProbeVersion, ReasonClass, Verifiability,
    content_hash_of_bytes,
};
use gmr_probe::{ProbeCall, ProbeError, ProbeErrorCode, Transport};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions, SqliteRow};
use sqlx::{Column, Row, TypeInfo, ValueRef};

pub use crate::select::VALUE;

pub const SCHEMA: &str = "gmr.probe-sql.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Given(String),
    FromEnv(String),
}

impl Source {
    fn resolve(&self) -> Result<String, ProbeError> {
        match self {
            Self::Given(url) => {
                crate::given::without_credentials(url)?;
                Ok(url.clone())
            }
            Self::FromEnv(var) => std::env::var(var).map_err(|_| {
                ProbeError::with_code(
                    ReasonClass::Unusable,
                    ProbeErrorCode::ArtifactInvalid,
                    format!(
                        "the database is declared to come from the environment variable \
                         `{var}`, and it is not set"
                    ),
                )
            }),
        }
    }

    fn named(&self) -> (&'static str, &str) {
        match self {
            Self::Given(url) => ("given", url.as_str()),
            Self::FromEnv(var) => ("from-env", var.as_str()),
        }
    }

    fn tellable(&self, e: impl std::fmt::Display) -> String {
        match self {
            Self::Given(_) => e.to_string(),
            Self::FromEnv(var) => format!(
                "the reason is not repeated here, because it can quote the connection \
                 string. `{var}` is read at the moment of the call so that what it holds \
                 never reaches anything that keeps it"
            ),
        }
    }
}

pub fn sqlite_url(url: &str) -> bool {
    let scheme = url.split_once("://").map(|(s, _)| s).unwrap_or("");
    scheme.is_empty() || scheme.eq_ignore_ascii_case("sqlite")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ask {
    pub source: Source,
    pub query: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<String>,
}

impl Ask {
    pub fn on(source: Source, query: impl Into<String>) -> Self {
        Self {
            source,
            query: query.into(),
            column: None,
        }
    }

    pub fn taking(mut self, column: impl Into<String>) -> Self {
        self.column = Some(column.into());
        self
    }

    pub fn version(&self) -> ProbeVersion {
        let (kind, named) = self.source.named();
        let mut acc = Vec::new();
        acc.extend_from_slice(kind.as_bytes());
        acc.push(0);
        acc.extend_from_slice(named.as_bytes());
        acc.push(0);
        acc.extend_from_slice(self.query.as_bytes());
        acc.push(0);
        acc.extend_from_slice(self.column.as_deref().unwrap_or("").as_bytes());
        ProbeVersion::of(content_hash_of_bytes(&acc))
    }
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

pub struct Sql {
    kind: Kind,
    asks: Arc<dyn Asks>,
}

impl Sql {
    pub fn new(asks: impl Asks + 'static) -> Self {
        Self {
            kind: Kind::new("sql"),
            asks: Arc::new(asks),
        }
    }
}

pub fn cell(row: &SqliteRow, at: usize) -> Value {
    let raw = row.try_get_raw(at).ok();
    let Some(raw) = raw else {
        return Value::Null;
    };
    if raw.is_null() {
        return Value::Null;
    }
    match raw.type_info().name() {
        "INTEGER" => row
            .try_get::<i64, _>(at)
            .map(Value::from)
            .unwrap_or(Value::Null),
        "REAL" => row
            .try_get::<f64, _>(at)
            .ok()
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        "BLOB" => Value::Null,
        _ => row
            .try_get::<String, _>(at)
            .map(Value::from)
            .unwrap_or(Value::Null),
    }
}

fn shaped(row: &SqliteRow, column: Option<&str>) -> Result<Value, ProbeError> {
    if let Some(want) = column {
        let at = row
            .columns()
            .iter()
            .position(|c| c.name() == want)
            .ok_or_else(|| {
                ProbeError::unusable(format!(
                    "the query returns no column named `{want}`; it returns {}",
                    row.columns()
                        .iter()
                        .map(Column::name)
                        .collect::<Vec<_>>()
                        .join(" · ")
                ))
            })?;
        return Ok(cell(row, at));
    }
    match row.columns().len() {
        1 => Ok(cell(row, 0)),
        _ => Ok(Value::Object(
            row.columns()
                .iter()
                .enumerate()
                .map(|(at, c)| (c.name().to_owned(), cell(row, at)))
                .collect::<Map<_, _>>(),
        )),
    }
}

#[async_trait]
impl Transport for Sql {
    fn kind(&self) -> &Kind {
        &self.kind
    }

    fn resolve(&self, name: &ProbeName) -> Option<Derivation> {
        let ask = self.asks.ask(name)?;
        let local = match &ask.source {
            Source::Given(url) => sqlite_url(url),
            Source::FromEnv(_) => false,
        };
        Some(Derivation {
            version: ask.version(),
            verifiability: match local {
                true => Verifiability::Closed,
                false => Verifiability::open([Openness::Network, Openness::Clock]),
            },
        })
    }

    async fn invoke(&self, call: &ProbeCall<'_>) -> Result<Outcome, ProbeError> {
        let name = &call.probe.name;
        let ask = self.asks.ask(name).ok_or_else(|| {
            ProbeError::with_code(
                ReasonClass::Unusable,
                ProbeErrorCode::ArtifactInvalid,
                format!("no query is declared for the probe named `{name}`"),
            )
        })?;

        let left = call
            .budget
            .remaining()
            .ok_or_else(|| ProbeError::spent(Spent::Deadline, call.budget))?;

        let url = ask.source.resolve().map_err(|e| e.about(name))?;
        if !sqlite_url(&url) {
            return Err(ProbeError::with_code(
                ReasonClass::Unusable,
                ProbeErrorCode::ArtifactInvalid,
                format!(
                    "this build speaks sqlite, and the database `{name}` names is a `{}` \
                     one. Another backend is a feature this binary was not built with, \
                     which is a declaration to fix and never an outage to retry",
                    url.split_once("://").map(|(s, _)| s).unwrap_or("(none)")
                ),
            ));
        }
        let options = SqliteConnectOptions::from_str(&url)
            .map_err(|e| {
                ProbeError::with_code(
                    ReasonClass::Unusable,
                    ProbeErrorCode::ArtifactInvalid,
                    format!(
                        "the database `{name}` names is not a usable sqlite url: {}",
                        ask.source.tellable(e)
                    ),
                )
            })?
            .read_only(true)
            .create_if_missing(false);

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .acquire_timeout(left)
            .connect_with(options)
            .await
            .map_err(|e| {
                ProbeError::unreachable(format!(
                    "cannot open the database `{name}` names: {}",
                    ask.source.tellable(e)
                ))
            })?;

        let rows = tokio::time::timeout(left, sqlx::query(&ask.query).fetch_all(&pool))
            .await
            .map_err(|_| ProbeError::spent(Spent::Deadline, call.budget))?
            .map_err(|e| {
                ProbeError::unusable(format!(
                    "the database `{name}` names refused the query: {e}"
                ))
            })?;
        pool.close().await;

        match rows.len() {
            0 => Ok(Outcome::NotFound),
            1 => {
                let value = shaped(&rows[0], ask.column.as_deref())?;
                let size = serde_json::to_string(&value).map(|s| s.len()).unwrap_or(0);
                if size > call.budget.output_cap() {
                    return Err(ProbeError::too_large(size, call.budget.output_cap()));
                }
                Ok(crate::select::held(value))
            }
            many => Err(ProbeError::unusable(format!(
                "`{name}` answered with {many} rows; a probe reports one fact, so say which \
                 one the declaration means"
            ))),
        }
    }
}
