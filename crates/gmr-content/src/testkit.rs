//! The two things a provider must get right that no type can express.
//!
//! Everything else a store owes is checked by the compiler: a capability it
//! lacks is a trait it does not implement. What is left is the meaning of
//! two answers — what a `Version` tracks, and what `Ok(None)` claims — and
//! a backend passes or fails those the same way whatever it is talking to.
//!
//! A backend supplies a `Corpus`, which is the smallest world these can be
//! asked in, and calls `conforms`.

use std::time::Duration;

use async_trait::async_trait;
use gmr_core::ExternalId;
use gmr_probe::Budget;

use crate::ContentProvider;

#[async_trait]
pub trait Corpus: Send + Sync {
    fn provider(&self) -> &dyn ContentProvider;

    async fn holding(&self, bytes: &[u8]) -> ExternalId;

    async fn never_held(&self) -> ExternalId;

    async fn out_of_reach(&self) -> Box<dyn ContentProvider>;
}

fn budget() -> Budget {
    Budget::within(Duration::from_secs(30), usize::MAX)
}

pub async fn conforms(corpus: &dyn Corpus) -> Result<(), String> {
    same_content_is_the_same_version(corpus).await?;
    changed_content_is_a_changed_version(corpus).await?;
    an_id_never_held_is_never_content(corpus).await?;
    a_store_out_of_reach_never_reads_as_an_absence(corpus).await
}

pub async fn same_content_is_the_same_version(corpus: &dyn Corpus) -> Result<(), String> {
    let text = b"prefers tabs over spaces";
    let one = fetched(corpus, &corpus.holding(text).await).await?;
    let two = fetched(corpus, &corpus.holding(text).await).await?;

    match one.version == two.version {
        true => Ok(()),
        false => Err(format!(
            "two records holding the same bytes came back with different versions ({} and \
             {}). GMR compares a binding's stored version against the current one to decide \
             whether a memory was rewritten, so a version tracking anything besides content \
             — a timestamp, an id, a revision counter — reports every untouched record as \
             rewritten and buries the ones that really were",
            one.version, two.version
        )),
    }
}

pub async fn changed_content_is_a_changed_version(corpus: &dyn Corpus) -> Result<(), String> {
    let one = fetched(corpus, &corpus.holding(b"likes spaces").await).await?;
    let two = fetched(corpus, &corpus.holding(b"prefers tabs").await).await?;

    match one.version == two.version {
        false => Ok(()),
        true => Err(format!(
            "two records holding different bytes came back with the same version ({}). \
             Nothing downstream can tell that a memory changed, so it is never handed back \
             for re-reading — the quietest way this system can fail",
            one.version
        )),
    }
}

pub async fn an_id_never_held_is_never_content(corpus: &dyn Corpus) -> Result<(), String> {
    let id = corpus.never_held().await;
    match corpus.provider().fetch(&id, &budget()).await {
        Ok(Some(_)) => Err(format!(
            "the store handed back content for `{id}`, which it was never given. Either the \
             scope being read is wider than it was asked for, or the address being built is \
             not the one this provider thinks it is"
        )),
        Ok(None) | Err(_) => Ok(()),
    }
}

pub async fn a_store_out_of_reach_never_reads_as_an_absence(
    corpus: &dyn Corpus,
) -> Result<(), String> {
    let id = corpus.holding(b"still here").await;
    let broken = corpus.out_of_reach().await;

    match broken.fetch(&id, &budget()).await {
        Err(_) => Ok(()),
        Ok(None) => Err(format!(
            "a store that could not be reached answered that `{id}` is gone. `Ok(None)` is \
             the world's answer and turns into a dead reference a reader is told to delete; \
             a store that will not answer is our failure and must stay `Err`, which D6 keeps \
             out of every exit code. Confusing the two makes `doctor` print a screenful of \
             bindings to remove that are all still there"
        )),
        Ok(Some(_)) => Err(format!(
            "a store that should have been out of reach still answered for `{id}`, so this \
             conformance run proved nothing about the case it exists to check"
        )),
    }
}

async fn fetched(corpus: &dyn Corpus, id: &ExternalId) -> Result<crate::Fetched, String> {
    corpus
        .provider()
        .fetch(id, &budget())
        .await
        .map_err(|e| format!("fetching `{id}` failed: {e}"))?
        .ok_or_else(|| format!("the store has no `{id}`, which it was just given"))
}
