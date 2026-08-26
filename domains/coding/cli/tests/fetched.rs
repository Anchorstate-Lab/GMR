use coding_anchor::probes::Catalog;
use coding_anchor::verbs::sync;

fn a_repository() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".anchor")).unwrap();
    std::fs::create_dir_all(dir.path().join("memories")).unwrap();
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
        .sealer(std::sync::Arc::new(store.bindings()))
        .links(std::sync::Arc::new(store.links()))
        .queue(std::sync::Arc::new(store.queue()))
        .settings(std::sync::Arc::new(store.queue()))
        .sightings(std::sync::Arc::new(store.queue()))
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
