//! A canary for mem0's wire shape, not a test of this crate's logic.
//!
//! Everything decidable without a network — version derivation, history
//! reconstruction, the 404 split, listing and pagination — is tested in the
//! module itself against a fake. What is left here is the one thing a fake
//! cannot check: that mem0 still answers the way this battery reads it.
//!
//! It is `#[ignore]` because it needs credentials and somebody else's
//! service being up. Run it deliberately, after a mem0 release or when a
//! user reports something this crate's own tests all say is impossible:
//!
//!     MEM0_API_KEY=... MEM0_USER_ID=... \
//!         cargo test -p gmr-provider --features mem0 -- --ignored --nocapture

#![cfg(feature = "mem0")]

use gmr_content::{ContentProvider, MemorySource};
use gmr_probe::Budget;
use gmr_provider::mem0::{Mem0, Scope};
use std::time::Duration;

fn from_env() -> Option<Mem0> {
    let key = std::env::var("MEM0_API_KEY").ok()?;
    let user = std::env::var("MEM0_USER_ID").ok()?;
    Mem0::new(key, Scope::user(user)).ok()
}

#[tokio::test]
#[ignore = "needs MEM0_API_KEY and MEM0_USER_ID, and mem0 being up"]
async fn mem0_still_answers_the_way_this_battery_reads_it() {
    let Some(provider) = from_env() else {
        panic!("set MEM0_API_KEY and MEM0_USER_ID to run this");
    };
    let budget = Budget::within(Duration::from_secs(30), usize::MAX);

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
}
