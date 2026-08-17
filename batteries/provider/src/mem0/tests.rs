use super::http::testkit::Canned;
use super::*;
use std::time::Duration;

fn plenty() -> Budget {
    Budget::within(Duration::from_secs(30), usize::MAX)
}

fn mem0(http: Canned) -> Mem0 {
    Mem0::faked(Box::new(http), Scope::user("u1"))
}

const ONE: &str = r#"{"id":"m-1","memory":"prefers tabs over spaces"}"#;

#[tokio::test]
async fn a_memorys_version_is_the_hash_of_its_text() {
    let provider = mem0(Canned::new().on("/v1/memories/m-1/", 200, ONE));

    let fetched = provider
        .fetch(&ExternalId::new("m-1"), &plenty())
        .await
        .unwrap()
        .unwrap();

    assert_eq!(fetched.bytes, b"prefers tabs over spaces");
    assert_eq!(fetched.version, version_of("prefers tabs over spaces"));
}

#[tokio::test]
async fn the_same_text_always_hashes_to_the_same_version() {
    let a = mem0(Canned::new().on("/v1/memories/m-1/", 200, ONE));
    let b = mem0(Canned::new().on(
        "/v1/memories/m-1/",
        200,
        r#"{"id":"m-1","memory":"prefers tabs over spaces","updated_at":"2026-08-17T10:00:00Z"}"#,
    ));

    let one = a.fetch(&ExternalId::new("m-1"), &plenty()).await.unwrap();
    let two = b.fetch(&ExternalId::new("m-1"), &plenty()).await.unwrap();

    assert_eq!(
        one.unwrap().version,
        two.unwrap().version,
        "the version has to come from the text alone. Deriving it from updated_at would \
         make an untouched memory look rewritten every time mem0 touched a timestamp, and \
         two updates inside one millisecond indistinguishable"
    );
}

#[tokio::test]
async fn a_deleted_memory_is_the_worlds_answer_once_the_scope_still_lists() {
    let provider = mem0(Canned::new().on("/v1/memories/m-gone/", 404, "{}").on(
        "page_size=1",
        200,
        "[]",
    ));

    let fetched = provider
        .fetch(&ExternalId::new("m-gone"), &plenty())
        .await
        .unwrap();

    assert!(fetched.is_none());
}

#[tokio::test]
async fn a_404_that_is_really_a_scope_problem_is_our_failure_not_the_worlds_answer() {
    let provider = mem0(Canned::new().on("/v1/memories/m-1/", 404, "{}").on(
        "page_size=1",
        401,
        r#"{"detail":"invalid token"}"#,
    ));

    let err = provider
        .fetch(&ExternalId::new("m-1"), &plenty())
        .await
        .expect_err(
            "mem0 answers 404 for a memory that was deleted, for a key that lost permission, \
             and for a scope that no longer matches. Mapping all three to Ok(None) makes \
             doctor report a screenful of dead references that are all still there, and the \
             obvious fix a reader would apply is to delete the bindings",
        );

    assert!(err.message.contains("credentials or scope"), "{err}");
}

#[tokio::test]
async fn history_is_reconstructed_by_hashing_what_each_change_produced() {
    let provider = mem0(Canned::new().on(
        "/v1/memories/m-1/history/",
        200,
        r#"[{"event":"ADD","old_memory":null,"new_memory":"likes spaces"},
            {"event":"UPDATE","old_memory":"likes spaces","new_memory":"prefers tabs over spaces"}]"#,
    ));

    let was = provider
        .fetch_at(
            &ExternalId::new("m-1"),
            &version_of("likes spaces"),
            &plenty(),
        )
        .await
        .unwrap();

    assert_eq!(
        was,
        Some(b"likes spaces".to_vec()),
        "mem0 has no endpoint that returns a memory as of a version. It does keep an \
         append-only log of what each change produced, which is enough to rebuild any \
         version the memory ever held"
    );
}

#[tokio::test]
async fn a_version_that_never_appears_in_the_log_is_none_not_an_error() {
    let provider = mem0(Canned::new().on(
        "/v1/memories/m-1/history/",
        200,
        r#"[{"event":"ADD","new_memory":"likes spaces"}]"#,
    ));

    let was = provider
        .fetch_at(
            &ExternalId::new("m-1"),
            &version_of("something nobody ever wrote"),
            &plenty(),
        )
        .await
        .unwrap();

    assert!(
        was.is_none(),
        "mem0 lets a memory be written with an expiry and its consolidation deletes layers \
         it supersedes, so a version genuinely falling out of the log is normal. That is \
         NotRetained, which is about this one binding — not NoHistory, which would say this \
         backend keeps none at all"
    );
}

#[tokio::test]
async fn a_listing_says_nothing_about_what_each_record_is_about() {
    let provider = mem0(Canned::new().on(
        "/v1/memories/?user_id=u1",
        200,
        r#"[{"id":"m-1","memory":"one","metadata":{"gmr":{"about":"src/a.rs"}}},
            {"id":"m-2","memory":"two"}]"#,
    ));

    let records = provider.list(&plenty()).await.unwrap();

    assert_eq!(records.len(), 2);
    assert!(
        records.iter().all(|r| r.claim == Claim::Silent),
        "even the record carrying metadata.gmr reads as Silent. Honouring it would promise \
         a declaration channel mem0 does not guarantee — its update path makes no promise \
         about metadata surviving — and a claim that works today and vanishes tomorrow is \
         worse than one that never existed. Declarations go through `gmr bind`"
    );
    assert_eq!(records[0].reference, Ref::new("mem0", "m-1"));
    assert_eq!(records[0].version, version_of("one"));
}

#[tokio::test]
async fn a_listing_cut_short_by_the_budget_is_an_error_not_a_short_list() {
    let provider = mem0(Canned::new().on(
        "/v1/memories/?user_id=u1",
        200,
        r#"{"results":[{"id":"m-1","memory":"one"}],"next":"https://mem0.test/v1/memories/?page=2"}"#,
    ));

    let err = provider
        .list(&Budget::within(Duration::from_millis(0), usize::MAX))
        .await
        .expect_err("a partial listing must not be handed back as a complete one");

    assert_eq!(err.code, gmr_content::ContentErrorCode::BudgetSpent);
}

#[tokio::test]
async fn a_store_that_will_not_answer_is_never_read_as_a_record_being_gone() {
    let provider = mem0(Canned::new().refusing("connection reset"));

    let err = provider
        .fetch(&ExternalId::new("m-1"), &plenty())
        .await
        .unwrap_err();

    assert_eq!(err.code, gmr_content::ContentErrorCode::ProviderFailed);
}

#[tokio::test]
async fn this_provider_offers_history_unlike_the_local_file_ones() {
    let provider = mem0(Canned::new());
    assert!(provider.history().is_some());
}
