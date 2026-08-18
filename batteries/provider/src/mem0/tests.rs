use super::http::testkit::Canned;
use super::*;
use std::time::Duration;

fn plenty() -> Budget {
    Budget::within(Duration::from_secs(30), usize::MAX)
}

fn mem0(http: Canned) -> Mem0 {
    Mem0::faked(Box::new(http), Scope::user("u1"), Deployment::Platform)
}

fn self_hosted(http: Canned) -> Mem0 {
    Mem0::faked(Box::new(http), Scope::user("u1"), Deployment::SelfHosted)
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
    assert_eq!(records[0].reference, Ref::new("mem0", "m-1"));
    assert_eq!(records[0].version, version_of("one"));
}

#[tokio::test]
async fn an_answer_without_the_text_is_refused_not_read_as_an_empty_memory() {
    let provider = mem0(Canned::new().on("/v1/memories/m-1/", 200, r#"{"id":"m-1"}"#));

    provider
        .fetch(&ExternalId::new("m-1"), &plenty())
        .await
        .expect_err(
            "mem0 renaming or dropping the field the text arrives in must be refused. Read as \
             an empty memory it hashes to a stable version, so every record reports as current \
             and empty and nothing anywhere says the store stopped being understood",
        );
}

#[tokio::test]
async fn a_listing_without_the_records_is_refused_not_read_as_an_empty_store() {
    let provider = mem0(Canned::new().on("/v1/memories/?user_id=u1", 200, r#"{"next":null}"#));

    provider.list(&plenty()).await.expect_err(
        "a listing whose shape we no longer understand must be refused. Read as zero records it \
         is indistinguishable from a scope that is genuinely empty",
    );
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

const SELF_HOSTED_ONE: &str = r#"{"id":"aaaaaaaa-0000-4000-8000-000000000001","memory":"prefers tabs over spaces","hash":"h1","metadata":null,"score":null,"created_at":"2026-08-17T00:00:00Z","updated_at":"2026-08-17T01:00:00Z","user_id":"gmr-probe"}"#;

const SELF_HOSTED_HISTORY: &str = r#"[{"id":"h-1","memory_id":"aaaaaaaa-0000-4000-8000-000000000001","old_memory":null,"new_memory":"likes spaces","event":"ADD","created_at":"2026-08-17T00:00:00","updated_at":"2026-08-17T00:00:00","is_deleted":false,"actor_id":null,"role":null},{"id":"h-2","memory_id":"aaaaaaaa-0000-4000-8000-000000000001","old_memory":"likes spaces","new_memory":"prefers tabs over spaces","event":"UPDATE","created_at":"2026-08-17T01:00:00","updated_at":"2026-08-17T01:00:00","is_deleted":false,"actor_id":null,"role":null}]"#;

const SELF_HOSTED_LISTING: &str = r#"{"results":[{"id":"aaaaaaaa-0000-4000-8000-000000000001","memory":"prefers tabs over spaces","hash":"h1","metadata":null,"created_at":"2026-08-17T00:00:00Z","updated_at":"2026-08-17T01:00:00Z","user_id":"gmr-probe"}]}"#;

#[tokio::test]
async fn the_self_hosted_routes_carry_no_v1_and_no_trailing_slash() {
    let http = Canned::new()
        .on("/memories/m-1", 200, SELF_HOSTED_ONE)
        .on("/memories/m-1/history", 200, SELF_HOSTED_HISTORY)
        .on("top_k=1000", 200, SELF_HOSTED_LISTING);
    let asked = http.log();
    let provider = self_hosted(http);
    let id = ExternalId::new("m-1");

    provider.fetch(&id, &plenty()).await.unwrap();
    provider
        .fetch_at(&id, &version_of("likes spaces"), &plenty())
        .await
        .unwrap();
    provider.list(&plenty()).await.unwrap();

    assert_eq!(
        asked.lock().unwrap().clone(),
        vec![
            "https://mem0.test/memories/m-1".to_owned(),
            "https://mem0.test/memories/m-1/history".to_owned(),
            "https://mem0.test/memories?user_id=u1&top_k=1000".to_owned(),
        ],
        "the self-hosted server runs FastAPI with redirect_slashes=False, so a trailing slash \
         is a 404 rather than a redirect, and it mounts no /v1 prefix at all. Pointing the \
         platform's URLs at it 404s on every route — and a 404 on the history route reads as \
         `this version was not retained`, which is a normal condition nobody is asked to fix"
    );
}

#[tokio::test]
async fn a_self_hosted_absence_is_authoritative_and_costs_no_second_call() {
    let http = Canned::new().on("/memories/m-gone", 200, "null");
    let asked = http.log();
    let provider = self_hosted(http);

    let fetched = provider
        .fetch(&ExternalId::new("m-gone"), &plenty())
        .await
        .unwrap();

    assert!(fetched.is_none());
    assert_eq!(
        asked.lock().unwrap().len(),
        1,
        "the platform needs a second call because its 404 also comes from a lost key and a \
         moved scope. A self-hosted 200 null cannot: a rejected key is 401 and a store that \
         cannot answer is 502, so nothing but a genuinely absent memory produces it. Probing \
         anyway would be a round trip that can decide nothing"
    );
}

#[tokio::test]
async fn a_self_hosted_404_on_the_history_route_is_a_failure_not_an_unretained_version() {
    let provider =
        self_hosted(Canned::new().on("/memories/m-1/history", 404, r#"{"detail":"Not Found"}"#));

    let err = provider
        .fetch_at(&ExternalId::new("m-1"), &version_of("anything"), &plenty())
        .await
        .expect_err(
            "this route answers 200 [] for a memory it has never heard of, so it has no 404 to \
             give — one means the address is not a mem0 server. Reading it as Ok(None) is how \
             a wholly misconfigured provider used to report `that version was not retained`, \
             a condition D6 puts in the bucket that never asks anyone to act",
        );

    assert_eq!(err.code, gmr_content::ContentErrorCode::ProviderFailed);
}

#[tokio::test]
async fn a_self_hosted_listing_sitting_on_the_ceiling_is_refused() {
    let record = || Record {
        reference: Ref::new("mem0", "m"),
        version: version_of("x"),
        bytes: b"x".to_vec(),
    };

    assert!(
        Deployment::SelfHosted
            .whole(std::iter::repeat_with(record).take(999).collect())
            .is_ok()
    );
    assert!(
        Deployment::SelfHosted
            .whole(std::iter::repeat_with(record).take(1000).collect())
            .is_err(),
        "top_k caps at 1000 and 1001 is a 422, while the route carries neither a cursor nor a \
         total — so exactly 1000 records is where a complete listing and a truncated one stop \
         being distinguishable. Handing that back as complete would read as every record past \
         the thousandth having disappeared"
    );
    assert!(
        Deployment::Platform
            .whole(std::iter::repeat_with(record).take(1000).collect())
            .is_ok(),
        "the platform pages through `next`, so a thousand records there is just a thousand"
    );
}

#[test]
fn the_key_travels_in_a_different_header_per_deployment() {
    let platform = Deployment::Platform.credential("k".to_owned());
    assert_eq!(
        (platform.header, platform.value.as_str()),
        ("Authorization", "Token k")
    );

    let self_hosted = Deployment::SelfHosted.credential("k".to_owned());
    assert_eq!(
        (self_hosted.header, self_hosted.value.as_str()),
        ("X-API-Key", "k")
    );
}

#[test]
fn a_scope_named_only_by_app_id_is_refused_for_a_self_hosted_server() {
    let err = Mem0::self_hosted(
        "http://localhost:8888",
        None,
        Scope {
            user_id: None,
            agent_id: None,
            app_id: Some("a1".to_owned()),
        },
    )
    .err()
    .expect(
        "the self-hosted listing route filters on user_id, agent_id and run_id, and drops an \
         app_id it does not know. What is left is a request that names no scope at all, which \
         that route answers with every memory in the store rather than with none",
    );

    assert!(err.message.contains("app_id"), "{err}");
}

#[test]
fn a_scope_that_names_nothing_is_refused_by_both_deployments() {
    let nothing = || Scope {
        user_id: None,
        agent_id: None,
        app_id: None,
    };

    assert!(Mem0::platform("k", nothing()).is_err());
    assert!(Mem0::self_hosted("http://localhost:8888", None, nothing()).is_err());
}
