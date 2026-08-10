use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use gmr_probe::Budget;
use gmr_survey as coord;
use serde_json::{Value, json};

const VERSION: &str = env!("GMR_EXTRACTOR_NAME");

pub(crate) const ITEMS: [&str; 2] = ["name", "scope"];

#[derive(Default)]
struct Seen<'a> {
    count: usize,
    files: BTreeSet<&'a str>,
    first: Option<(&'a str, usize)>,
}

fn idents(line: &str) -> impl Iterator<Item = &str> {
    line.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|w| !w.is_empty() && !w.chars().next().is_some_and(|c| c.is_ascii_digit()))
}

fn scopes_of(rel: &str) -> Result<Vec<&str>, String> {
    let mut out = vec![""];
    let mut end = 0;
    for part in rel.split('/') {
        if part.is_empty() {
            return Err(format!(
                "{rel} has an empty path component, so its scopes are not prefixes of it. \
                 A walked path cannot look like this; something upstream is handing out \
                 paths it did not build from directory entries"
            ));
        }
        end += part.len();
        out.push(&rel[..end]);
        end += 1;
    }
    Ok(out)
}

fn collect(path: &Path, rel: &str, out: &mut Vec<coord::Candidate>) {
    let Ok(src) = std::fs::read_to_string(path) else {
        return;
    };
    let mut here: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for (i, line) in src.lines().enumerate() {
        for w in idents(line) {
            let e = here.entry(w).or_insert((0, i + 1));
            e.0 += 1;
        }
    }
    out.extend(here.into_iter().map(|(name, (count, line))| {
        coord::Candidate::new(
            format!("{rel}#{name}"),
            [("name", name), ("file", rel)]
                .into_iter()
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                .collect(),
            json!({"count": count, "line": line}),
        )
    }));
}

fn merge(fragments: &[coord::Candidate]) -> Result<BTreeMap<(&str, &str), Seen<'_>>, String> {
    let mut seen: BTreeMap<(&str, &str), Seen> = BTreeMap::new();
    let mut walked: Option<(&str, Vec<&str>)> = None;
    for f in fragments {
        let (Some(name), Some(rel)) = (f.coord.get("name"), f.coord.get("file")) else {
            continue;
        };
        let count = f.facts["count"].as_u64().unwrap_or_default() as usize;
        let line = f.facts["line"].as_u64().unwrap_or_default() as usize;
        if walked.as_ref().is_none_or(|(had, _)| had != rel) {
            walked = Some((rel, scopes_of(rel)?));
        }
        let (_, scopes) = walked.as_ref().expect("just filled");
        for s in scopes {
            let e = seen.entry((name.as_str(), s)).or_default();
            e.count += count;
            e.files.insert(rel.as_str());
            e.first.get_or_insert((rel.as_str(), line));
        }
    }
    Ok(seen)
}

fn rolled(fragments: &[coord::Candidate]) -> Result<Vec<coord::Candidate>, String> {
    Ok(merge(fragments)?
        .into_iter()
        .map(|((name, scope), s)| {
            let c: BTreeMap<String, String> = [("name", name), ("scope", scope)]
                .into_iter()
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                .collect();
            coord::Candidate::new(
                format!("{}@{}", c["name"], c["scope"]),
                c,
                json!({
                    "occurrences": s.count,
                    "file_count": s.files.len(),
                    "files": s.files.iter().take(20).collect::<Vec<_>>(),
                    "first": s.first.map(|(f, l)| format!("{f}:{l}")),
                }),
            )
        })
        .collect())
}

pub fn probe(
    root: &Path,
    pos: &Value,
    cache: &coord::Cache,
    budget: &Budget,
) -> Result<Value, coord::Halt> {
    let want = coord::wanted(pos, &ITEMS)?;
    let cands = coord::visit_folded(
        root,
        cache,
        "name-map",
        budget,
        |p, rel, out| {
            collect(p, rel, out);
            Ok(())
        },
        rolled,
    )?;
    if cands.is_empty() {
        return Err(coord::Halt::Refused(format!(
            "{} contains no readable files; the probe is likely pointed at the wrong directory",
            root.display()
        )));
    }
    Ok(coord::report(VERSION, &want, coord::nth(pos), &cands)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roomy() -> Budget {
        Budget::within(std::time::Duration::from_secs(600), 1 << 24)
    }

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
        probe(dir, &pos, &coord::Cache::disabled(), &roomy()).unwrap()
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
            scopes_of("a/b/c.rs").unwrap(),
            vec!["", "a", "a/b", "a/b/c.rs"]
        );
        assert_eq!(scopes_of("top.rs").unwrap(), vec!["", "top.rs"]);
    }

    #[test]
    fn a_path_whose_scopes_are_not_its_prefixes_is_refused_not_dropped() {
        let e = scopes_of("a//b.rs").unwrap_err();
        assert!(
            e.contains("empty path component"),
            "scopes are borrowed slices of the path, which only works while every scope is \
             a prefix of it. A path where that breaks has to say so — skipping the file \
             would take it out of every count in silence"
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
        assert!(probe(&d, &json!({}), &coord::Cache::disabled(), &roomy()).is_err());
    }

    #[test]
    fn an_unreadable_tree_is_our_failure_too() {
        let d = fixture("bare", &[]);
        assert!(
            probe(
                &d,
                &json!({"name": "x"}),
                &coord::Cache::disabled(),
                &roomy()
            )
            .is_err()
        );
    }

    #[test]
    fn the_extractor_hashes_its_own_source() {
        let d = fixture("version", &[("a.rs", "fn f(){}")]);
        let v = at(&d, json!({"name": "f"}));
        assert_eq!(v["extractor"], VERSION);
        assert_eq!(v["extractor"].as_str().unwrap().len(), 64);
    }

    fn corpus() -> Vec<(&'static str, &'static str)> {
        vec![
            ("a.rs", "fn build() {}\nfn build_more() { build(); }\n"),
            ("core/b.rs", "fn build() { helper(); }\n"),
            ("core/deep/c.rs", "fn helper() {}\nfn build() {}\n"),
            ("shell/d.rs", "fn unrelated() {}\n"),
        ]
    }

    fn live_cache(tag: &str) -> (tempfile::TempDir, coord::Cache) {
        let state = tempfile::tempdir().unwrap();
        let cache = coord::Cache::load(
            &state.path().join(format!("{tag}.json")),
            [("name-map".to_owned(), VERSION.to_owned())]
                .into_iter()
                .collect(),
        )
        .unwrap();
        (state, cache)
    }

    #[test]
    fn a_second_query_reuses_what_the_first_one_read() {
        let d = fixture("reuse", &corpus());
        let (state, cache) = live_cache("reuse");
        let file = state.path().join("reuse.json");

        probe(&d, &json!({"name": "build", "scope": ""}), &cache, &roomy()).unwrap();

        let on_disk = std::fs::read_to_string(&file).unwrap_or_default();
        assert!(
            on_disk.contains("name-map"),
            "name-map wrote nothing to the cache, so it re-reads and re-tokenises the whole \
             repository on every single query — the one probe that pays its full cost every \
             time it is asked, forever"
        );
    }

    #[test]
    fn caching_changes_nothing_about_the_answer() {
        let d = fixture("identical", &corpus());
        let (_state, cache) = live_cache("identical");

        for pos in [
            json!({"name": "build", "scope": ""}),
            json!({"name": "build", "scope": "core"}),
            json!({"name": "helper", "scope": "core/deep"}),
            json!({"name": "build", "scope": "shell"}),
            json!({"name": "nowhere", "scope": ""}),
        ] {
            let cold = probe(&d, &pos, &coord::Cache::disabled(), &roomy()).unwrap();
            let warm = probe(&d, &pos, &cache, &roomy()).unwrap();
            let again = probe(&d, &pos, &cache, &roomy()).unwrap();
            assert_eq!(cold, warm, "a cache that changes the answer is not a cache");
            assert_eq!(warm, again, "and it has to keep changing nothing");
        }
    }
}
