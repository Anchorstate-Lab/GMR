use std::collections::BTreeMap;
use std::time::Duration;

use gmr_budget::Budget;
use gmr_core::{Kind, Outcome, ProbeName, ProbeRef};
use gmr_probe::{ProbeCall, ProbeError, Transport};
use gmr_transport::file::{Ask, Files, Shaped, inside};

fn world(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (name, body) in files {
        let at = dir.path().join(name);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(at, body).unwrap();
    }
    dir
}

async fn ask_in(root: &std::path::Path, ask: Ask) -> Result<Outcome, ProbeError> {
    let mut asks = BTreeMap::new();
    asks.insert(ProbeName::new("cfg"), ask);
    let files = Files::new(root, asks);
    let budget = Budget::within(Duration::from_secs(5), 1 << 20);
    let probe = ProbeRef::new(
        Kind::new("file"),
        ProbeName::new("cfg"),
        serde_json::Value::Null,
    );
    files
        .invoke(&ProbeCall {
            probe: &probe,
            position: &serde_json::Value::Null,
            budget: &budget,
        })
        .await
}

#[tokio::test]
async fn a_declared_field_is_read_out_of_yaml_toml_and_json_alike() {
    let dir = world(&[
        (
            "deploy.yaml",
            "service:\n  replicas: 3\n  timeout_ms: 5000\n",
        ),
        (
            "Cargo.toml",
            "[package]\nname = \"x\"\nversion = \"1.2.3\"\n",
        ),
        ("pkg.json", r#"{"engines":{"node":"22"}}"#),
    ]);
    let root = dir.path();

    for (path, select, want) in [
        ("deploy.yaml", "$.service.replicas", serde_json::json!(3)),
        (
            "Cargo.toml",
            "$.package.version",
            serde_json::json!("1.2.3"),
        ),
        ("pkg.json", "$.engines.node", serde_json::json!("22")),
    ] {
        assert_eq!(
            ask_in(root, Ask::at(path).selecting(select)).await.unwrap(),
            Outcome::Found {
                facts: gmr_core::Facts::new(serde_json::json!({ "value": want }))
            },
            "`{path}` at `{select}` is one value, not a hash of the whole file. That is the \
             whole point of this probe family: the catchall reports a fingerprint, so a \
             memory about `replicas` fires when `timeout_ms` moves and never tracks the \
             number it is about"
        );
    }
}

#[tokio::test]
async fn a_neighbouring_field_moving_is_not_this_fact_moving() {
    let dir = world(&[(
        "deploy.yaml",
        "service:\n  replicas: 3\n  timeout_ms: 5000\n",
    )]);
    let root = dir.path();
    let ask = || Ask::at("deploy.yaml").selecting("$.service.replicas");

    let before = ask_in(root, ask()).await.unwrap();
    std::fs::write(
        root.join("deploy.yaml"),
        "service:\n  replicas: 3\n  timeout_ms: 8000\n",
    )
    .unwrap();
    assert_eq!(
        ask_in(root, ask()).await.unwrap(),
        before,
        "the file changed and this fact did not. An anchor watching `replicas` must not be \
         woken by an edit to `timeout_ms` -- being woken for nothing is how a person learns \
         to stop reading what they are handed"
    );
}

#[tokio::test]
async fn a_file_that_is_not_there_is_the_worlds_answer_and_an_unreadable_one_is_ours() {
    let dir = world(&[("deploy.yaml", "service:\n  replicas: 3\n")]);
    let root = dir.path();

    assert_eq!(
        ask_in(root, Ask::at("gone.yaml").selecting("$.a"))
            .await
            .unwrap(),
        Outcome::NotFound,
        "the filesystem answered, and the answer is that it is not there. That is as \
         definite as a 404 and must not be filed under our failures, or the anchor backs \
         off and retries a settled fact"
    );

    assert_eq!(
        ask_in(root, Ask::at("deploy.yaml").selecting("$.service.tls"))
            .await
            .unwrap(),
        Outcome::NotFound,
        "a field that is not in a file that is there is absent, not broken"
    );

    let junk = world(&[("bad.yaml", "service:\n  - [unclosed\n")]);
    let err = ask_in(junk.path(), Ask::at("bad.yaml").selecting("$.a"))
        .await
        .expect_err("a file we cannot parse is not a fact");
    assert_eq!(
        err.code(),
        "probe_unusable",
        "unparseable is ours to fix, and is not the same as the field being absent"
    );
}

#[test]
fn a_declaration_may_not_read_outside_the_tree() {
    let root = std::path::Path::new("/repo");
    assert!(inside(root, "config/deploy.yaml").is_some());
    assert!(inside(root, "./config/../deploy.yaml").is_some());
    for escape in [
        "../secrets.yaml",
        "config/../../secrets.yaml",
        "/etc/passwd",
        "/Users/someone/.aws/credentials",
    ] {
        assert!(
            inside(root, escape).is_none(),
            "`{escape}` leaves the tree. Declarations are reviewed, which is the authorization \
             model -- but what a `file` probe reads goes verbatim into an append-only log, so \
             a path that can walk out of the repository is a way to commit a host secret that \
             nothing can later delete"
        );
    }
}

fn resolved(root: &std::path::Path, ask: Ask) -> gmr_core::ProbeVersion {
    let mut asks = BTreeMap::new();
    asks.insert(ProbeName::new("cfg"), ask);
    Files::new(root, asks)
        .resolve(&ProbeName::new("cfg"))
        .expect("a declared probe resolves")
        .version
}

#[test]
fn the_version_is_the_declaration_and_not_the_contents() {
    let ask = || Ask::at("deploy.yaml").selecting("$.service.replicas");
    let three = world(&[("deploy.yaml", "service:\n  replicas: 3\n")]);
    let nine = world(&[("deploy.yaml", "service:\n  replicas: 9\n")]);

    assert_eq!(
        resolved(three.path(), ask()),
        resolved(nine.path(), ask()),
        "the file's contents are the FACT, not the instrument. Hashing them would move the \
         probe version every time the thing being watched moved, and every reading would \
         come back `Incomparable` -- the anchor could never once say the value changed, \
         which is the only thing it exists to say. `script` hashes its file because there \
         the file is the instrument; here it is the subject"
    );

    assert_ne!(
        ask().version(),
        Ask::at("deploy.yaml")
            .selecting("$.service.timeout_ms")
            .version(),
        "the selector decides the answer, so it is in"
    );
    assert_ne!(
        ask().version(),
        Ask::at("other.yaml")
            .selecting("$.service.replicas")
            .version(),
        "and so does which file"
    );
    assert_ne!(
        Ask::at("f.json")
            .selecting("$.a")
            .shaped_as(Shaped::Yaml)
            .version(),
        Ask::at("f.json").selecting("$.a").version(),
        "reading the same bytes as a different format is a different instrument"
    );
}

#[test]
fn nothing_here_can_reach_the_file_while_deciding_what_the_instrument_is() {
    let ask = Ask::at("deploy.yaml").selecting("$.service.replicas");
    let _ = ask.version();
    assert!(
        ask.path == "deploy.yaml",
        "`Ask` holds a relative path and no root -- the root lives on `Files`. So `version()`          has nothing to open even if someone tried, which is the structural half of the          guarantee the test above checks behaviourally. If a root ever lands on `Ask`,          hashing the contents becomes one line away and every reading of a moved value          starts coming back `Incomparable`"
    );
}
