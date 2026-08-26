use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use gmr_budget::Budget;
use gmr_core::{Kind, Outcome, ProbeName, ProbeRef};
use gmr_probe::{ProbeCall, ProbeError, Transport};
use gmr_transport::http::{Ask, Fetch, Header, Http, Reply, pointer};

struct Answers {
    status: u16,
    body: String,
    seen: std::sync::Mutex<Vec<(String, String)>>,
}

impl Answers {
    fn new(status: u16, body: &str) -> Arc<Self> {
        Arc::new(Self {
            status,
            body: body.to_owned(),
            seen: std::sync::Mutex::new(Vec::new()),
        })
    }
}

#[async_trait]
impl Fetch for Answers {
    async fn get(
        &self,
        _url: &str,
        headers: &[(String, String)],
        _budget: &Budget,
    ) -> Result<Reply, ProbeError> {
        self.seen.lock().unwrap().extend_from_slice(headers);
        Ok(Reply {
            status: self.status,
            body: self.body.clone(),
        })
    }
}

fn probe() -> ProbeRef {
    ProbeRef::new(
        Kind::new("http"),
        ProbeName::new("quote"),
        serde_json::Value::Null,
    )
}

async fn ask_with(ask: Ask, fetch: Arc<dyn Fetch>) -> Result<Outcome, ProbeError> {
    let mut asks = BTreeMap::new();
    asks.insert(ProbeName::new("quote"), ask);
    let http = Http::with(asks, fetch);
    let budget = Budget::within(Duration::from_secs(5), 1024 * 1024);
    let probe = probe();
    http.invoke(&ProbeCall {
        probe: &probe,
        position: &serde_json::Value::Null,
        budget: &budget,
    })
    .await
}

#[tokio::test]
async fn a_missing_resource_is_the_worlds_answer_and_an_outage_is_ours() {
    let gone = ask_with(Ask::at("https://x/q"), Answers::new(404, ""))
        .await
        .expect("404 is an answer, not a failure");
    assert_eq!(
        gone,
        Outcome::NotFound,
        "the endpoint answered, and the answer was that the thing is not there. Reporting \
         this as a ProbeError would file the world's answer under our failures, and the \
         anchor would back off and retry a fact that is settled"
    );

    let down = ask_with(Ask::at("https://x/q"), Answers::new(503, ""))
        .await
        .expect_err("an outage is not an answer");
    assert_eq!(
        down.code(),
        "probe_unreachable",
        "and a server that is down establishes nothing either way. This is the line OCSP \
         gave up on: never let unreachable read as an answer"
    );

    let refused = ask_with(Ask::at("https://x/q"), Answers::new(403, ""))
        .await
        .expect_err("a refusal is our misconfiguration");
    assert_eq!(
        refused.code(),
        "artifact_invalid",
        "401/403 is not the remote failing and not the fact being absent -- it is our \
         credentials being wrong, which no amount of retrying fixes"
    );
}

#[tokio::test]
async fn a_selector_that_matches_nothing_is_absent_not_broken() {
    let body = r#"{"crate":{"max_stable_version":"1.2.3"}}"#;
    let got = ask_with(
        Ask::at("https://x/q").selecting("$.crate.max_stable_version"),
        Answers::new(200, body),
    )
    .await
    .unwrap();
    assert_eq!(
        got,
        Outcome::Found {
            facts: gmr_core::Facts::new(serde_json::json!({ "value": "1.2.3" }))
        },
        "the declared path is what this probe reports, under the one field name this \
         transport exports as `VALUE`. It is named rather than bare so the probe's obs is \
         an object like every other probe's, and so `unmet` can check a shape's reads \
         against a declaration that says `facts = [\"value\"]`"
    );

    let missing = ask_with(
        Ask::at("https://x/q").selecting("$.crate.yanked_at"),
        Answers::new(200, body),
    )
    .await
    .unwrap();
    assert_eq!(
        missing,
        Outcome::NotFound,
        "the endpoint answered and the field is not in the answer. That is the fact being \
         absent, the same as a file that exists without the symbol in it -- not an error \
         about our selector, which we cannot tell apart from here anyway"
    );
}

#[tokio::test]
async fn a_body_that_is_not_json_is_unusable_and_a_huge_one_is_not_truncated() {
    let junk = ask_with(Ask::at("https://x/q"), Answers::new(200, "<html>nope"))
        .await
        .expect_err("html is not an answer we can use");
    assert_eq!(junk.code(), "probe_invalid_json");

    let mut asks = BTreeMap::new();
    asks.insert(ProbeName::new("quote"), Ask::at("https://x/q"));
    let http = Http::with(asks, Answers::new(200, &"x".repeat(500)));
    let budget = Budget::within(Duration::from_secs(5), 100);
    let probe = probe();
    let big = http
        .invoke(&ProbeCall {
            probe: &probe,
            position: &serde_json::Value::Null,
            budget: &budget,
        })
        .await
        .expect_err("a body past the cap is refused, never trimmed");
    assert_eq!(
        big.code(),
        "probe_output_too_large",
        "storing a truncated reading as fact would be a lie"
    );
}

#[test]
fn the_version_is_earned_from_what_decides_the_answer_and_from_nothing_else() {
    let base = Ask::at("https://x/q").selecting("$.last");
    assert_eq!(
        base.version(),
        Ask::at("https://x/q").selecting("$.last").version(),
        "the same declaration is the same instrument"
    );
    assert_ne!(
        base.version(),
        Ask::at("https://x/other").selecting("$.last").version(),
        "a different url answers a different question"
    );
    assert_ne!(
        base.version(),
        Ask::at("https://x/q").selecting("$.first").version(),
        "and so does a different selector -- rule 5 says the hash covers everything that \
         can change the output, and the selector decides what the output is"
    );

    let by_reference = base
        .clone()
        .with_header("Authorization", Header::FromEnv("TOKEN_A".to_owned()));
    let rotated = base
        .clone()
        .with_header("Authorization", Header::FromEnv("TOKEN_B".to_owned()));
    assert_ne!(
        by_reference.version(),
        rotated.version(),
        "which environment variable the credential comes from is part of the declaration"
    );
    assert_ne!(
        base.version(),
        by_reference.version(),
        "and sending a header at all can change what comes back"
    );

    unsafe { std::env::set_var("TOKEN_A", "first") };
    let before = by_reference.version();
    unsafe { std::env::set_var("TOKEN_A", "second") };
    assert_eq!(
        by_reference.version(),
        before,
        "rotating the secret must not move the instrument's identity. The credential is \
         held as a reference and the value is never read here, so this holds by \
         construction -- and it has to: a version that moved when a token was rotated \
         would report every anchor behind that endpoint as read by a different instrument, \
         and `Incomparable` would bury the corpus on the day somebody did the responsible \
         thing"
    );
    unsafe { std::env::remove_var("TOKEN_A") };
}

#[tokio::test]
async fn a_credential_reaches_the_request_and_never_the_error_text() {
    unsafe { std::env::set_var("GMR_TEST_TOKEN", "s3cret-value") };
    let seen = Answers::new(500, "");
    let err = ask_with(
        Ask::at("https://x/q").with_header(
            "Authorization",
            Header::FromEnv("GMR_TEST_TOKEN".to_owned()),
        ),
        seen.clone(),
    )
    .await
    .expect_err("500 is an outage");

    assert_eq!(
        seen.seen.lock().unwrap().as_slice(),
        &[("Authorization".to_owned(), "s3cret-value".to_owned())],
        "the value is resolved from the environment at call time and sent"
    );
    assert!(
        !err.to_string().contains("s3cret-value"),
        "and it must never appear in what we report. A probe error is written to the \
         journal verbatim, so a secret in this string is a secret committed to an \
         append-only log that nothing can delete: {err}"
    );

    unsafe { std::env::remove_var("GMR_TEST_TOKEN") };
    let unset = ask_with(
        Ask::at("https://x/q").with_header(
            "Authorization",
            Header::FromEnv("GMR_TEST_TOKEN".to_owned()),
        ),
        Answers::new(200, "{}"),
    )
    .await
    .expect_err("a credential we cannot resolve is not something to guess at");
    assert!(
        unset.to_string().contains("GMR_TEST_TOKEN"),
        "the variable's name is the useful half and is safe to say: {unset}"
    );
}

#[test]
fn the_declared_path_is_read_the_way_people_write_it() {
    assert_eq!(
        pointer("$.crate.max_stable_version"),
        "/crate/max_stable_version"
    );
    assert_eq!(
        pointer("crate.max_stable_version"),
        "/crate/max_stable_version"
    );
    assert_eq!(
        pointer("/crate/max_stable_version"),
        "/crate/max_stable_version"
    );
    assert_eq!(pointer("$.last"), "/last");
    assert_eq!(
        pointer("a.b~c"),
        "/a/b~0c",
        "RFC 6901 escapes, because the pointer is serde_json's and not one we invented"
    );
}
