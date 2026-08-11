//! What an extractor is, factored out of the four that already exist, and the
//! body all four of their `probe` functions were separate copies of.
//!
//! Nothing calls this yet. It lands ahead of the extractors that will use it so
//! the shape can be argued about before four version bumps ride on it.

use std::path::Path;

use gmr_probe::Budget;
use serde_json::Value;

use crate::cache::{Cache, Halt, gather};
use crate::matching::{Candidate, nth, report, wanted};

pub type Collect = fn(&str, &[u8], &mut Vec<Candidate>) -> Result<(), String>;

pub type Eligible = fn(&str) -> bool;

pub enum Merge {
    Concat,
    Fold(fn(&[Candidate]) -> Result<Vec<Candidate>, String>),
}

impl Merge {
    pub fn apply(&self, gathered: &[Candidate]) -> Result<Vec<Candidate>, String> {
        match self {
            Self::Concat => Ok(gathered.to_vec()),
            Self::Fold(fold) => fold(gathered),
        }
    }
}

pub struct Recipe {
    pub name: &'static str,
    pub version: &'static str,
    pub items: &'static [&'static str],
    pub eligible: Eligible,
    pub collect: Collect,
    pub merge: Merge,
    pub barren: &'static str,
}

pub fn look(
    recipe: &Recipe,
    root: &Path,
    pos: &Value,
    cache: &Cache,
    budget: &Budget,
) -> Result<Value, Halt> {
    let want = wanted(pos, recipe.items)?;
    let gathered = gather(root, cache, recipe, budget)?;
    let candidates = recipe.merge.apply(&gathered)?;
    if candidates.is_empty() {
        return Err(Halt::Refused(format!(
            "{} {}; the probe is likely pointed at the wrong directory",
            root.display(),
            recipe.barren
        )));
    }
    Ok(report(recipe.version, &want, nth(pos), &candidates)?)
}

#[cfg(test)]
pub(crate) mod fixture {
    use super::*;
    use std::collections::BTreeMap;

    pub(crate) fn one(rel: &str, bytes: &[u8], out: &mut Vec<Candidate>) -> Result<(), String> {
        let body = String::from_utf8_lossy(bytes).trim().to_owned();
        let coord: BTreeMap<String, String> = [
            ("file".to_owned(), rel.to_owned()),
            ("name".to_owned(), body.clone()),
        ]
        .into();
        out.push(Candidate::new(
            format!("{rel}#{body}"),
            coord,
            serde_json::json!({ "bytes": bytes.len() }),
        ));
        Ok(())
    }

    pub(crate) fn rust_only(rel: &str) -> bool {
        rel.ends_with(".rs")
    }

    pub(crate) fn anything(_: &str) -> bool {
        true
    }

    pub(crate) fn recipe(eligible: Eligible, merge: Merge) -> Recipe {
        Recipe {
            name: "p",
            version: "v1",
            items: &["file", "name"],
            eligible,
            collect: one,
            merge,
            barren: "holds nothing this probe can read",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fixture::*;
    use super::*;

    fn candidate(name: &str) -> Candidate {
        Candidate::new(
            name,
            [("name".to_owned(), name.to_owned())].into(),
            serde_json::json!({ "n": 1 }),
        )
    }

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
        Budget::within(std::time::Duration::from_secs(60), 1 << 20)
    }

    #[test]
    fn concat_hands_on_what_it_was_given() {
        let gathered = [candidate("a"), candidate("b")];
        let out = Merge::Concat.apply(&gathered).unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].coord["name"], "a");
    }

    #[test]
    fn a_fold_may_return_fewer_than_it_was_given() {
        fn total(all: &[Candidate]) -> Result<Vec<Candidate>, String> {
            Ok(vec![Candidate::new(
                "all",
                [("name".to_owned(), "all".to_owned())].into(),
                serde_json::json!({ "n": all.len() }),
            )])
        }
        let out = Merge::Fold(total)
            .apply(&[candidate("a"), candidate("b")])
            .unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].facts, serde_json::json!({ "n": 2 }));
    }

    #[test]
    fn a_fold_that_refuses_refuses_the_whole_reading() {
        fn refuse(_: &[Candidate]) -> Result<Vec<Candidate>, String> {
            Err("that corpus makes no sense".to_owned())
        }
        let Err(why) = Merge::Fold(refuse).apply(&[candidate("a")]) else {
            panic!("a fold that cannot make sense of the corpus refuses the reading")
        };
        assert_eq!(why, "that corpus makes no sense");
    }

    #[test]
    fn look_reports_on_the_coordinate_it_was_given() {
        let d = tree(&[("a.rs", "alpha"), ("b.rs", "beta")]);
        let out = look(
            &recipe(anything, Merge::Concat),
            d.path(),
            &serde_json::json!({ "file": "a.rs", "name": "alpha" }),
            &Cache::disabled(),
            &roomy(),
        )
        .unwrap();
        assert_eq!(out["found"], true);
        assert_eq!(out["matched"], serde_json::json!(["file", "name"]));
        assert_eq!(out["extractor"], "v1");
    }

    #[test]
    fn a_position_naming_none_of_the_items_is_our_failure_not_the_worlds_answer() {
        let d = tree(&[("a.rs", "alpha")]);
        let e = look(
            &recipe(anything, Merge::Concat),
            d.path(),
            &serde_json::json!({ "unrelated": "x" }),
            &Cache::disabled(),
            &roomy(),
        )
        .unwrap_err();
        assert!(matches!(e, Halt::Refused(_)), "{e:?}");
    }

    #[test]
    fn a_tree_that_yields_nothing_is_our_failure_and_says_which_probe_was_pointed_wrong() {
        let d = tree(&[("notes.md", "prose")]);
        let e = look(
            &recipe(rust_only, Merge::Concat),
            d.path(),
            &serde_json::json!({ "file": "a.rs" }),
            &Cache::disabled(),
            &roomy(),
        )
        .unwrap_err();
        let Halt::Refused(why) = e else {
            panic!("an empty corpus is our failure, not the world's answer")
        };
        assert!(why.contains("holds nothing this probe can read"), "{why}");
        assert!(why.contains("wrong directory"), "{why}");
    }

    #[test]
    fn a_coordinate_that_matches_nothing_in_a_corpus_that_exists_is_found_false() {
        let d = tree(&[("a.rs", "alpha")]);
        let out = look(
            &recipe(anything, Merge::Concat),
            d.path(),
            &serde_json::json!({ "name": "gone" }),
            &Cache::disabled(),
            &roomy(),
        )
        .unwrap();
        assert_eq!(
            out["found"], false,
            "`I looked and it is not there` and `I could not look` are different answers, \
             and the barren check is the only thing between them"
        );
    }

    #[test]
    fn the_eligible_predicate_decides_what_the_corpus_even_contains() {
        let d = tree(&[("a.rs", "alpha"), ("notes.md", "alpha")]);
        let seen = |eligible: Eligible| {
            look(
                &recipe(eligible, Merge::Concat),
                d.path(),
                &serde_json::json!({ "name": "alpha" }),
                &Cache::disabled(),
                &roomy(),
            )
            .unwrap()["candidates"]
                .as_u64()
                .unwrap()
        };
        assert_eq!(seen(anything), 2);
        assert_eq!(
            seen(rust_only),
            1,
            "a file the recipe rules out is not read, so it cannot contribute — which is \
             why the predicate belongs to the extractor and inside its earned version"
        );
    }
}
