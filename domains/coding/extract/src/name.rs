use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use gmr_survey as coord;
use serde_json::{Value, json};

const VERSION: &str = env!("GMR_EXTRACTOR_NAME");

const ITEMS: [&str; 2] = ["name", "scope"];

#[derive(Default)]
struct Seen {
    count: usize,
    files: BTreeSet<String>,
    first: Option<(String, usize)>,
}

fn idents(line: &str) -> impl Iterator<Item = &str> {
    line.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|w| !w.is_empty() && !w.chars().next().is_some_and(|c| c.is_ascii_digit()))
}

fn scopes_of(rel: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut acc = String::new();
    for part in rel.split('/').filter(|p| !p.is_empty()) {
        if !acc.is_empty() {
            acc.push('/');
        }
        acc.push_str(part);
        out.push(acc.clone());
    }
    out
}

fn collect(path: &Path, rel: &str, out: &mut BTreeMap<(String, String), Seen>) {
    let Ok(src) = std::fs::read_to_string(path) else {
        return;
    };
    let scopes = scopes_of(rel);
    for (i, line) in src.lines().enumerate() {
        for w in idents(line) {
            for s in &scopes {
                let e = out.entry((w.to_owned(), s.clone())).or_default();
                e.count += 1;
                e.files.insert(rel.to_owned());
                e.first.get_or_insert_with(|| (rel.to_owned(), i + 1));
            }
        }
    }
}

pub fn probe(root: &Path, pos: &Value) -> Result<Value, String> {
    let want = coord::wanted(pos, &ITEMS)?;
    let mut seen = BTreeMap::new();
    coord::visit(root, &mut |p, rel| {
        collect(p, rel, &mut seen);
        Ok(())
    })?;
    if seen.is_empty() {
        return Err(format!(
            "{} contains no readable files; the probe is likely pointed at the wrong directory",
            root.display()
        ));
    }
    let cands: Vec<coord::Candidate> = seen
        .into_iter()
        .map(|((name, scope), s)| {
            let c: BTreeMap<String, String> = [("name", name), ("scope", scope)]
                .into_iter()
                .map(|(k, v)| (k.to_owned(), v))
                .collect();
            coord::Candidate::new(
                c,
                json!({
                    "occurrences": s.count,
                    "file_count": s.files.len(),
                    "files": s.files.iter().take(20).collect::<Vec<_>>(),
                    "first": s.first.map(|(f, l)| format!("{f}:{l}")),
                }),
            )
        })
        .collect();
    coord::report(VERSION, &want, coord::nth(pos), &cands)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str, files: &[(&str, &str)]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("name-map-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        for (path, body) in files {
            let p = dir.join(path);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, body).unwrap();
        }
        dir
    }

    fn at(dir: &Path, pos: Value) -> Value {
        probe(dir, &pos).unwrap()
    }

    #[test]
    fn a_name_that_is_present_reports_where_and_how_often() {
        let d = fixture(
            "present",
            &[("a.rs", "struct Baseline;\nfn f(b: Baseline) {}\n")],
        );
        let v = at(&d, json!({"name": "Baseline", "scope": ""}));
        assert_eq!(v["missed"], json!([]));
        assert_eq!(v["facts"]["occurrences"], 2);
        assert_eq!(v["facts"]["first"], "a.rs:1");
    }

    #[test]
    fn a_dead_concept_coming_back_is_just_a_name_that_is_present_again() {
        let gone = fixture("gone", &[("a.rs", "struct State;\n")]);
        assert_eq!(at(&gone, json!({"name": "Baseline"}))["found"], false);
        let back = fixture("back", &[("a.rs", "struct Baseline;\n")]);
        assert_eq!(at(&back, json!({"name": "Baseline"}))["missed"], json!([]));
    }

    #[test]
    fn a_name_that_left_its_scope_reports_only_scope_missed() {
        let d = fixture(
            "scope",
            &[
                ("core/a.rs", "fn keep() {}"),
                ("shell/b.rs", "fn moved() {}"),
            ],
        );
        let v = at(&d, json!({"name": "moved", "scope": "core"}));
        assert_eq!(v["missed"], json!(["scope"]));
        assert_eq!(v["at"]["scope"], "");
    }

    #[test]
    fn scopes_are_every_prefix_of_the_path() {
        assert_eq!(
            scopes_of("a/b/c.rs"),
            vec!["", "a", "a/b", "a/b/c.rs"]
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn digits_and_punctuation_are_not_identifiers() {
        let got: Vec<&str> = idents("let x_1 = 42 + foo(bar);").collect();
        assert_eq!(got, vec!["let", "x_1", "foo", "bar"]);
    }

    #[test]
    fn an_empty_position_is_our_failure_not_the_worlds_answer() {
        let d = fixture("empty", &[("a.rs", "fn f(){}")]);
        assert!(probe(&d, &json!({})).is_err());
    }

    #[test]
    fn an_unreadable_tree_is_our_failure_too() {
        let d = fixture("bare", &[]);
        assert!(probe(&d, &json!({"name": "x"})).is_err());
    }

    #[test]
    fn the_extractor_hashes_its_own_source() {
        let d = fixture("version", &[("a.rs", "fn f(){}")]);
        let v = at(&d, json!({"name": "f"}));
        assert_eq!(v["extractor"], VERSION);
        assert_eq!(v["extractor"].as_str().unwrap().len(), 64);
    }
}
