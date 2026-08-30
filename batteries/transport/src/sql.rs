use std::collections::BTreeMap;
use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use gmr_budget::Spent;
use gmr_core::{
    Derivation, Kind, Observes, Openness, Outcome, ProbeName, ProbeVersion, ReasonClass,
    Verifiability, content_hash_of_bytes,
};
use gmr_probe::{ProbeCall, ProbeError, ProbeErrorCode, Transport};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sqlx::query::Query;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions, SqliteRow};
use sqlx::{Column, Database, Encode, Row, Type, TypeInfo, ValueRef};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spoken {
    Sqlite,
    Postgres,
}

impl Spoken {
    pub fn local(self) -> bool {
        matches!(self, Self::Sqlite)
    }
}

pub fn spoken(url: &str) -> Option<Spoken> {
    let scheme = url.split_once("://").map(|(s, _)| s).unwrap_or("");
    match scheme {
        "" => Some(Spoken::Sqlite),
        s if s.eq_ignore_ascii_case("sqlite") => Some(Spoken::Sqlite),
        s if s.eq_ignore_ascii_case("postgres") || s.eq_ignore_ascii_case("postgresql") => {
            Some(Spoken::Postgres)
        }
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ask {
    pub source: Source,
    pub query: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub binds: Vec<String>,
}

impl Ask {
    pub fn on(source: Source, query: impl Into<String>) -> Self {
        Self {
            source,
            query: query.into(),
            column: None,
            binds: Vec::new(),
        }
    }

    pub fn taking(mut self, column: impl Into<String>) -> Self {
        self.column = Some(column.into());
        self
    }

    pub fn binding(mut self, name: impl Into<String>) -> Self {
        self.binds.push(name.into());
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
        for bound in &self.binds {
            acc.push(0);
            acc.extend_from_slice(bound.as_bytes());
        }
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

#[cfg_attr(not(feature = "postgres"), allow(dead_code))]
fn unreadable(kind: &str, column: &str) -> ProbeError {
    ProbeError::unusable(format!(
        "the column `{column}` comes back as `{kind}`, and this build has no reading for \
         that. A probe reports one fact and inventing a shape for a type it cannot decode \
         would be reporting something the database did not say -- ask for one it can, with \
         a cast in the query"
    ))
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

#[allow(clippy::type_complexity)]
fn bound<'q, DB: Database>(
    query: Query<'q, DB, <DB as Database>::Arguments<'q>>,
    name: &str,
    position: &Value,
    ask: &Ask,
) -> Result<Query<'q, DB, <DB as Database>::Arguments<'q>>, ProbeError>
where
    String: Encode<'q, DB> + Type<DB>,
    i64: Encode<'q, DB> + Type<DB>,
    f64: Encode<'q, DB> + Type<DB>,
    bool: Encode<'q, DB> + Type<DB>,
    Option<String>: Encode<'q, DB> + Type<DB>,
{
    let refuse = |what: String| {
        ProbeError::with_code(
            ReasonClass::Unusable,
            ProbeErrorCode::ArtifactInvalid,
            format!(
                "the query binds `{name}`, and the position {what}: {position}. A bound name \
                 is how the query says which part of a coordinate it is asking about, so one \
                 the position cannot fill means the declaration and the anchor disagree about \
                 what is being watched"
            ),
        )
    };
    let Some(held) = position.get(name) else {
        return Err(refuse(format!("carries no such field for `{}`", ask.query)));
    };
    match held {
        Value::String(s) => Ok(query.bind(s.clone())),
        Value::Bool(b) => Ok(query.bind(*b)),
        Value::Null => Ok(query.bind(None::<String>)),
        Value::Number(n) => match (n.as_i64(), n.as_f64()) {
            (Some(i), _) => Ok(query.bind(i)),
            (None, Some(f)) => Ok(query.bind(f)),
            (None, None) => Err(refuse(
                "holds a number no database column can hold".to_owned(),
            )),
        },
        _ => Err(refuse(
            "holds a list or an object there, and a bound value is one value".to_owned(),
        )),
    }
}

type Cell<R> = fn(&R, usize) -> Result<Value, ProbeError>;

fn shaped<R: Row>(row: &R, column: Option<&str>, cell: Cell<R>) -> Result<Value, ProbeError> {
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
        return cell(row, at);
    }
    match row.columns().len() {
        1 => cell(row, 0),
        _ => Ok(Value::Object(
            row.columns()
                .iter()
                .enumerate()
                .map(|(at, c)| Ok((c.name().to_owned(), cell(row, at)?)))
                .collect::<Result<Map<_, _>, ProbeError>>()?,
        )),
    }
}

fn from_sqlite(row: &SqliteRow, at: usize) -> Result<Value, ProbeError> {
    Ok(cell(row, at))
}

#[async_trait]
impl Transport for Sql {
    fn kind(&self) -> &Kind {
        &self.kind
    }

    fn resolve(&self, name: &ProbeName) -> Option<Derivation> {
        let ask = self.asks.ask(name)?;
        let local = match &ask.source {
            Source::Given(url) => spoken(url).is_some_and(Spoken::local),
            Source::FromEnv(_) => false,
        };
        Some(Derivation {
            version: ask.version(),
            observes: Observes::named([crate::select::VALUE]),
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
        let rows = match spoken(&url) {
            Some(Spoken::Sqlite) => sqlite(&ask, &url, name, call, left).await?,
            Some(Spoken::Postgres) => postgres(&ask, &url, name, call, left).await?,
            None => {
                return Err(ProbeError::with_code(
                    ReasonClass::Unusable,
                    ProbeErrorCode::ArtifactInvalid,
                    format!(
                        "the database `{name}` names is a `{}` one, and nothing here speaks \
                         that. A scheme this build does not know is a declaration to fix and \
                         never an outage to retry",
                        url.split_once("://").map(|(s, _)| s).unwrap_or("(none)")
                    ),
                ));
            }
        };

        match rows.len() {
            0 => Ok(Outcome::NotFound),
            1 => {
                let value = rows.into_iter().next().unwrap_or(Value::Null);
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

async fn sqlite(
    ask: &Ask,
    url: &str,
    name: &ProbeName,
    call: &ProbeCall<'_>,
    left: std::time::Duration,
) -> Result<Vec<Value>, ProbeError> {
    let options = SqliteConnectOptions::from_str(url)
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

    let mut asked = sqlx::query(&ask.query);
    for bind in &ask.binds {
        asked = bound(asked, bind, call.position, ask)?;
    }
    let rows = tokio::time::timeout(left, asked.fetch_all(&pool))
        .await
        .map_err(|_| ProbeError::spent(Spent::Deadline, call.budget))?
        .map_err(|e| {
            ProbeError::unusable(format!(
                "the database `{name}` names refused the query: {e}"
            ))
        })?;
    pool.close().await;

    rows.iter()
        .map(|row| shaped(row, ask.column.as_deref(), from_sqlite))
        .collect()
}

#[cfg(not(feature = "postgres"))]
async fn postgres(
    _ask: &Ask,
    _url: &str,
    name: &ProbeName,
    _call: &ProbeCall<'_>,
    _left: std::time::Duration,
) -> Result<Vec<Value>, ProbeError> {
    Err(ProbeError::with_code(
        ReasonClass::Unusable,
        ProbeErrorCode::ArtifactInvalid,
        format!(
            "the database `{name}` names is a postgres one, and this binary was built \
             without that feature. It is a build to change and never an outage to retry"
        ),
    ))
}

#[cfg(feature = "postgres")]
async fn postgres(
    ask: &Ask,
    url: &str,
    name: &ProbeName,
    call: &ProbeCall<'_>,
    left: std::time::Duration,
) -> Result<Vec<Value>, ProbeError> {
    let options = sqlx::postgres::PgConnectOptions::from_str(url).map_err(|e| {
        ProbeError::with_code(
            ReasonClass::Unusable,
            ProbeErrorCode::ArtifactInvalid,
            format!(
                "the database `{name}` names is not a usable postgres url: {}",
                ask.source.tellable(e)
            ),
        )
    })?;

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(left)
        .after_connect(|conn, _| {
            Box::pin(async move {
                use sqlx::Executor;
                conn.execute(sqlx::raw_sql(
                    "SET SESSION CHARACTERISTICS AS TRANSACTION READ ONLY",
                ))
                .await?;
                Ok(())
            })
        })
        .connect_with(options)
        .await
        .map_err(|e| {
            ProbeError::unreachable(format!(
                "cannot reach the database `{name}` names: {}",
                ask.source.tellable(e)
            ))
        })?;

    let mut asked = sqlx::query(&ask.query);
    for bind in &ask.binds {
        asked = bound(asked, bind, call.position, ask)?;
    }
    let rows = tokio::time::timeout(left, asked.fetch_all(&pool))
        .await
        .map_err(|_| ProbeError::spent(Spent::Deadline, call.budget))?
        .map_err(|e| {
            ProbeError::unusable(format!(
                "the database `{name}` names refused the query: {e}"
            ))
        })?;
    pool.close().await;

    rows.iter()
        .map(|row| shaped(row, ask.column.as_deref(), from_postgres))
        .collect()
}

#[cfg(feature = "postgres")]
fn from_postgres(row: &sqlx::postgres::PgRow, at: usize) -> Result<Value, ProbeError> {
    let Ok(raw) = row.try_get_raw(at) else {
        return Ok(Value::Null);
    };
    if raw.is_null() {
        return Ok(Value::Null);
    }
    let kind = raw.type_info().name().to_owned();
    let column = row.column(at).name().to_owned();
    let taken = |v: Option<Value>| v.ok_or_else(|| unreadable(&kind, &column));
    match kind.as_str() {
        "BOOL" => taken(row.try_get::<bool, _>(at).ok().map(Value::from)),
        "INT2" => taken(row.try_get::<i16, _>(at).ok().map(Value::from)),
        "INT4" => taken(row.try_get::<i32, _>(at).ok().map(Value::from)),
        "INT8" => taken(row.try_get::<i64, _>(at).ok().map(Value::from)),
        "FLOAT4" => taken(
            row.try_get::<f32, _>(at)
                .ok()
                .and_then(|v| serde_json::Number::from_f64(v as f64))
                .map(Value::Number),
        ),
        "FLOAT8" => taken(
            row.try_get::<f64, _>(at)
                .ok()
                .and_then(serde_json::Number::from_f64)
                .map(Value::Number),
        ),
        "TEXT" | "VARCHAR" | "BPCHAR" | "NAME" | "UNKNOWN" => {
            taken(row.try_get::<String, _>(at).ok().map(Value::from))
        }
        _ => Err(unreadable(&kind, &column)),
    }
}
