use std::sync::Arc;

use gmr::{AnchorKey, Runtime, StatusId};
use gmr_console as core;
use napi::bindgen_prelude::*;
use napi_derive::napi;
use serde_json::Value;

#[napi]
pub const CONTRACT: &str = gmr::contract::CONTRACT;

#[napi]
pub struct Gmr {
    rt: Arc<Runtime>,
}

#[napi]
pub async fn open(options: Value) -> Result<Gmr> {
    let asked: core::Opening = ok(core::said(options))?;
    let work: Assembling = Box::pin(async move {
        let rt = core::opened(asked).await.map_err(failed)?;
        Ok(Gmr { rt: Arc::new(rt) })
    });
    spawned(work).await
}

type Assembling = std::pin::Pin<Box<dyn std::future::Future<Output = Result<Gmr>> + Send>>;

#[napi]
impl Gmr {
    #[napi]
    pub async fn ground(&self, claims: Vec<Value>, how: Option<Value>) -> Result<Value> {
        let rt = Arc::clone(&self.rt);
        let claims = claims
            .into_iter()
            .map(|c| ok(core::asking(c)))
            .collect::<Result<Vec<_>>>()?;
        let how = ok(core::asked(how))?;
        spawned(async move { ok(core::answered(rt.ground(&claims, &how).await)) }).await
    }

    #[napi]
    pub async fn sample(&self, anchor: String, how: Option<Value>) -> Result<Value> {
        let rt = Arc::clone(&self.rt);
        let key = AnchorKey::new(anchor);
        let how = ok(core::asked(how))?;
        spawned(async move { ok(core::answered(rt.sample(&key, &how).await)) }).await
    }

    #[napi]
    pub async fn read(&self, anchor: String, how: Option<Value>) -> Result<Value> {
        let rt = Arc::clone(&self.rt);
        let key = AnchorKey::new(anchor);
        let how = ok(core::asked(how))?;
        spawned(async move { ok(core::answered(rt.grounded_within(&key, &how).await)) }).await
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
        spawned(async move {
            ok(core::answered(
                rt.changed_since(cursor, status.as_ref()).await,
            ))
        })
        .await
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
        let how: core::Asserting = match how {
            Some(stated) => ok(core::said(stated))?,
            None => core::Asserting::default(),
        };
        let (binding, bound_version, saw, source) = ok(core::bound(claim, anchors, &source, how))?;
        spawned(async move {
            ok(core::answered(
                rt.bind(binding, bound_version, saw, source).await,
            ))
        })
        .await
    }

    #[napi]
    pub async fn revoke(&self, claim: String, source: String) -> Result<Value> {
        let rt = Arc::clone(&self.rt);
        let claim = ok(core::named(claim))?;
        let source = ok(core::attested(&source))?;
        spawned(async move { ok(core::answered(rt.revoke(&claim, source).await)) }).await
    }

    #[napi]
    pub async fn link(&self, from: String, to: String, kind: String, source: String) -> Result<()> {
        let rt = Arc::clone(&self.rt);
        let from = ok(core::stored(from))?;
        let to = ok(core::stored(to))?;
        let kind = gmr::LinkKind(kind);
        let source = ok(core::attested(&source))?;
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
        let revocation = ok(core::revoking(from, to, kind, &source, chrono::Utc::now()))?;
        spawned(async move {
            rt.unlink(&revocation)
                .await
                .map(|n| n as i64)
                .map_err(|e| failed(e.to_string()))
        })
        .await
    }

    #[napi]
    pub async fn links(&self, record: String) -> Result<Value> {
        let rt = Arc::clone(&self.rt);
        let record = ok(core::stored(record))?;
        spawned(async move { ok(core::answered(rt.links(&record).await)) }).await
    }

    #[napi]
    pub async fn condense(&self, said: String, into: String, source: String) -> Result<Value> {
        let rt = Arc::clone(&self.rt);
        let said = ok(core::uttered(said))?;
        let into = ok(core::stored(into))?;
        let source = ok(core::attested(&source))?;
        spawned(async move { ok(core::answered(rt.condense(&said, into, source).await)) }).await
    }

    #[napi]
    pub async fn open(&self, request: Value) -> Result<Value> {
        let rt = Arc::clone(&self.rt);
        let request = ok(core::said(request))?;
        spawned(async move { ok(core::answered(rt.open(request).await)) }).await
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

fn ok<T>(outcome: std::result::Result<T, core::Fault>) -> Result<T> {
    outcome.map_err(failed)
}

fn failed(message: impl Into<String>) -> napi::Error {
    napi::Error::from_reason(message.into())
}
