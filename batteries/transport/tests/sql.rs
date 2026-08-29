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
    at(ask, serde_json::Value::Null).await
}

async fn at(ask: Ask, position: serde_json::Value) -> Result<Outcome, ProbeError> {
    within(ask, position, Duration::from_secs(10)).await
}

async fn within(
    ask: Ask,
    position: serde_json::Value,
    span: Duration,
) -> Result<Outcome, ProbeError> {
    let mut asks = BTreeMap::new();
    asks.insert(ProbeName::new("db"), ask);
    let sql = Sql::new(asks);
    let budget = Budget::within(span, 1 << 20);
    let probe = ProbeRef::new(
        Kind::new("sql"),
        ProbeName::new("db"),
        serde_json::Value::Null,
    );
    sql.invoke(&ProbeCall {
        probe: &probe,
        position: &position,
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

#[tokio::test]
async fn a_backend_this_build_cannot_speak_is_ours_to_fix_and_not_an_outage() {
    let err = run(Ask::on(
        Source::Given("mysql://host:3306/db".to_owned()),
        "SELECT 1",
    ))
    .await
    .expect_err("nothing here speaks mysql");
    assert_ne!(
        err.code(),
        "probe_unreachable",
        "sqlx's sqlite parser takes ANY string as a filename, so an unrouted url becomes a \
         file by that name and failing to open it read as `Unreachable` -- a transient \
         outage the anchor backs off and retries forever, for a thing that can never work. \
         Which code it carries matters less than which side of that line it falls on"
    );
    assert_eq!(
        err.code(),
        "artifact_invalid",
        "and the artifact a declared probe has is its declaration, which is what is wrong \
         here -- the same code `http` gives an unresolvable one"
    );
}

#[tokio::test]
async fn a_reading_too_large_to_store_is_refused_rather_than_trimmed() {
    let dir = tempfile::tempdir().unwrap();
    let url = a_database(dir.path()).await;
    let mut asks = BTreeMap::new();
    asks.insert(
        ProbeName::new("db"),
        Ask::on(
            Source::Given(url),
            "SELECT printf('%.*c', 5000, 'x') AS blob",
        ),
    );
    let sql = Sql::new(asks);
    let probe = ProbeRef::new(
        Kind::new("sql"),
        ProbeName::new("db"),
        serde_json::Value::Null,
    );
    let err = sql
        .invoke(&ProbeCall {
            probe: &probe,
            position: &serde_json::Value::Null,
            budget: &Budget::within(Duration::from_secs(10), 100),
        })
        .await
        .expect_err("a row past the cap is refused, never trimmed");
    assert_eq!(
        err.code(),
        "probe_output_too_large",
        "http and file both cap what they will store and this must too -- storing a \
         truncated reading as fact would be a lie, and a query is the easiest of the three \
         to point at something enormous"
    );
}

#[test]
fn a_local_database_is_not_a_remote_system() {
    let mut asks = BTreeMap::new();
    asks.insert(
        ProbeName::new("db"),
        Ask::on(Source::Given("sqlite://app.db".to_owned()), "SELECT 1"),
    );
    let d = Sql::new(asks).resolve(&ProbeName::new("db")).unwrap();
    assert_eq!(
        d.verifiability,
        gmr_core::Verifiability::Closed,
        "sqlite is a local file. Claiming `Network` openness says a remote system we cannot \
         inspect took part, which is false -- and it is the opposite call from `file`, which \
         reads a local file and is `Closed`, and from the extractors, which do the same. \
         Openness must describe what actually happened; over-claiming it is not the safe \
         direction, it is a wrong answer that a reader has no way to check"
    );
}

#[tokio::test]
async fn a_connection_url_that_carries_a_credential_is_refused_rather_than_opened() {
    let err = run(Ask::on(
        Source::Given("postgres://svc:hunter2@db.internal/app".to_owned()),
        "SELECT 1",
    ))
    .await
    .expect_err("a password written into a declaration is a declaration to fix");

    assert_eq!(
        err.code(),
        "artifact_invalid",
        "not an outage to retry: nothing about the database decides this: {err}"
    );
    assert!(
        !err.to_string().contains("hunter2"),
        "and saying it back is the leak this refusal exists to prevent: {err}"
    );
    assert!(
        err.to_string().contains("environment variable"),
        "the refusal has to say what to do instead, or it is a wall: {err}"
    );
}

#[tokio::test]
async fn a_database_reached_by_reference_does_not_have_its_url_quoted_back() {
    let dir = tempfile::tempdir().unwrap();
    let missing = format!("sqlite://{}", dir.path().join("no-such.db").display());
    unsafe { std::env::set_var("GMR_TEST_ABSENT_DB", &missing) };

    let err = run(Ask::on(
        Source::FromEnv("GMR_TEST_ABSENT_DB".to_owned()),
        "SELECT 1",
    ))
    .await
    .expect_err("a database that is not there cannot be opened");

    unsafe { std::env::remove_var("GMR_TEST_ABSENT_DB") };

    assert!(
        !err.to_string().contains(&missing),
        "the driver quotes the connection string it was handed, and for a url held by \
         reference that string is whatever the variable holds -- a password included. \
         What reaches the journal must be the variable's name and the fact that it \
         failed: {err}"
    );
    assert!(
        err.to_string().contains("GMR_TEST_ABSENT_DB") && err.to_string().contains("db"),
        "and both names a reader needs are safe to say: which probe, and which \
         variable: {err}"
    );
}

#[tokio::test]
async fn the_position_reaches_the_query_as_a_bound_value_and_never_as_text() {
    let dir = tempfile::tempdir().unwrap();
    let url = a_database(dir.path()).await;
    let ask = Ask::on(
        Source::Given(url.clone()),
        "SELECT applied_at FROM migrations WHERE version = ?1",
    )
    .binding("version");

    assert_eq!(
        at(
            ask.clone(),
            serde_json::json!({ "version": "0042_add_index" })
        )
        .await
        .unwrap(),
        value(serde_json::json!(1700000000)),
        "one declaration answers for every migration somebody anchors"
    );
    assert_eq!(
        at(ask.clone(), serde_json::json!({ "version": "0001_init" }))
            .await
            .unwrap(),
        Outcome::NotFound,
        "and a row that is not there is the database answering, not a broken probe"
    );

    assert_eq!(
        at(
            ask.clone(),
            serde_json::json!({ "version": "x' OR 1=1 --" }),
        )
        .await
        .unwrap(),
        Outcome::NotFound,
        "a position is data all the way down. Pasted into the text this would return every \
         row and the probe would then refuse the answer for having too many -- looking, from \
         the outside, exactly like a fact that had moved"
    );
}

#[tokio::test]
async fn what_a_query_binds_is_part_of_the_instrument_and_what_fills_it_is_not() {
    let base = || {
        Ask::on(
            Source::Given("sqlite://app.db".to_owned()),
            "SELECT v FROM t WHERE k = ?1",
        )
    };
    assert_ne!(
        base().version(),
        base().binding("k").version(),
        "a query that reads its parameter from the position is a different instrument from \
         one that does not, even with the same text"
    );
    assert_ne!(
        base().binding("k").version(),
        base().binding("other").version(),
        "and so is one that reads a different field"
    );
    assert_eq!(
        base().binding("k").version(),
        base().binding("k").version(),
        "what the field holds at any moment is the position, which is where the probe is \
         pointed and never what the probe is"
    );
}

#[tokio::test]
async fn a_bound_name_the_position_cannot_fill_is_ours_to_fix_and_not_an_absence() {
    let dir = tempfile::tempdir().unwrap();
    let url = a_database(dir.path()).await;
    let err = at(
        Ask::on(
            Source::Given(url),
            "SELECT applied_at FROM migrations WHERE version = ?1",
        )
        .binding("version"),
        serde_json::json!({ "release": "0042_add_index" }),
    )
    .await
    .expect_err("the declaration and the position disagree about what is watched");

    assert_eq!(
        err.code(),
        "artifact_invalid",
        "NotFound would say the database answered and there is no such row; nothing was \
         asked at all: {err}"
    );
    assert!(
        err.to_string().contains("version") && err.to_string().contains("release"),
        "the message has to show both halves of the disagreement: {err}"
    );
}

#[tokio::test]
async fn a_second_backend_is_a_branch_on_the_same_decision_and_not_a_way_around_it() {
    use gmr_transport::sql::{Spoken, spoken};

    assert_eq!(spoken("app.db"), Some(Spoken::Sqlite));
    assert_eq!(spoken("sqlite://app.db"), Some(Spoken::Sqlite));
    assert_eq!(spoken("SQLITE://app.db"), Some(Spoken::Sqlite));
    assert_eq!(spoken("postgres://host/db"), Some(Spoken::Postgres));
    assert_eq!(spoken("postgresql://host/db"), Some(Spoken::Postgres));
    assert_eq!(
        spoken("mysql://host/db"),
        None,
        "a scheme nothing here speaks is not silently treated as a filename, which is what \
         a boolean `is this sqlite` could only ever do"
    );

    let closed = |url: &str| {
        Sql::new(BTreeMap::from([(
            ProbeName::new("db"),
            Ask::on(Source::Given(url.to_owned()), "SELECT 1"),
        )]))
        .resolve(&ProbeName::new("db"))
        .unwrap()
        .verifiability
    };
    assert_eq!(
        closed("app.db"),
        gmr_core::Verifiability::Closed,
        "a local file is a closed reading"
    );
    assert_eq!(
        closed("postgres://host/db"),
        gmr_core::Verifiability::open([gmr_core::Openness::Network, gmr_core::Openness::Clock]),
        "and a database across a network is not -- the second backend does not get to be \
         quieter about that than the first"
    );
}

#[tokio::test]
async fn a_database_nobody_answers_for_is_an_outage_and_never_an_absent_fact() {
    let err = within(
        Ask::on(
            Source::Given("postgres://127.0.0.1:1/app".to_owned()),
            "SELECT version FROM migrations",
        ),
        serde_json::Value::Null,
        Duration::from_millis(250),
    )
    .await
    .expect_err("nothing is listening on port 1");

    assert_eq!(
        err.code(),
        "probe_unreachable",
        "NotFound would say the database answered and there is no such row. Nothing \
         answered at all, and an anchor that folds silence into absence reports a fact as \
         gone the first time a network blinks: {err}"
    );
}

#[tokio::test]
async fn the_second_backend_refuses_a_credential_in_its_url_like_the_first() {
    let err = run(Ask::on(
        Source::Given("postgres://svc:hunter2@127.0.0.1:1/app".to_owned()),
        "SELECT 1",
    ))
    .await
    .expect_err("a password written into a declaration is a declaration to fix");

    assert_eq!(err.code(), "artifact_invalid", "{err}");
    assert!(
        !err.to_string().contains("hunter2"),
        "the refusal must not repeat it, and a new backend does not get a new answer to \
         that question: {err}"
    );

    let named = run(Ask::on(
        Source::Given("postgres://svc@127.0.0.1:1/app".to_owned()),
        "SELECT 1",
    ))
    .await
    .expect_err("a bare username before the host is userinfo too");
    assert_eq!(
        named.code(),
        "artifact_invalid",
        "which for postgres means a connection string is declared by reference or not at \
         all. That is the right end of the trade: a DSN naming its user today is a DSN \
         carrying its password tomorrow, and the second spelling is one character away from \
         the first: {named}"
    );
}

fn a_postgres() -> Option<String> {
    std::env::var("GMR_TEST_POSTGRES_URL")
        .ok()
        .filter(|v| !v.is_empty())
}

async fn a_schema(url: &str, table: &str) -> sqlx::PgPool {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(url)
        .await
        .expect("GMR_TEST_POSTGRES_URL names a database this test can reach");
    for statement in [
        format!("DROP TABLE IF EXISTS {table}"),
        format!(
            "CREATE TABLE {table} (version TEXT, applied_at BIGINT, shipped BOOLEAN, note TEXT)"
        ),
        format!("INSERT INTO {table} VALUES ('0042_add_index', 1700000000, true, NULL)"),
    ] {
        sqlx::query(&statement).execute(&pool).await.unwrap();
    }
    pool
}

#[tokio::test]
#[ignore = "needs GMR_TEST_POSTGRES_URL"]
async fn a_fact_in_postgres_is_read_the_way_one_in_sqlite_is() {
    let Some(url) = a_postgres() else { return };
    let _ = a_schema(&url, "read_the_way").await;
    let on = |q: &str| Ask::on(Source::FromEnv("GMR_TEST_POSTGRES_URL".to_owned()), q);

    assert_eq!(
        run(on(
            "SELECT applied_at FROM read_the_way WHERE version = '0042_add_index'"
        ))
        .await
        .unwrap(),
        value(serde_json::json!(1700000000i64)),
        "a BIGINT is a number, not the string a driver would hand back by default -- state \
         is compared by value, and \"1700000000\" never equals 1700000000"
    );
    assert_eq!(
        run(on("SELECT shipped FROM read_the_way")).await.unwrap(),
        value(serde_json::json!(true))
    );
    assert_eq!(
        run(on("SELECT note FROM read_the_way")).await.unwrap(),
        Outcome::NotFound,
        "a NULL is the database saying there is nothing there"
    );
    assert_eq!(
        run(on(
            "SELECT version FROM read_the_way WHERE version = 'nope'"
        ))
        .await
        .unwrap(),
        Outcome::NotFound
    );

    assert_eq!(
        at(
            on("SELECT applied_at FROM read_the_way WHERE version = $1").binding("version"),
            serde_json::json!({ "version": "0042_add_index" }),
        )
        .await
        .unwrap(),
        value(serde_json::json!(1700000000i64)),
        "and the position reaches the query as a bound value here too"
    );
}

#[tokio::test]
#[ignore = "needs GMR_TEST_POSTGRES_URL"]
async fn a_postgres_probe_observes_and_may_not_write() {
    let Some(url) = a_postgres() else { return };
    let pool = a_schema(&url, "may_not_write").await;

    let err = run(Ask::on(
        Source::FromEnv("GMR_TEST_POSTGRES_URL".to_owned()),
        "UPDATE may_not_write SET note = 'probe wrote this'",
    ))
    .await
    .expect_err("a probe observes");
    assert_eq!(
        err.code(),
        "probe_unusable",
        "sqlite gets this from the driver's own read_only flag. Postgres has no such flag, \
         so the session says so to the server and the server refuses -- which is the same \
         kind of enforcement and not a promise this crate makes to itself: {err}"
    );

    let note: Option<String> = sqlx::query_scalar("SELECT note FROM may_not_write")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(note, None, "and nothing moved");
}

#[tokio::test]
#[ignore = "needs GMR_TEST_POSTGRES_URL"]
async fn a_column_this_build_cannot_read_says_so_instead_of_inventing_a_shape() {
    let Some(url) = a_postgres() else { return };
    let _ = a_schema(&url, "cannot_read").await;

    let err = run(Ask::on(
        Source::FromEnv("GMR_TEST_POSTGRES_URL".to_owned()),
        "SELECT now() AS observed",
    ))
    .await
    .expect_err("a timestamptz has no reading here");
    assert!(
        err.to_string().contains("observed") && err.to_string().contains("cast"),
        "naming the column and what to do about it, because a Null here would be this \
         transport reporting something the database did not say: {err}"
    );
}

#[tokio::test]
#[ignore = "needs GMR_TEST_POSTGRES_URL"]
async fn a_declaration_names_where_and_the_environment_names_who() {
    let Some(url) = a_postgres() else { return };
    let _ = a_schema(&url, "where_and_who").await;

    let parsed = url.strip_prefix("postgres://").unwrap();
    let (who, at) = parsed
        .split_once('@')
        .expect("the test url carries a credential");
    let (user, password) = who.split_once(':').unwrap_or((who, ""));
    unsafe {
        std::env::set_var("PGUSER", user);
        std::env::set_var("PGPASSWORD", password);
    }

    let found = run(Ask::on(
        Source::Given(format!("postgres://{at}")),
        "SELECT applied_at FROM where_and_who",
    ))
    .await;

    unsafe {
        std::env::remove_var("PGUSER");
        std::env::remove_var("PGPASSWORD");
    }

    assert_eq!(
        found.unwrap(),
        value(serde_json::json!(1700000000i64)),
        "a coordinate typed at a terminal cannot say `from_env`, and this is why it does \
         not have to: the declaration carries host, port and database -- none of them a \
         secret -- and PGUSER/PGPASSWORD carry the rest, which is postgres's own answer to \
         the same question rather than a syntax invented here"
    );
}
