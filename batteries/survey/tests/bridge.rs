use std::collections::BTreeMap;
use std::time::Duration;

use gmr_probe::Budget;
use gmr_survey::bridge::Bridge;
use gmr_survey::matching::Fragment;
use gmr_survey::recipe::{Merge, Recipe, look};
use gmr_survey::testkit::Surveyed;

fn collect(rel: &str, bytes: &[u8], out: &mut Vec<Fragment>) -> Result<(), String> {
    let body = String::from_utf8_lossy(bytes).trim().to_owned();
    let coord: BTreeMap<String, String> = [
        ("file".to_owned(), rel.to_owned()),
        ("name".to_owned(), body.clone()),
    ]
    .into();
    out.push(Fragment::new(
        format!("{rel}#{body}"),
        coord,
        serde_json::json!({ "bytes": bytes.len() }),
    ));
    Ok(())
}

fn anything(_: &str) -> bool {
    true
}

const RECIPE: Recipe = Recipe {
    name: "bridge-test",
    version: "v1",
    items: &["file", "name"],
    narrows_on: &["file", "name"],
    eligible: anything,
    collect,
    merge: Merge::Concat,
    barren: "holds nothing this probe can read",
};

fn tree(files: &[(&str, &str)]) -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    for (path, body) in files {
        let p = d.path().join(path);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, body).unwrap();
    }
    d
}

fn roomy() -> Budget {
    Budget::within(Duration::from_secs(60), 1 << 20)
}

// Deliberately a plain #[test], not #[tokio::test]: the whole point of the
// bridge is that it must not assume the calling thread is inside a tokio
// runtime at all. If this test only passed under #[tokio::test], the bridge
// would be quietly depending on exactly the ambient-runtime assumption it
// was built to avoid.
#[test]
fn the_bridge_agrees_with_the_in_memory_reference() {
    let d = tree(&[("a.rs", "alpha"), ("b.rs", "beta")]);
    let pos = serde_json::json!({ "file": "a.rs", "name": "alpha" });

    let surveyed = Surveyed::over(d.path());
    let via_surveyed = look(&RECIPE, "", &pos, &surveyed, &roomy()).unwrap();

    let bridge = Bridge::spawn(d.path(), gmr_survey::sqlite::open_in_memory).unwrap();
    let via_bridge = look(&RECIPE, "", &pos, &bridge, &roomy()).unwrap();

    assert_eq!(
        via_surveyed, via_bridge,
        "the bridge is a translation layer, not a second implementation — it must \
         report byte-identical answers to the in-memory reference for the same tree"
    );
}

#[test]
fn a_second_refresh_with_nothing_changed_still_answers_correctly() {
    let d = tree(&[("a.rs", "alpha")]);
    let bridge = Bridge::spawn(d.path(), gmr_survey::sqlite::open_in_memory).unwrap();
    let pos = serde_json::json!({ "file": "a.rs", "name": "alpha" });

    let first = look(&RECIPE, "", &pos, &bridge, &roomy()).unwrap();
    let second = look(&RECIPE, "", &pos, &bridge, &roomy()).unwrap();

    assert_eq!(
        first, second,
        "a refresh that finds every file's stamp unchanged takes the fast path and \
         writes nothing new — the answer must not move just because refresh ran twice"
    );
}

#[test]
fn a_file_that_stops_existing_is_forgotten() {
    let d = tree(&[("a.rs", "alpha"), ("b.rs", "beta")]);
    let bridge = Bridge::spawn(d.path(), gmr_survey::sqlite::open_in_memory).unwrap();

    let before = look(
        &RECIPE,
        "",
        &serde_json::json!({ "name": "beta" }),
        &bridge,
        &roomy(),
    )
    .unwrap();
    assert_eq!(before["found"], true);

    std::fs::remove_file(d.path().join("b.rs")).unwrap();
    let after = look(
        &RECIPE,
        "",
        &serde_json::json!({ "name": "beta" }),
        &bridge,
        &roomy(),
    )
    .unwrap();
    assert_eq!(
        after["found"], false,
        "refresh must forget a file the walk no longer sees, not just stop updating it"
    );
}
