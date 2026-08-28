use coding_anchor::probes::Catalog;
use coding_anchor::verbs::sync;

fn a_repository() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".anchor")).unwrap();
    std::fs::create_dir_all(dir.path().join("memories")).unwrap();
    std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(dir.path())
        .status()
        .expect("git is on PATH in this test environment");
    dir
}

#[tokio::test]
async fn a_fetched_anchor_is_declared_in_the_file_even_when_a_note_carries_its_memory() {
    let dir = a_repository();
    let root = dir.path();

    let store = gmr::sqlite::open(root.join("memory.db")).await.unwrap();
    let rt = gmr::Runtime::builder()
        .journal(std::sync::Arc::new(store.journal()))
        .bindings(std::sync::Arc::new(store.bindings()))
        .sealer(std::sync::Arc::new(store.sealer()))
        .links(std::sync::Arc::new(store.links()))
        .queue(std::sync::Arc::new(store.queue()))
        .settings(std::sync::Arc::new(store.settings()))
        .sightings(std::sync::Arc::new(store.sightings()))
        .transport(std::sync::Arc::new(
            gmr_transport::http::Http::new(coding_anchor::probes::Declared::at(root)).unwrap(),
        ))
        .build();
    let stores = coding_anchor::stores::assembled(root).unwrap();

    let _ = coding_anchor::verbs::anchor::run(
        &rt,
        root,
        &stores,
        coding_anchor::verbs::anchor::Asked {
            coordinate: Some(
                "http://127.0.0.1:1/api/v1/crates/reqwest#$.crate.max_stable_version".to_owned(),
            ),
            named: Some("reqwest-latest".to_owned()),
            memory: Some("We are one breaking line behind, deliberately.".to_owned()),
            record: None,
        },
        true,
    )
    .await;

    let catalog = Catalog::load(root).unwrap();
    assert_eq!(
        catalog.kind_of("reqwest-latest").as_str(),
        "http",
        "a [http.<name>] table is what makes the name an http probe"
    );

    let declared = sync::read_declared(root, sync::DEFAULT_FILE).unwrap();
    let held = declared
        .anchor
        .iter()
        .find(|d| d.key == "reqwest-latest")
        .expect(
            "a fetched anchor MUST be declared in anchors.toml. A path coordinate carries its \
             own routing -- `src/main.rs#get` names a file whose extension picks the probe -- \
             but a name minted from a URL carries none, so the declaration is the only thing \
             that can say which probe and which shape it means. Skipping it (which the -m \
             branch used to do, because a note was written instead) leaves the name to be \
             re-routed as if it were a file path: it falls through to the catchall probe, \
             comes back as the `roster` shape, and reports `absent` forever while looking \
             like a working anchor",
        );
    assert_eq!(held.probe, "reqwest-latest");
    assert_eq!(held.shape.as_deref(), Some("value"));

    let note = std::fs::read_to_string(root.join("memories/reqwest-latest.md")).unwrap();
    assert!(
        note.contains("anchors:") && !note.contains("about:"),
        "and the note points at that declaration rather than asking to be routed itself: {note}"
    );
}

#[test]
fn a_note_on_a_fetched_anchor_points_at_the_declaration_instead_of_re_deriving_it() {
    let dir = a_repository();
    let root = dir.path();
    let note = root.join("memories/reqwest-latest.md");
    std::fs::write(
        &note,
        "---\nanchors:\n  - \"reqwest-latest\"\n---\n\nWe are one line behind.\n",
    )
    .unwrap();

    let text = std::fs::read_to_string(&note).unwrap();
    assert!(
        text.contains("anchors:") && !text.contains("about:"),
        "`about:` asks the reader to route the coordinate itself, which is right for a path \
         and impossible for a minted name. The frontmatter for a fetched anchor names the \
         anchor and lets the declaration do the routing"
    );
}

#[tokio::test]
async fn a_config_value_is_watched_as_a_value_and_not_as_a_hash_of_its_file() {
    let dir = a_repository();
    let root = dir.path();
    std::fs::write(
        root.join("deploy.yaml"),
        "service:\n  replicas: 3\n  timeout_ms: 5000\n",
    )
    .unwrap();

    let store = gmr::sqlite::open(root.join("memory.db")).await.unwrap();
    let stores = coding_anchor::stores::assembled(root).unwrap();
    let mut builder = gmr::Runtime::builder()
        .journal(std::sync::Arc::new(store.journal()))
        .bindings(std::sync::Arc::new(store.bindings()))
        .sealer(std::sync::Arc::new(store.sealer()))
        .links(std::sync::Arc::new(store.links()))
        .queue(std::sync::Arc::new(store.queue()))
        .settings(std::sync::Arc::new(store.settings()))
        .sightings(std::sync::Arc::new(store.sightings()))
        .transport(std::sync::Arc::new(gmr_transport::file::Files::new(
            root,
            coding_anchor::probes::Declared::at(root),
        )));
    for built in &stores.built {
        builder = builder.provider(built.content());
    }
    let rt = builder.build();

    coding_anchor::verbs::anchor::run(
        &rt,
        root,
        &stores,
        coding_anchor::verbs::anchor::Asked {
            coordinate: Some("file://deploy.yaml#$.service.replicas".to_owned()),
            named: None,
            memory: Some("Three, because two cannot survive a rolling restart.".to_owned()),
            record: None,
        },
        true,
    )
    .await
    .unwrap();

    let key = gmr::AnchorKey::new("deploy-replicas");
    let opened = rt.read(&key).await.expect("the anchor opened");
    assert_eq!(
        opened.state.0.pointer("/baseline/value"),
        Some(&serde_json::json!(3)),
        "the baseline is the number somebody chose, not a digest of the file it sits in. \
         Routed to the catchall instead, this same coordinate came back as an opaque \
         fingerprint and reported `path matched, name did not` -- it was never watching \
         `replicas` at all: {}",
        opened.state.0
    );

    std::fs::write(
        root.join("deploy.yaml"),
        "service:\n  replicas: 3\n  timeout_ms: 8000\n",
    )
    .unwrap();
    rt.observe(&key).await.unwrap();
    assert_eq!(
        rt.read(&key).await.unwrap().state.0.pointer("/v/value"),
        Some(&serde_json::json!(false)),
        "an edit to a neighbouring field is not this fact moving, and waking somebody for \
         it is how they learn to stop reading what they are handed"
    );

    std::fs::write(
        root.join("deploy.yaml"),
        "service:\n  replicas: 5\n  timeout_ms: 8000\n",
    )
    .unwrap();
    rt.observe(&key).await.unwrap();
    let moved = rt.read(&key).await.unwrap();
    assert_eq!(
        moved.state.0.pointer("/v/value"),
        Some(&serde_json::json!(true)),
        "and the field it does watch moving is: {}",
        moved.state.0
    );
    assert_eq!(
        moved.state.0.pointer("/now/value"),
        Some(&serde_json::json!(5)),
        "with the new value legible, so a reader sees 3 -> 5 rather than one hash -> another"
    );
}

#[test]
fn every_kind_a_declaration_can_name_is_a_kind_probes_list_shows() {
    let dir = a_repository();
    let root = dir.path();
    std::fs::write(
        root.join(".anchor/probes.toml"),
        r#"
[http.a-http]
url = "https://example.com/x"
select = "$.a"

[file.a-file]
path = "deploy.yaml"
select = "$.a"

[sql.a-sql]
url = "sqlite://app.db"
query = "SELECT 1"

[script.a-script]
run = "probe.sh"
obs = { schema = "gmr.probe.v1", facts = ["v"] }
"#,
    )
    .unwrap();

    let catalog = Catalog::load(root).unwrap();
    let declared: Vec<String> = ["a-http", "a-file", "a-sql", "a-script"]
        .iter()
        .map(|n| catalog.kind_of(n).as_str().to_owned())
        .collect();
    assert_eq!(
        declared,
        vec!["http", "file", "sql", "script"],
        "each table routes to its own kind"
    );

    let listed = coding_anchor::verbs::probes::rows(root).unwrap();
    let by_name: std::collections::BTreeMap<&str, &str> = listed
        .iter()
        .map(|(name, kind)| (name.as_str(), *kind))
        .collect();

    for (name, kind) in [
        ("a-http", "http"),
        ("a-file", "file"),
        ("a-sql", "sql"),
        ("a-script", "script"),
    ] {
        assert_eq!(
            by_name.get(name),
            Some(&kind),
            "`gmr probes` answers what this build can reach, and a declaration it cannot \\
             show is a probe that works while being invisible. Three kinds were added and \\
             all three were missed here, because nothing connected `kind_of` gaining a \\
             branch to this list gaining a loop -- this assertion is that connection. \\
             Listed: {by_name:?}"
        );
    }
}
