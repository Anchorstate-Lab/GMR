use gmr_budget::Budget;
use serde_json::Value;

use crate::corpus::{Corpus, Halt};
use crate::matching::{Candidate, Fragment, Want, nth, report, wanted};

pub type Collect = fn(&str, &[u8], &mut Vec<Fragment>) -> Result<(), String>;

pub type Eligible = fn(&str) -> bool;

pub type Fold = fn(&[Fragment]) -> Result<Vec<Candidate>, String>;

pub enum Merge {
    Concat,
    Fold(Fold),
}

pub struct Recipe {
    pub name: &'static str,
    pub version: &'static str,
    pub items: &'static [&'static str],
    pub narrows_on: &'static [&'static str],
    pub identity: &'static [&'static str],
    pub eligible: Eligible,
    pub collect: Collect,
    pub merge: Merge,
    pub barren: &'static str,
}

impl Recipe {
    pub fn narrowable(&self, want: &Want) -> Want {
        want.iter()
            .filter(|(k, _)| self.narrows_on.contains(&k.as_str()))
            .cloned()
            .collect()
    }
}

pub fn look(
    recipe: &Recipe,
    root: &str,
    pos: &Value,
    corpus: &dyn Corpus,
    budget: &Budget,
) -> Result<Value, Halt> {
    let want = wanted(pos, recipe.items)?;
    corpus.refresh(recipe, budget)?;

    let narrowed = recipe.narrowable(&want);
    let fragments = match narrowed.is_empty() {
        true => corpus.whole(recipe, root)?,
        false => corpus.touching(recipe, root, &narrowed)?,
    };
    if fragments.is_empty() && !corpus.populated(recipe, root)? {
        return Err(Halt::Refused(format!(
            "{root} {}; the probe is likely pointed at the wrong directory",
            recipe.barren
        )));
    }

    let candidates = match recipe.merge {
        Merge::Concat => fragments.into_iter().map(Candidate::verbatim).collect(),
        Merge::Fold(fold) => fold(&fragments)?,
    };
    Ok(report(
        recipe.version,
        &want,
        recipe.identity,
        nth(pos),
        &candidates,
    )?)
}

#[cfg(test)]
pub(crate) mod fixture {
    use super::*;
    use std::collections::BTreeMap;

    pub(crate) fn one(rel: &str, bytes: &[u8], out: &mut Vec<Fragment>) -> Result<(), String> {
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
            identity: &[],
            narrows_on: match merge {
                Merge::Concat => &["file", "name"],
                Merge::Fold(_) => &[],
            },
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

    fn surveyed(d: &tempfile::TempDir) -> crate::testkit::Surveyed {
        crate::testkit::Surveyed::over(d.path())
    }

    fn tally(all: &[Fragment]) -> Result<Vec<Candidate>, String> {
        Ok(vec![Candidate::new(
            "all",
            [("name".to_owned(), "all".to_owned())].into(),
            serde_json::json!({ "n": all.len() }),
        )])
    }

    fn refuse(_: &[Fragment]) -> Result<Vec<Candidate>, String> {
        Err("that corpus makes no sense".to_owned())
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
    fn look_reports_on_the_coordinate_it_was_given() {
        let d = tree(&[("a.rs", "alpha"), ("b.rs", "beta")]);
        let out = look(
            &recipe(anything, Merge::Concat),
            "",
            &serde_json::json!({ "file": "a.rs", "name": "alpha" }),
            &surveyed(&d),
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
            "",
            &serde_json::json!({ "unrelated": "x" }),
            &surveyed(&d),
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
            "",
            &serde_json::json!({ "file": "a.rs" }),
            &surveyed(&d),
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
            "",
            &serde_json::json!({ "name": "gone" }),
            &surveyed(&d),
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
                "",
                &serde_json::json!({ "name": "alpha" }),
                &surveyed(&d),
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

    #[test]
    fn a_fold_may_answer_about_something_no_single_file_holds() {
        let d = tree(&[("a.rs", "alpha"), ("b.rs", "beta")]);
        let out = look(
            &recipe(anything, Merge::Fold(tally)),
            "",
            &serde_json::json!({ "name": "all" }),
            &surveyed(&d),
            &roomy(),
        )
        .unwrap();
        assert_eq!(
            out["facts"]["n"], 2,
            "the fold sees every fragment the walk produced, which is the only way a \
             cross-file total can exist at all"
        );
    }

    #[test]
    fn a_fold_that_refuses_refuses_the_whole_reading() {
        let d = tree(&[("a.rs", "alpha")]);
        let e = look(
            &recipe(anything, Merge::Fold(refuse)),
            "",
            &serde_json::json!({ "name": "alpha" }),
            &surveyed(&d),
            &roomy(),
        )
        .unwrap_err();
        let Halt::Refused(why) = e else {
            panic!("a corpus the fold cannot make sense of is our failure")
        };
        assert_eq!(why, "that corpus makes no sense");
    }
}
