//! A canary for mem0's wire shape, not a test of this crate's logic.
//!
//! Everything decidable without a network — version derivation, history
//! reconstruction, both absence spellings, the routes each deployment
//! mounts, listing and pagination — is tested in the module itself against
//! a fake. What is left here is the one thing a fake cannot check: that a
//! real mem0 still answers the way this battery reads it.
//!
//! One criterion set, run against whichever deployment the environment
//! names, because the point is that both are the same store to GMR:
//!
//!     # the managed platform
//!     MEM0_API_KEY=... MEM0_USER_ID=... \
//!         cargo test -p gmr-provider --features mem0 -- --ignored --nocapture
//!
//!     # a self-hosted server: `docker compose up` in mem0's own server/
//!     MEM0_BASE_URL=http://localhost:8888 MEM0_USER_ID=... \
//!         cargo test -p gmr-provider --features mem0 -- --ignored --nocapture
//!
//! It is `#[ignore]` because it needs somebody else's service being up. Run
//! it deliberately, after a mem0 release or when a user reports something
//! this crate's own tests all say is impossible.

#![cfg(feature = "mem0")]

use gmr_content::{ContentProvider, MemorySource};
use gmr_core::ExternalId;
use gmr_probe::Budget;
use gmr_provider::mem0::{Mem0, Scope};
use std::time::Duration;

fn from_env() -> Option<(Mem0, &'static str)> {
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    let scope = Scope::user(env("MEM0_USER_ID")?);
    match (env("MEM0_BASE_URL"), env("MEM0_API_KEY")) {
        (Some(base), key) => Some((Mem0::self_hosted(base, key, scope).unwrap(), "self-hosted")),
        (None, Some(key)) => Some((Mem0::platform(key, scope).unwrap(), "platform")),
        (None, None) => None,
    }
}

fn addressed() -> (Mem0, &'static str, Budget) {
    let Some((provider, deployment)) = from_env() else {
        panic!(
            "name a deployment to run this: MEM0_API_KEY for the managed platform, or \
             MEM0_BASE_URL for a self-hosted server. MEM0_USER_ID is needed either way — a \
             scope that names nobody is refused at assembly"
        );
    };
    println!("--- against the {deployment} deployment ---");
    (
        provider,
        deployment,
        Budget::within(Duration::from_secs(30), usize::MAX),
    )
}

#[tokio::test]
#[ignore = "needs a real mem0: MEM0_API_KEY, or MEM0_BASE_URL for a self-hosted one"]
async fn mem0_still_answers_the_way_this_battery_reads_it() {
    let (provider, _, budget) = addressed();

    let records = provider
        .list(&budget)
        .await
        .expect("listing the configured scope");
    println!("listed {} record(s)", records.len());

    let Some(first) = records.first() else {
        println!("nothing in this scope; the shape of a listing was still confirmed");
        return;
    };

    let fetched = provider
        .fetch(&first.reference.external_id, &budget)
        .await
        .expect("fetching a record the listing just named")
        .expect("a record the listing just named must be fetchable");

    assert_eq!(
        fetched.version, first.version,
        "the version a listing reports and the version a fetch reports have to agree, or \
         every listed record reads as rewritten the moment it is bound"
    );

    let history = provider
        .history()
        .expect("mem0 keeps history")
        .fetch_at(&first.reference.external_id, &fetched.version, &budget)
        .await
        .expect("asking for the version we just read");

    assert_eq!(
        history.as_deref(),
        Some(fetched.bytes.as_slice()),
        "the current version must appear in the change log, since that log is the only \
         thing this battery can rebuild an older version out of"
    );
    println!("fetch, version agreement and history all confirmed");
}

#[tokio::test]
#[ignore = "needs a real mem0: MEM0_API_KEY, or MEM0_BASE_URL for a self-hosted one"]
async fn an_id_that_was_never_there_is_never_answered_with_content() {
    let (provider, deployment, budget) = addressed();
    let nobodys = ExternalId::new("3f8c1d02-0000-4000-8000-000000000000");

    match provider.fetch(&nobodys, &budget).await {
        Ok(None) => println!("{deployment}: an unknown id reads as gone, authoritatively"),
        Err(e) => println!("{deployment}: an unknown id reads as a failure to answer: {e}"),
        Ok(Some(_)) => panic!(
            "mem0 handed back content for an id that was never written. Either this scope is \
             wider than it was asked to be, or the route being addressed is not the one this \
             battery thinks it is"
        ),
    }
}

#[tokio::test]
#[ignore = "needs a real mem0: MEM0_API_KEY, or MEM0_BASE_URL for a self-hosted one"]
async fn a_mem0_out_of_reach_never_reads_as_a_record_being_gone() {
    let (provider, deployment, budget) = addressed();
    let scope = Scope::user(std::env::var("MEM0_USER_ID").unwrap());
    let unreachable = Mem0::self_hosted("http://127.0.0.1:9", None, scope).unwrap();

    let records = provider.list(&budget).await.expect("listing the scope");
    let Some(first) = records.first() else {
        println!("{deployment}: nothing in this scope to ask about");
        return;
    };

    let answer = unreachable
        .fetch(&first.reference.external_id, &budget)
        .await;

    assert!(
        answer.is_err(),
        "a record the live store just listed came back from an unreachable one as {answer:?}. \
         `Ok(None)` is the world's answer and becomes a dead reference a reader is told to \
         delete; a store that will not answer is our failure, which D6 keeps out of every \
         exit code. This is the half of the conformance suite mem0 cannot run offline, \
         because building a corpus would mean writing into it and this battery cannot"
    );
}
