use std::time::Duration;

use async_trait::async_trait;
use gmr_budget::Budget;
use gmr_core::{ExternalId, Version};

use crate::{ContentProvider, MemorySource};

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

#[async_trait]
pub trait Listing: Corpus {
    fn source(&self) -> &dyn MemorySource;
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

pub async fn lists(listing: &dyn Listing) -> Result<(), String> {
    a_listing_addresses_the_store_it_came_from(listing).await?;
    everything_listed_can_be_fetched(listing).await?;
    a_listed_version_is_the_one_fetch_computes(listing).await
}

pub async fn a_listing_addresses_the_store_it_came_from(
    listing: &dyn Listing,
) -> Result<(), String> {
    let source = listing.source().provider();
    let content = listing.provider().provider();

    match source == content {
        true => Ok(()),
        false => Err(format!(
            "the listing hands back references addressed to `{source}` while the store that \
             answers for them is `{content}`. A binding stamped with one of those references \
             is looked up by its provider name, so every record this listing offers binds to \
             a store that will never be asked, and `read` reports each one as having no \
             provider registered"
        )),
    }
}

pub async fn everything_listed_can_be_fetched(listing: &dyn Listing) -> Result<(), String> {
    let id = listing.holding(b"a record this store was just given").await;
    listed(listing, &id).await?;

    let offered = listing
        .source()
        .list(&budget())
        .await
        .map_err(|e| format!("listing the store failed: {e}"))?;

    for record in offered {
        let id = &record.reference.external_id;
        match listing.provider().fetch(id, &budget()).await {
            Ok(Some(_)) => {}
            Ok(None) => {
                return Err(format!(
                    "the listing offered `{id}` and the store then said it holds no such \
                     record. `gmr memories` exists so a reference can be found and bound \
                     without having to guess one, so a listing that names records the store \
                     cannot answer for offers nothing but dead bindings — each of which \
                     `doctor` will tell the reader to delete"
                ));
            }
            Err(e) => {
                return Err(format!(
                    "the listing offered `{id}` and fetching it failed: {e}. Whatever \
                     address the listing builds is not the one this provider reads"
                ));
            }
        }
    }
    Ok(())
}

pub async fn a_listed_version_is_the_one_fetch_computes(
    listing: &dyn Listing,
) -> Result<(), String> {
    let id = listing.holding(b"prefers tabs over spaces").await;
    let record = listed(listing, &id).await?;
    let fetched = fetched(listing, &record.reference.external_id).await?;

    match record.version == fetched.version {
        true => Ok(()),
        false => Err(format!(
            "the listing versions `{}` as {} and fetching it computes {}. A binding is \
             stamped with the version the listing gave and `read` compares it against the \
             version the provider computes, so two ways of arriving at a version means one \
             store state where they disagree — and there every record reports as rewritten, \
             with a bound version nothing can retrieve",
            record.reference.external_id, record.version, fetched.version
        )),
    }
}

pub async fn retains(corpus: &dyn Corpus) -> Result<(), String> {
    let Some(history) = corpus.provider().history() else {
        return Err(
            "this store was run against the history suite and implements no `History`. \
             Declining that capability is not implementing the trait, so a store that \
             declines has nothing to check here and must not be asked"
                .to_owned(),
        );
    };
    let id = corpus.holding(b"still here").await;
    let never = Version::new("a-version-this-store-never-issued");

    match history.fetch_at(&id, &never, &budget()).await {
        Ok(None) => Ok(()),
        Ok(Some(_)) => Err(format!(
            "the store handed back content for `{id}` at a version it never issued, so \
             whatever `fetch_at` matches on is not the version bindings are stamped with"
        )),
        Err(e) => Err(format!(
            "asking for `{id}` at a version this store never issued failed: {e}. A version \
             that has genuinely fallen out of the log is the world's answer and renders as \
             `the bound version was not kept`; a store that will not answer is our failure. \
             Reported as the second, every consolidated-away version turns a build red that \
             nobody holding this repository can fix"
        )),
    }
}

async fn listed(listing: &dyn Listing, id: &ExternalId) -> Result<crate::Record, String> {
    listing
        .source()
        .list(&budget())
        .await
        .map_err(|e| format!("listing the store failed: {e}"))?
        .into_iter()
        .find(|r| &r.reference.external_id == id)
        .ok_or_else(|| {
            format!(
                "`{id}` was just given to this store and its listing does not offer it. A \
                 listing is what a store will show rather than a roster of all it holds, but \
                 a record written and immediately withheld leaves no way to tell that from a \
                 scope being read narrower than it was asked for"
            )
        })
}
