use std::collections::BTreeMap;
use std::time::Duration;

use gmr_probe::Budget;
use gmr_survey::bridge::{Bridge, run_blocking};
use gmr_survey::index::{Built, Fault, Generation, Index, IndexError, Indexed, Snapshot};
use gmr_survey::matching::{Fragment, Want};
use gmr_survey::recipe::{Merge, Recipe, look};
use gmr_survey::testkit::Surveyed;
use gmr_survey::walk::{Held, Stamp};

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
    identity: &["name"],
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

#[test]
fn the_bridge_agrees_with_the_in_memory_reference() {
    let d = tree(&[("a.rs", "alpha"), ("b.rs", "beta")]);
    let pos = serde_json::json!({ "file": "a.rs", "name": "alpha" });

    let surveyed = Surveyed::over(d.path());
    let via_surveyed = look(&RECIPE, "", &pos, &surveyed, &roomy()).unwrap();

    let bridge = run_blocking(Bridge::open(d.path(), gmr_survey::sqlite::open_in_memory)).unwrap();
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
    let bridge = run_blocking(Bridge::open(d.path(), gmr_survey::sqlite::open_in_memory)).unwrap();
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
    let bridge = run_blocking(Bridge::open(d.path(), gmr_survey::sqlite::open_in_memory)).unwrap();

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

#[test]
fn a_bridge_told_the_tree_is_still_walks_each_generation_once() {
    let d = tree(&[("a.rs", "alpha"), ("b.rs", "beta")]);
    let bridge = run_blocking(Bridge::open(d.path(), gmr_survey::sqlite::open_in_memory))
        .unwrap()
        .over_a_still_tree();
    let beta = serde_json::json!({ "name": "beta" });

    assert_eq!(
        look(&RECIPE, "", &beta, &bridge, &roomy()).unwrap()["found"],
        true
    );
    std::fs::remove_file(d.path().join("b.rs")).unwrap();

    assert_eq!(
        look(&RECIPE, "", &beta, &bridge, &roomy()).unwrap()["found"],
        true,
        "this is the precondition the name states, made visible: a bridge told the tree \
         is still walks a generation once and answers every later question from what \
         that walk found. `gmr check` observes hundreds of anchors against one tree that \
         cannot change under it, and walking it once per anchor is the cost this exists \
         to remove. A caller that does rewrite files between questions must not ask for \
         it — `a_file_that_stops_existing_is_forgotten` holds the default, where every \
         refresh walks again"
    );
}

#[test]
fn a_generation_this_build_does_not_carry_is_dropped() {
    let d = tree(&[("a.rs", "alpha")]);
    let bridge = run_blocking(Bridge::open(d.path(), gmr_survey::sqlite::open_in_memory)).unwrap();
    let pos = serde_json::json!({ "name": "alpha" });
    look(&RECIPE, "", &pos, &bridge, &roomy()).unwrap();

    let mine = gmr_survey::index::Generation::of(RECIPE.name, RECIPE.version);
    let stale = gmr_survey::index::Generation::of(RECIPE.name, "v0");

    assert_eq!(
        run_blocking(bridge.retain(std::slice::from_ref(&mine))).unwrap(),
        Vec::new(),
        "a generation this build carries is kept"
    );
    assert_eq!(
        run_blocking(bridge.retain(&[stale])).unwrap(),
        vec![mine],
        "an extractor version this build no longer carries answers with candidates the \
         current logic would not produce, so it is dropped rather than kept forever. \
         Without this every version bump leaves a whole repository's postings behind and \
         the index only ever grows"
    );
}

struct BusyOnce {
    inner: gmr_survey::testkit::Remembered,
    refuse_next_known: std::sync::atomic::AtomicBool,
}

impl BusyOnce {
    fn new() -> Self {
        Self {
            inner: gmr_survey::testkit::Remembered::new(),
            refuse_next_known: std::sync::atomic::AtomicBool::new(true),
        }
    }
}

#[async_trait::async_trait]
impl Index for BusyOnce {
    async fn built(&self, of: &Generation) -> Result<Option<Built>, IndexError> {
        self.inner.built(of).await
    }

    async fn known(&self, of: &Generation) -> Result<BTreeMap<String, Held>, IndexError> {
        match self
            .refuse_next_known
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            true => Err(IndexError::new(Fault::Busy, "the database is locked")),
            false => self.inner.known(of).await,
        }
    }

    async fn write(&self, of: &Generation, files: &[Indexed]) -> Result<(), IndexError> {
        self.inner.write(of, files).await
    }

    async fn restamp(
        &self,
        of: &Generation,
        restamped: &[(String, Option<Stamp>)],
    ) -> Result<(), IndexError> {
        self.inner.restamp(of, restamped).await
    }

    async fn forget(&self, of: &Generation, gone: &[String]) -> Result<(), IndexError> {
        self.inner.forget(of, gone).await
    }

    async fn seal(
        &self,
        of: &Generation,
        at: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), IndexError> {
        self.inner.seal(of, at).await
    }

    async fn generations(&self) -> Result<Vec<(Generation, Built)>, IndexError> {
        self.inner.generations().await
    }

    async fn discard(&self, of: &Generation) -> Result<(), IndexError> {
        self.inner.discard(of).await
    }

    async fn rows(&self, of: &Generation, root: &str) -> Result<Option<Snapshot>, IndexError> {
        self.inner.rows(of, root).await
    }

    async fn union(
        &self,
        of: &Generation,
        root: &str,
        want: &Want,
    ) -> Result<Option<Snapshot>, IndexError> {
        self.inner.union(of, root, want).await
    }
}

#[test]
fn a_walk_that_ran_out_of_budget_is_not_remembered() {
    let d = tree(&[("a.rs", "alpha")]);
    let bridge = run_blocking(Bridge::open(d.path(), gmr_survey::sqlite::open_in_memory))
        .unwrap()
        .over_a_still_tree();
    let pos = serde_json::json!({ "name": "alpha" });

    let spent = Budget::within(Duration::from_secs(0), 1 << 20);
    assert!(
        look(&RECIPE, "", &pos, &bridge, &spent).is_err(),
        "a budget with no time left cannot finish a walk"
    );

    assert_eq!(
        look(&RECIPE, "", &pos, &bridge, &roomy()).unwrap()["found"],
        true,
        "a spent budget is one caller's deadline, not a fact about the tree. Remembering \
         it hands the next anchor — which arrives with a budget of its own — a deadline \
         that was never its, and one slow moment then fails every anchor behind it"
    );
}

#[test]
fn a_walk_the_index_would_not_answer_is_not_remembered() {
    let d = tree(&[("a.rs", "alpha")]);
    let bridge = run_blocking(Bridge::open(d.path(), || async { Ok(BusyOnce::new()) }))
        .unwrap()
        .over_a_still_tree();
    let pos = serde_json::json!({ "name": "alpha" });

    assert!(
        look(&RECIPE, "", &pos, &bridge, &roomy()).is_err(),
        "an index that will not answer cannot produce a reading"
    );

    assert_eq!(
        look(&RECIPE, "", &pos, &bridge, &roomy()).unwrap()["found"],
        true,
        "a store that would not answer is our failure, not the corpus refusing. A lock \
         held for a moment must not become a permanent refusal for the life of the process"
    );
}

#[test]
fn a_corpus_the_recipe_refuses_is_remembered() {
    fn refuses(_: &str, _: &[u8], _: &mut Vec<Fragment>) -> Result<(), String> {
        Err("that corpus makes no sense".to_owned())
    }
    const REFUSING: Recipe = Recipe {
        collect: refuses,
        ..RECIPE
    };

    let d = tree(&[("a.rs", "alpha")]);
    let bridge = run_blocking(Bridge::open(d.path(), gmr_survey::sqlite::open_in_memory))
        .unwrap()
        .over_a_still_tree();
    let pos = serde_json::json!({ "name": "alpha" });

    assert!(look(&REFUSING, "", &pos, &bridge, &roomy()).is_err());
    std::fs::remove_file(d.path().join("a.rs")).unwrap();
    assert!(
        look(&REFUSING, "", &pos, &bridge, &roomy()).is_err(),
        "a recipe that cannot read this corpus refuses it the same way however often it \
         is asked, so the answer is remembered and the tree is walked once"
    );
}
