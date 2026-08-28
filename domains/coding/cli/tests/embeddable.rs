use coding_anchor::{delivery::Subscriptions, probes::Catalog, shapes, stores};

fn a_repository() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".anchor")).unwrap();
    std::fs::write(dir.path().join(".anchor/probes.toml"), "").unwrap();
    std::fs::create_dir_all(dir.path().join("memories")).unwrap();
    dir
}

#[test]
fn the_four_things_only_the_binary_could_reach_are_reachable_from_outside_it() {
    let dir = a_repository();
    let root = dir.path();

    let catalog = Catalog::load(root).expect("a catalog loads without a `gmr` process");

    let built = stores::assembled(root).expect("content providers assemble the same way");
    let (subs, faults) = Subscriptions::load(root, &catalog, &built.names)
        .expect("subscriptions read the same declarations");

    assert!(
        faults.is_empty(),
        "an empty repository has nothing to fault"
    );
    let _ = subs;

    assert!(
        shapes::of(&gmr::Transitions(Vec::new())).is_none(),
        "shapes answers for a rule table nobody wrote, which is the point: these four were \
         private modules of the `gmr` binary, so any other front end had to reimplement them \
         or wait for this crate to come apart. It has, and this test is the proof that stays \
         red if it goes back together"
    );
}
