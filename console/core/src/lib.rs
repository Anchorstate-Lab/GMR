use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use gmr::{
    AnchorKey, Asked, Binding, Claim, FactAddress, Instructions, LinkKind, LinkRevocation, Policy,
    ProbeName, Runtime, Source, Version,
};
use gmr_transport::recipes::Recipes;
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fault {
    pub kind: &'static str,
    pub message: String,
}

impl Fault {
    pub fn refused(message: impl Into<String>) -> Self {
        Self {
            kind: "refused",
            message: message.into(),
        }
    }

    pub fn assembly(message: impl Into<String>) -> Self {
        Self {
            kind: "assembly",
            message: message.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            kind: "internal",
            message: message.into(),
        }
    }
}

impl std::fmt::Display for Fault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.kind, self.message)
    }
}

pub fn fault(e: gmr::RuntimeError) -> Fault {
    Fault {
        kind: e.code(),
        message: e.to_string(),
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Opening {
    pub root: String,
    #[serde(default)]
    pub db: Option<String>,
    #[serde(default)]
    pub recipes: Recipes,
    #[serde(default)]
    pub scripts: BTreeMap<ProbeName, PathBuf>,
    #[serde(default)]
    pub providers: Providers,
    #[serde(default)]
    pub policy: Policy,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Providers {
    #[serde(default)]
    pub git: bool,
    #[serde(default)]
    pub claude_code: bool,
    #[serde(default)]
    pub mem0: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Asserting {
    #[serde(default)]
    pub bound_version: Option<String>,
    #[serde(default)]
    pub saw: Vec<String>,
    #[serde(default)]
    pub asserts: Option<Value>,
    #[serde(default)]
    pub depends: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Inline {
    claim: String,
    #[serde(default)]
    anchors: Vec<String>,
    #[serde(default)]
    saw: Vec<String>,
    #[serde(default)]
    asserts: Option<Value>,
    #[serde(default)]
    depends: Option<String>,
}

pub async fn opened(asked: Opening) -> Result<Runtime, Fault> {
    let root = PathBuf::from(&asked.root);
    let db = match asked.db {
        Some(at) => PathBuf::from(at),
        None => root.join(".anchor").join("state").join("memory.db"),
    };
    if let Some(parent) = db.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| Fault::assembly(format!("cannot make room for the store at {parent:?}: {e}")))?;
    }

    let store = gmr::sqlite::open(db.clone())
        .await
        .map_err(|e| Fault::assembly(format!("cannot open the store at {}: {e}", db.display())))?;

    let recipes = Arc::new(asked.recipes);
    let probes = root.join(".anchor").join("probes");
    let mut builder = Runtime::builder()
        .policy(asked.policy)
        .journal(Arc::new(store.journal()))
        .bindings(Arc::new(store.bindings()))
        .sealer(Arc::new(store.sealer()))
        .links(Arc::new(store.links()))
        .queue(Arc::new(store.queue()))
        .settings(Arc::new(store.settings()))
        .sightings(Arc::new(store.sightings()))
        .usage(Arc::new(store.usage()))
        .ledger(Arc::new(store.ledger()))
        .transport(Arc::new(gmr_transport::shell::Shell::new(&root, probes)))
        .transport(Arc::new(gmr_transport::script::Script::new(
            &root,
            asked.scripts,
        )))
        .transport(Arc::new(
            gmr_transport::http::Http::new(Arc::clone(&recipes))
                .map_err(|e| Fault::assembly(format!("cannot build the http transport: {e}")))?,
        ))
        .transport(Arc::new(gmr_transport::file::Files::new(
            &root,
            Arc::clone(&recipes),
        )))
        .transport(Arc::new(gmr_transport::sql::Sql::new(Arc::clone(&recipes))));

    for (name, made) in stores(&root, &asked.providers) {
        match made {
            Ok(store) => builder = builder.provider(store.content()),
            Err(e) => builder = builder.provider_warning(name, e.to_string()),
        }
    }

    builder
        .try_build()
        .map_err(|e| Fault::assembly(format!("cannot assemble a runtime: {e}")))
}

pub fn said<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, Fault> {
    serde_json::from_value(value).map_err(|e| Fault::refused(e.to_string()))
}

pub fn answered<T: serde::Serialize>(outcome: Result<T, gmr::RuntimeError>) -> Result<Value, Fault> {
    let held = outcome.map_err(fault)?;
    serde_json::to_value(held).map_err(|e| Fault::internal(e.to_string()))
}

pub async fn served<T: serde::Serialize>(
    rt: &Runtime,
    verb: &'static str,
    outcome: Result<T, gmr::RuntimeError>,
) -> Result<Value, Fault> {
    let value = answered(outcome)?;
    let bytes = value.to_string().len() as u64;
    rt.spent(verb, bytes).await.map_err(fault)?;
    Ok(value)
}

pub fn asked(how: Option<Value>) -> Result<Instructions, Fault> {
    match how {
        Some(stated) => said(stated),
        None => Ok(Instructions::default()),
    }
}

pub fn named(address: String) -> Result<Claim, Fault> {
    Claim::parse(&address).ok_or_else(|| {
        Fault::refused(format!(
            "`{address}` names nothing. A stored record is `<provider>:<id>` -- which store \
             to ask, and what to ask it for. Something an agent said is `said:<id>`, and it \
             is not stored anywhere: the utterance is the claim"
        ))
    })
}

pub fn asserting(claim: Claim, asserts: Option<Value>) -> Result<Claim, Fault> {
    match (claim, asserts) {
        (claim, None) => Ok(claim),
        (Claim::Said { id, .. }, asserts) => Ok(Claim::Said { id, asserts }),
        (Claim::Stored(reference), Some(_)) => Err(Fault::refused(format!(
            "`{reference}` is a record that lives in a store, so what it asserts is its own \
             content -- reading it off the caller instead would be a second copy of the \
             same sentence, and nothing would notice the day they disagreed"
        ))),
    }
}

pub fn asking(one: Value) -> Result<Asked, Fault> {
    let Value::String(address) = one else {
        let stated: Inline = serde_json::from_value(one).map_err(|e| {
            Fault::refused(format!(
                "an ask is either an address, or an object naming the claim and what this \
                 turn rested on: {e}"
            ))
        })?;
        let claim = asserting(named(stated.claim)?, stated.asserts)?;
        let saw = stated
            .saw
            .into_iter()
            .map(looked)
            .collect::<Result<Vec<_>, _>>()?;
        let mut ask = Asked::about(claim)
            .on(stated.anchors.into_iter().map(AnchorKey::new))
            .saw(saw);
        if let Some(source) = stated.depends {
            invariant(&source)?;
            ask = ask.depending(source);
        }
        return Ok(ask);
    };
    Ok(Asked::about(named(address)?))
}

pub fn invariant(source: &str) -> Result<(), Fault> {
    gmr::expr::parse(source).map(|_| ()).map_err(|e| {
        Fault::refused(format!(
            "`depends` is one expression that is true while the claim still stands: {e}"
        ))
    })
}

pub fn looked(address: String) -> Result<FactAddress, Fault> {
    FactAddress::try_new(&address).map_err(|e| {
        Fault::refused(format!(
            "`saw` is the address of a reading, as `sample` handed it back: {e}"
        ))
    })
}

pub fn stored(address: String) -> Result<gmr::Ref, Fault> {
    match named(address)? {
        Claim::Stored(reference) => Ok(reference),
        Claim::Said { id, .. } => Err(Fault::refused(format!(
            "`said:{id}` is an utterance, and a link runs between stored records; an \
             utterance has no store-side identity for the far end of an edge to name",
            id = id.as_str()
        ))),
    }
}

pub fn bound(
    claim: String,
    anchors: Vec<String>,
    source: &str,
    how: Asserting,
) -> Result<(Binding, Option<Version>, BTreeSet<FactAddress>, Source), Fault> {
    let claim = asserting(named(claim)?, how.asserts)?;
    let anchors = anchors.into_iter().map(AnchorKey::new).collect();
    let mut binding = Binding::on(claim, anchors);
    if let Some(condition) = how.depends {
        invariant(&condition)?;
        binding = binding.depending(condition);
    }
    let source = attested(source)?;
    let bound_version = how.bound_version.map(Version::new);
    let saw = how
        .saw
        .into_iter()
        .map(looked)
        .collect::<Result<BTreeSet<_>, _>>()?;
    Ok((binding, bound_version, saw, source))
}

pub fn revoking(
    from: String,
    to: String,
    kind: String,
    source: &str,
    asserted_as: Option<String>,
    when: DateTime<Utc>,
) -> Result<LinkRevocation, Fault> {
    Ok(LinkRevocation {
        from: stored(from)?,
        to: stored(to)?,
        kind: LinkKind(kind),
        asserted_as: asserted_as.as_deref().map(attested).transpose()?,
        source: attested(source)?,
        when,
    })
}

pub fn uttered(address: String) -> Result<gmr::SaidId, Fault> {
    match named(address)? {
        Claim::Said { id, .. } => Ok(id),
        Claim::Stored(reference) => Err(Fault::refused(format!(
            "`{reference}` is a stored record already -- condense runs from an utterance \
             into the record it became"
        ))),
    }
}

pub fn attested(source: &str) -> Result<Source, Fault> {
    Source::parse(source).ok_or_else(|| {
        Fault::refused(format!(
            "`{source}` is not a provenance. A binding says where it came from: derived, \
             self_attested, adjudicated, configured, or unknown -- and `unknown` is how you \
             say you do not know, which is why silence is not offered"
        ))
    })
}

type Made = (&'static str, Result<gmr::MemoryStore, gmr::ContentError>);

fn stores(root: &std::path::Path, asked: &Providers) -> Vec<Made> {
    let mut out = Vec::new();
    if asked.git {
        out.push(("git", Ok(gmr_provider::git::store(root))));
    }
    if asked.claude_code {
        out.push(("claude-code", gmr_provider::claude_code::store(root)));
    }
    if asked.mem0 {
        out.push(("mem0", mem0().map(gmr_provider::mem0::Mem0::store)));
    }
    out
}

fn mem0() -> Result<gmr_provider::mem0::Mem0, gmr::ContentError> {
    let held = |key: &str| std::env::var(key).ok().filter(|v| !v.is_empty());
    let scope = gmr_provider::mem0::Scope {
        user_id: held("MEM0_USER_ID"),
        agent_id: held("MEM0_AGENT_ID"),
        app_id: held("MEM0_APP_ID"),
    };
    match (held("MEM0_BASE_URL"), held("MEM0_API_KEY")) {
        (Some(base), key) => gmr_provider::mem0::Mem0::self_hosted(base, key, scope),
        (None, Some(key)) => gmr_provider::mem0::Mem0::platform(key, scope),
        (None, None) => Err(gmr::ContentError::new(
            "mem0 is asked for and neither MEM0_BASE_URL nor MEM0_API_KEY is set; a store \
             configured by environment cannot be configured by silence",
        )),
    }
}
