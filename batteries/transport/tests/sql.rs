use std::collections::BTreeMap;
use std::time::Duration;

use gmr_budget::Budget;
use gmr_core::{Kind, Outcome, ProbeName, ProbeRef};
use gmr_probe::{ProbeCall, ProbeError, Transport};
use gmr_transport::sql::{Ask, Source, Sql};

async fn a_database(dir: &std::path::Path) -> String {
    let at = dir.join("app.db");
    let url = format!("sqlite://{}", at.display());
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(&at)
                .create_if_missing(true),
        )
        .await
        .unwrap();
    sqlx::query("CREATE TABLE migrations (version TEXT, applied_at INTEGER, note TEXT)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO migrations VALUES ('0042_add_index', 1700000000, NULL)")
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
    url
}

async fn run(ask: Ask) -> Result<Outcome, ProbeError> {
    let mut asks = BTreeMap::new();
    asks.insert(ProbeName::new("db"), ask);
    let sql = Sql::new(asks);
    let budget = Budget::within(Duration::from_secs(10), 1 << 20);
    let probe = ProbeRef::new(
        Kind::new("sql"),
        ProbeName::new("db"),
        serde_json::Value::Null,
    );
    sql.invoke(&ProbeCall {
        probe: &probe,
        position: &serde_json::Value::Null,
        budget: &budget,
    })
    .await
}

fn value(v: serde_json::Value) -> Outcome {
    Outcome::Found {
        facts: gmr_core::Facts::new(serde_json::json!({ "value": v })),
    }
}

#[tokio::test]
async fn one_row_is_a_fact_and_no_rows_is_the_database_answering() {
    let dir = tempfile::tempdir().unwrap();
    let url = a_database(dir.path()).await;
    let on = |q: &str| Ask::on(Source::Given(url.clone()), q);

    assert_eq!(
        run(on("SELECT version FROM migrations")).await.unwrap(),
        value(serde_json::json!("0042_add_index")),
        "one row, one column, one fact"
    );
    assert_eq!(
        run(on("SELECT applied_at FROM migrations")).await.unwrap(),
        value(serde_json::json!(1_700_000_000i64)),
        "an INTEGER comes back a number, not the text of one -- a state comparison is by \
         value, so a number that arrives as a string never equals the one that arrives as \
         a number"
    );
    assert_eq!(
        run(on("SELECT note FROM migrations")).await.unwrap(),
        Outcome::NotFound,
        "a NULL column is the row saying it holds nothing there"
    );
    assert_eq!(
        run(on("SELECT version FROM migrations WHERE version = 'nope'"))
            .await
            .unwrap(),
        Outcome::NotFound,
        "and no rows is the database answering that there is no such fact -- as definite as \
         a 404, and filed as an error it would have the anchor back off and retry something \
         settled"
    );
}

#[tokio::test]
async fn a_probe_observes_and_may_not_write() {
    let dir = tempfile::tempdir().unwrap();
    let url = a_database(dir.path()).await;

    let err = run(Ask::on(
        Source::Given(url.clone()),
        "UPDATE migrations SET version = 'tampered'",
    ))
    .await
    .expect_err("a probe that can write can change the fact it is reporting on");
    assert_eq!(err.code(), "probe_unusable");

    assert_eq!(
        run(Ask::on(
            Source::Given(url.clone()),
            "SELECT version FROM migrations"
        ))
        .await
        .unwrap(),
        value(serde_json::json!("0042_add_index")),
        "and the refusal left the row alone. The connection is opened read-only, so this \
         is enforced by the driver rather than by hoping declarations only ever SELECT -- \
         a probe whose reading can move the world it reads makes every anchor downstream \
         of it meaningless"
    );
}

#[tokio::test]
async fn what_the_database_cannot_answer_and_what_it_will_not_are_different_people() {
    let dir = tempfile::tempdir().unwrap();
    let url = a_database(dir.path()).await;

    let refused = run(Ask::on(
        Source::Given(url.clone()),
        "SELECT nope FROM migrations",
    ))
    .await
    .expect_err("a query the schema cannot satisfy is ours to fix");
    assert_eq!(
        refused.code(),
        "probe_unusable",
        "the declaration assumed a schema that is not there. Retrying never fixes it, so it \
         must not read as an outage"
    );

    let many = run(Ask::on(
        Source::Given(url.clone()),
        "SELECT version FROM migrations UNION ALL SELECT 'second'",
    ))
    .await
    .expect_err("two rows is not one fact");
    assert_eq!(many.code(), "probe_unusable");

    let gone = run(Ask::on(
        Source::Given(format!("sqlite://{}/nowhere.db", dir.path().display())),
        "SELECT 1",
    ))
    .await
    .expect_err("a database we cannot open is not an answer about the row");
    assert_eq!(
        gone.code(),
        "probe_unreachable",
        "a missing database is NOT the `NotFound` a missing file is. A file that is not \
         there settles what the config says; a database that is not there settles nothing \
         about the row -- we never got to ask. Reporting absence here is the OCSP mistake"
    );
}

#[tokio::test]
async fn a_named_column_is_taken_and_several_unnamed_ones_come_back_together() {
    let dir = tempfile::tempdir().unwrap();
    let url = a_database(dir.path()).await;

    assert_eq!(
        run(Ask::on(
            Source::Given(url.clone()),
            "SELECT version, applied_at FROM migrations"
        )
        .taking("applied_at"))
        .await
        .unwrap(),
        value(serde_json::json!(1_700_000_000i64))
    );

    assert_eq!(
        run(Ask::on(
            Source::Given(url.clone()),
            "SELECT version, applied_at FROM migrations"
        ))
        .await
        .unwrap(),
        value(serde_json::json!({"version": "0042_add_index", "applied_at": 1_700_000_000i64})),
        "several columns and nobody said which: the row comes back whole rather than one of \
         them being picked by position, which would silently change meaning when somebody \
         reorders the SELECT"
    );

    let missing =
        run(Ask::on(Source::Given(url), "SELECT version FROM migrations").taking("applied_at"))
            .await
            .expect_err("naming a column the query does not return is a broken declaration");
    assert_eq!(missing.code(), "probe_unusable");
    assert!(
        missing.to_string().contains("version"),
        "and it says what the query does return: {missing}"
    );
}

#[test]
fn the_version_is_earned_from_the_query_and_never_from_the_secret() {
    let base = || Ask::on(Source::Given("sqlite://app.db".to_owned()), "SELECT 1");
    assert_eq!(base().version(), base().version());
    assert_ne!(
        base().version(),
        Ask::on(Source::Given("sqlite://app.db".to_owned()), "SELECT 2").version(),
        "the query decides the answer, so it is in"
    );
    assert_ne!(
        base().version(),
        base().taking("v").version(),
        "and so does which column is taken"
    );

    let by_reference = Ask::on(Source::FromEnv("DATABASE_URL".to_owned()), "SELECT 1");
    unsafe { std::env::set_var("DATABASE_URL", "postgres://u:first@host/db") };
    let before = by_reference.version();
    unsafe { std::env::set_var("DATABASE_URL", "postgres://u:rotated@host/db") };
    assert_eq!(
        by_reference.version(),
        before,
        "a connection url carries the password. Held by reference it is never read while \
         deciding what the instrument is, so rotating the credential does not report every \
         anchor behind this database as read by a different instrument"
    );
    unsafe { std::env::remove_var("DATABASE_URL") };

    assert_ne!(
        by_reference.version(),
        Ask::on(Source::FromEnv("OTHER_URL".to_owned()), "SELECT 1").version(),
        "which variable it comes from is part of the declaration"
    );
}

#[tokio::test]
async fn a_credential_that_cannot_be_resolved_says_the_variable_and_not_a_guess() {
    unsafe { std::env::remove_var("GMR_TEST_DB") };
    let err = run(Ask::on(
        Source::FromEnv("GMR_TEST_DB".to_owned()),
        "SELECT 1",
    ))
    .await
    .expect_err("an unset connection is not something to guess at");
    assert!(
        err.to_string().contains("GMR_TEST_DB"),
        "the variable's name is the useful half and is safe to say: {err}"
    );
}
