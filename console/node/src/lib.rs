use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use gmr::{
    AnchorKey, Asked, Binding, Claim, FactAddress, Instructions, Policy, ProbeName, Runtime,
    Source, StatusId, Version,
};
use gmr_transport::recipes::Recipes;
use napi::bindgen_prelude::*;
use napi_derive::napi;
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Opening {
    root: String,
    #[serde(default)]
    db: Option<String>,
    #[serde(default)]
    recipes: Recipes,
    #[serde(default)]
    scripts: BTreeMap<ProbeName, PathBuf>,
    #[serde(default)]
    providers: Providers,
    #[serde(default)]
    policy: Policy,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Asserting {
    #[serde(default)]
    bound_version: Option<String>,
    #[serde(default)]
    saw: Vec<String>,
    #[serde(default)]
    asserts: Option<Value>,
    #[serde(default)]
    depends: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Providers {
    #[serde(default)]
    git: bool,
    #[serde(default)]
    claude_code: bool,
    #[serde(default)]
    mem0: bool,
}

#[napi]
pub struct Gmr {
    rt: Arc<Runtime>,
}

#[napi]
pub async fn open(options: Value) -> Result<Gmr> {
    let asked: Opening = said(options)?;
    let work: Assembling = Box::pin(opened(asked));
    spawned(work).await
}

type Assembling = std::pin::Pin<Box<dyn std::future::Future<Output = Result<Gmr>> + Send>>;

async fn opened(asked: Opening) -> Result<Gmr> {
    let root = PathBuf::from(&asked.root);
    let db = match asked.db {
        Some(at) => PathBuf::from(at),
        None => root.join(".anchor").join("state").join("memory.db"),
    };
    if let Some(parent) = db.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| failed(format!("cannot make room for the store at {parent:?}: {e}")))?;
    }

    let store = gmr::sqlite::open(db.clone())
        .await
        .map_err(|e| failed(format!("cannot open the store at {}: {e}", db.display())))?;

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
        .transport(Arc::new(gmr_transport::shell::Shell::new(&root, probes)))
        .transport(Arc::new(gmr_transport::script::Script::new(
            &root,
            asked.scripts,
        )))
        .transport(Arc::new(
            gmr_transport::http::Http::new(Arc::clone(&recipes))
                .map_err(|e| failed(format!("cannot build the http transport: {e}")))?,
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

    let rt = builder
        .try_build()
        .map_err(|e| failed(format!("cannot assemble a runtime: {e}")))?;
    Ok(Gmr { rt: Arc::new(rt) })
}

#[napi]
impl Gmr {
    #[napi]
    pub async fn ground(&self, claims: Vec<Value>, how: Option<Value>) -> Result<Value> {
        let rt = Arc::clone(&self.rt);
        let claims = claims.into_iter().map(asking).collect::<Result<Vec<_>>>()?;
        let how = asked(how)?;
        spawned(async move { answered(rt.ground(&claims, &how).await) }).await
    }

    #[napi]
    pub async fn sample(&self, anchor: String, how: Option<Value>) -> Result<Value> {
        let rt = Arc::clone(&self.rt);
        let key = AnchorKey::new(anchor);
        let how = asked(how)?;
        spawned(async move { answered(rt.sample(&key, &how).await) }).await
    }

    #[napi]
    pub async fn since(&self, cursor: i64, status: Option<String>) -> Result<Value> {
        let rt = Arc::clone(&self.rt);
        let cursor = u64::try_from(cursor).map_err(|_| {
            failed(format!(
                "a cursor is a journal sequence and {cursor} is behind zero"
            ))
        })?;
        let status = status.map(StatusId::new);
        spawned(async move { answered(rt.changed_since(cursor, status.as_ref()).await) }).await
    }

    #[napi]
    pub async fn bind(
        &self,
        claim: String,
        anchors: Vec<String>,
        source: String,
        how: Option<Value>,
    ) -> Result<Value> {
        let rt = Arc::clone(&self.rt);
        let how: Asserting = match how {
            Some(stated) => said(stated)?,
            None => Asserting::default(),
        };
        let claim = asserting(named(claim)?, how.asserts)?;
        let anchors = anchors.into_iter().map(AnchorKey::new).collect();
        let mut binding = Binding::on(claim, anchors);
        if let Some(source) = how.depends {
            invariant(&source)?;
            binding = binding.depending(source);
        }
        let source = attested(&source)?;
        let bound_version = how.bound_version.map(Version::new);
        let saw = how.saw.into_iter().map(looked).collect::<Result<_>>()?;
        spawned(async move { answered(rt.bind(binding, bound_version, saw, source).await) }).await
    }

    #[napi]
    pub async fn revoke(&self, claim: String, source: String) -> Result<Value> {
        let rt = Arc::clone(&self.rt);
        let claim = named(claim)?;
        let source = attested(&source)?;
        spawned(async move { answered(rt.revoke(&claim, source).await) }).await
    }

    #[napi]
    pub async fn read(&self, anchor: String, how: Option<Value>) -> Result<Value> {
        let rt = Arc::clone(&self.rt);
        let key = AnchorKey::new(anchor);
        let how = asked(how)?;
        spawned(async move { answered(rt.grounded_within(&key, &how).await) }).await
    }

    #[napi]
    pub async fn link(&self, from: String, to: String, kind: String, source: String) -> Result<()> {
        let rt = Arc::clone(&self.rt);
        let from = stored(from)?;
        let to = stored(to)?;
        let kind = gmr::LinkKind(kind);
        let source = attested(&source)?;
        spawned(async move {
            rt.link(&from, &to, kind, source)
                .await
                .map_err(|e| failed(e.to_string()))
        })
        .await
    }

    #[napi]
    pub async fn unlink(
        &self,
        from: String,
        to: String,
        kind: String,
        source: String,
    ) -> Result<i64> {
        let rt = Arc::clone(&self.rt);
        let from = stored(from)?;
        let to = stored(to)?;
        let kind = gmr::LinkKind(kind);
        let source = attested(&source)?;
        spawned(async move {
            rt.unlink(&gmr::LinkRevocation {
                from,
                to,
                kind,
                asserted_as: None,
                source,
                when: chrono::Utc::now(),
            })
            .await
            .map(|n| n as i64)
            .map_err(|e| failed(e.to_string()))
        })
        .await
    }

    #[napi]
    pub async fn open(&self, request: Value) -> Result<Value> {
        let rt = Arc::clone(&self.rt);
        let request = said(request)?;
        spawned(async move { answered(rt.open(request).await) }).await
    }

    #[napi]
    pub async fn close(&self, key: String, why: String) -> Result<()> {
        let rt = Arc::clone(&self.rt);
        let key = AnchorKey::new(key);
        spawned(async move {
            rt.close(&key, why.as_bytes())
                .await
                .map_err(|e| failed(e.to_string()))
        })
        .await
    }
}

async fn spawned<T: Send + 'static>(
    work: impl std::future::Future<Output = Result<T>> + Send + 'static,
) -> Result<T> {
    match napi::tokio::spawn(work).await {
        Ok(done) => done,
        Err(e) => Err(failed(format!("the call did not finish: {e}"))),
    }
}

fn said<T: serde::de::DeserializeOwned>(value: Value) -> Result<T> {
    serde_json::from_value(value).map_err(|e| failed(e.to_string()))
}

fn answered<T: serde::Serialize, E: std::fmt::Display>(
    outcome: std::result::Result<T, E>,
) -> Result<Value> {
    let held = outcome.map_err(|e| failed(e.to_string()))?;
    serde_json::to_value(held).map_err(|e| failed(e.to_string()))
}

fn asked(how: Option<Value>) -> Result<Instructions> {
    match how {
        Some(stated) => said(stated),
        None => Ok(Instructions::default()),
    }
}

fn named(address: String) -> Result<Claim> {
    Claim::parse(&address).ok_or_else(|| {
        failed(format!(
            "`{address}` names nothing. A stored record is `<provider>:<id>` -- which store \
             to ask, and what to ask it for. Something an agent said is `said:<id>`, and it \
             is not stored anywhere: the utterance is the claim"
        ))
    })
}

fn asserting(claim: Claim, asserts: Option<Value>) -> Result<Claim> {
    match (claim, asserts) {
        (claim, None) => Ok(claim),
        (Claim::Said { id, .. }, asserts) => Ok(Claim::Said { id, asserts }),
        (Claim::Stored(reference), Some(_)) => Err(failed(format!(
            "`{reference}` is a record that lives in a store, so what it asserts is its own \
             content -- reading it off the caller instead would be a second copy of the \
             same sentence, and nothing would notice the day they disagreed"
        ))),
    }
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

fn asking(one: Value) -> Result<Asked> {
    let Value::String(address) = one else {
        let stated: Inline = serde_json::from_value(one).map_err(|e| {
            failed(format!(
                "an ask is either an address, or an object naming the claim and what this \
                 turn rested on: {e}"
            ))
        })?;
        let claim = asserting(named(stated.claim)?, stated.asserts)?;
        let saw = stated
            .saw
            .into_iter()
            .map(looked)
            .collect::<Result<Vec<_>>>()?;
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

fn invariant(source: &str) -> Result<()> {
    gmr::expr::parse(source).map(|_| ()).map_err(|e| {
        failed(format!(
            "`depends` is one expression that is true while the claim still stands: {e}"
        ))
    })
}

fn looked(address: String) -> Result<FactAddress> {
    FactAddress::try_new(&address).map_err(|e| {
        failed(format!(
            "`saw` is the address of a reading, as `sample` handed it back: {e}"
        ))
    })
}

fn stored(address: String) -> Result<gmr::Ref> {
    match named(address)? {
        Claim::Stored(reference) => Ok(reference),
        Claim::Said { id, .. } => Err(failed(format!(
            "`said:{id}` is an utterance, and a link runs between stored records; an \
             utterance has no store-side identity for the far end of an edge to name",
            id = id.as_str()
        ))),
    }
}

fn attested(source: &str) -> Result<Source> {
    Source::parse(source).ok_or_else(|| {
        failed(format!(
            "`{source}` is not a provenance. A binding says where it came from: derived, \
             self_attested, adjudicated, configured, or unknown -- and `unknown` is how you \
             say you do not know, which is why silence is not offered"
        ))
    })
}

fn failed(message: String) -> napi::Error {
    napi::Error::from_reason(message)
}

type Made = (
    &'static str,
    std::result::Result<gmr::MemoryStore, gmr::ContentError>,
);

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

fn mem0() -> std::result::Result<gmr_provider::mem0::Mem0, gmr::ContentError> {
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
