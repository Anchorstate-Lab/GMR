use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use gmr_budget::Spent;
use gmr_core::{
    Derivation, Kind, Outcome, ProbeName, ProbeVersion, ReasonClass, Verifiability,
    content_hash_of_bytes,
};
use gmr_probe::{ProbeCall, ProbeError, ProbeErrorCode, Transport};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use crate::select::VALUE;

pub const SCHEMA: &str = "gmr.probe-file.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Shaped {
    Json,
    Toml,
    #[serde(alias = "yml")]
    Yaml,
}

impl Shaped {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Toml => "toml",
            Self::Yaml => "yaml",
        }
    }

    pub fn of_extension(ext: &str) -> Option<Self> {
        match ext {
            "json" => Some(Self::Json),
            "toml" => Some(Self::Toml),
            "yaml" | "yml" => Some(Self::Yaml),
            _ => None,
        }
    }

    fn parse(self, text: &str) -> Result<Value, String> {
        match self {
            Self::Json => serde_json::from_str(text).map_err(|e| e.to_string()),
            Self::Toml => toml::from_str(text).map_err(|e| e.to_string()),
            Self::Yaml => serde_yaml_ng::from_str(text).map_err(|e| e.to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ask {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub select: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shaped: Option<Shaped>,
}

impl Ask {
    pub fn at(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            select: None,
            shaped: None,
        }
    }

    pub fn selecting(mut self, select: impl Into<String>) -> Self {
        self.select = Some(select.into());
        self
    }

    pub fn shaped_as(mut self, shaped: Shaped) -> Self {
        self.shaped = Some(shaped);
        self
    }

    pub fn reading(&self) -> Option<Shaped> {
        self.shaped
            .or_else(|| Shaped::of_extension(Path::new(&self.path).extension()?.to_str()?))
    }

    pub fn version(&self) -> ProbeVersion {
        let mut acc = Vec::new();
        acc.extend_from_slice(self.path.as_bytes());
        acc.push(0);
        acc.extend_from_slice(self.select.as_deref().unwrap_or("").as_bytes());
        acc.push(0);
        acc.extend_from_slice(self.reading().map(Shaped::as_str).unwrap_or("").as_bytes());
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

pub struct Files {
    kind: Kind,
    root: PathBuf,
    asks: Arc<dyn Asks>,
}

impl Files {
    pub fn new(root: impl Into<PathBuf>, asks: impl Asks + 'static) -> Self {
        Self {
            kind: Kind::new("file"),
            root: root.into(),
            asks: Arc::new(asks),
        }
    }
}

pub fn inside(root: &Path, declared: &str) -> Option<PathBuf> {
    let path = Path::new(declared);
    if path.is_absolute() {
        return None;
    }
    let mut depth = 0i32;
    for part in path.components() {
        match part {
            Component::ParentDir => depth -= 1,
            Component::Normal(_) => depth += 1,
            Component::CurDir => {}
            Component::RootDir | Component::Prefix(_) => return None,
        }
        if depth < 0 {
            return None;
        }
    }
    Some(root.join(path))
}

#[async_trait]
impl Transport for Files {
    fn kind(&self) -> &Kind {
        &self.kind
    }

    fn resolve(&self, name: &ProbeName) -> Option<Derivation> {
        Some(Derivation {
            version: self.asks.ask(name)?.version(),
            verifiability: Verifiability::Closed,
        })
    }

    async fn invoke(&self, call: &ProbeCall<'_>) -> Result<Outcome, ProbeError> {
        let name = &call.probe.name;
        let ask = self.asks.ask(name).ok_or_else(|| {
            ProbeError::with_code(
                ReasonClass::Unusable,
                ProbeErrorCode::ArtifactInvalid,
                format!("no file is declared for the probe named `{name}`"),
            )
        })?;

        let declared = crate::template::path(&ask.path, call.position)?;
        let at = inside(&self.root, &declared).ok_or_else(|| {
            ProbeError::with_code(
                ReasonClass::Unusable,
                ProbeErrorCode::ArtifactInvalid,
                format!(
                    "`{declared}` leaves the repository. A declaration is reviewed, but a \
                     probe that can read outside the tree can put anything on the host into \
                     an append-only log; say what you mean with a path inside it"
                ),
            )
        })?;

        let shaped = ask.reading().ok_or_else(|| {
            ProbeError::with_code(
                ReasonClass::Unusable,
                ProbeErrorCode::ArtifactInvalid,
                format!(
                    "cannot tell how to read `{}`; name it with `shaped` (json · toml · yaml)",
                    ask.path
                ),
            )
        })?;

        if call.budget.remaining().is_none() {
            return Err(ProbeError::spent(Spent::Deadline, call.budget));
        }

        let text = match std::fs::read_to_string(&at) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Outcome::NotFound),
            Err(e) => {
                return Err(ProbeError::unreachable(format!(
                    "cannot read `{declared}`: {e}"
                )));
            }
        };

        if text.len() > call.budget.output_cap() {
            return Err(ProbeError::too_large(text.len(), call.budget.output_cap()));
        }

        let body = shaped.parse(&text).map_err(|e| {
            ProbeError::unusable(format!(
                "`{declared}` is not readable as {}: {e}",
                shaped.as_str()
            ))
        })?;

        Ok(crate::select::pick(&body, ask.select.as_deref()))
    }
}
