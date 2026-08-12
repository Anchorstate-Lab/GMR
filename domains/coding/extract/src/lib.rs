//! The coding domain's extractors: what counts as a function in Rust, how a
//! Markdown section is fingerprinted, which files a name occurs in.
//!
//! Language knowledge lives here rather than in a battery because it is a
//! domain's, and it is linked rather than executed because a version earned
//! from the semantic closure means the same thing either way.

mod addr;
mod ast;
mod lang;
mod name;
mod prose;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use gmr_core::{ProbeName, ProbeVersion};
use gmr_probe::Budget;
use gmr_survey::{Cache, Halt};
use gmr_transport::inproc::{ExtractError, Reach, Registered};
use serde_json::Value;

pub const WHOLE: [&str; 2] = ["file", "path"];

pub fn addressable(at: &[&str]) -> bool {
    at.iter().any(|k| WHOLE.contains(k))
}

#[derive(Clone, Copy)]
pub enum Reads {
    Extensions(&'static [&'static str]),
    Anything,
}

pub struct Vocabulary {
    pub name: &'static str,
    pub schema: &'static str,
    pub at: &'static [&'static str],
    pub facts: &'static [&'static str],
    pub reads: Reads,
}

type Probe = fn(&Path, &Value, &Cache, &Budget) -> Result<Value, Halt>;

const PROBES: [(Vocabulary, Probe, &str); 4] = [
    (
        Vocabulary {
            name: "ast-map",
            schema: SCHEMA,
            at: &[
                "file", "kind", "form", "vis", "surface", "after", "name", "callee", "member",
                "shape",
            ],
            facts: &["body", "line"],
            reads: Reads::Extensions(&[
                "rs", "ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs", "py", "pyi", "go",
            ]),
        },
        ast::probe,
        env!("GMR_EXTRACTOR_AST"),
    ),
    (
        Vocabulary {
            name: "addr-map",
            schema: SCHEMA,
            at: &["path", "name", "fingerprint"],
            facts: &["bytes"],
            reads: Reads::Anything,
        },
        addr::probe,
        env!("GMR_EXTRACTOR_ADDR"),
    ),
    (
        Vocabulary {
            name: "name-map",
            schema: SCHEMA,
            at: &["name", "scope"],
            facts: &["occurrences", "file_count", "files", "first"],
            reads: Reads::Anything,
        },
        name::probe,
        env!("GMR_EXTRACTOR_NAME"),
    ),
    (
        Vocabulary {
            name: "prose-map",
            schema: SCHEMA,
            at: &["file", "heading", "fingerprint"],
            facts: &["line", "lines"],
            reads: Reads::Extensions(&["md"]),
        },
        prose::probe,
        env!("GMR_EXTRACTOR_PROSE"),
    ),
];

const SCHEMA: &str = gmr_survey::COORD_REPORT_SCHEMA;

pub fn vocabularies() -> impl Iterator<Item = &'static Vocabulary> {
    PROBES.iter().map(|(v, _, _)| v)
}

pub const RECIPES: [&gmr_survey::Recipe; 4] =
    [&ast::RECIPE, &addr::RECIPE, &name::RECIPE, &prose::RECIPE];

pub fn recipe(name: &str) -> Option<&'static gmr_survey::Recipe> {
    RECIPES.into_iter().find(|r| r.name == name)
}

pub fn for_extension(ext: &str) -> Option<&'static str> {
    let reachable: Vec<&Vocabulary> = vocabularies().filter(|v| addressable(v.at)).collect();

    if let Some(v) = reachable
        .iter()
        .find(|v| matches!(v.reads, Reads::Extensions(exts) if exts.contains(&ext)))
    {
        return Some(v.name);
    }

    let mut catchall = reachable
        .iter()
        .filter(|v| matches!(v.reads, Reads::Anything));
    let first = catchall.next()?;
    match catchall.next() {
        None => Some(first.name),
        Some(_) => None,
    }
}

fn root_of(cwd: &Path, params: &Value) -> std::path::PathBuf {
    cwd.join(params.get("root").and_then(Value::as_str).unwrap_or("."))
}

pub struct Linked {
    pub probes: BTreeMap<ProbeName, Registered>,
    pub cache_fault: Option<String>,
}

pub fn registry(state_dir: &Path) -> Linked {
    let stamps = PROBES
        .iter()
        .map(|(v, _, version)| (v.name.to_owned(), (*version).to_owned()))
        .collect();
    let cache = Cache::load(&state_dir.join("extract-cache.json"), stamps);
    Linked {
        cache_fault: cache.fault().map(str::to_owned),
        probes: bind(Arc::new(cache)),
    }
}

pub fn registry_uncached() -> BTreeMap<ProbeName, Registered> {
    bind(Arc::new(Cache::disabled()))
}

fn bind(cache: Arc<Cache>) -> BTreeMap<ProbeName, Registered> {
    PROBES
        .iter()
        .map(|(v, probe, version)| {
            let probe = *probe;
            let cache = Arc::clone(&cache);
            (
                ProbeName::new(v.name),
                Registered {
                    version: ProbeVersion::new(*version),
                    extract: Arc::new(move |reach: &Reach| {
                        probe(
                            &root_of(&reach.cwd, &reach.params),
                            &reach.position,
                            &cache,
                            &reach.budget,
                        )
                        .map_err(|halt| match halt {
                            Halt::Spent(spent) => ExtractError::Spent(spent),
                            Halt::Refused(why) => ExtractError::Refused(why),
                        })
                    }),
                },
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gmr_transport::inproc::Budget;

    #[test]
    fn the_contract_the_closure_hashes_is_the_one_core_declares() {
        assert_eq!(gmr_core::OUTCOME_CONTRACT, "gmr.outcome.v1");
    }

    #[test]
    fn every_version_is_earned_and_distinct() {
        let mut seen = std::collections::BTreeSet::new();
        for (v, _, version) in &PROBES {
            assert!(
                ProbeVersion::try_new(*version).is_ok(),
                "{} has no earned version",
                v.name
            );
            assert!(seen.insert(*version), "{} shares another's closure", v.name);
        }
    }

    #[test]
    fn a_name_is_a_name_and_registers_under_it() {
        let reg = registry_uncached();
        for v in vocabularies() {
            assert!(reg.contains_key(&ProbeName::new(v.name)), "{}", v.name);
        }
    }

    #[test]
    fn a_probes_reads_rule_is_never_wider_or_narrower_than_its_own_eligible() {
        for v in vocabularies().filter(|v| addressable(v.at)) {
            let recipe = recipe(v.name).unwrap();
            match v.reads {
                Reads::Extensions(exts) => {
                    for ext in exts {
                        assert!(
                            (recipe.eligible)(&format!("x.{ext}")),
                            "`{}` claims .{ext} in its routing table, but its own eligible \
                             rule rejects a file with that extension",
                            v.name
                        );
                    }
                    assert!(
                        !(recipe.eligible)("x.an-extension-declared-nowhere"),
                        "`{}` declares a closed set of extensions, but its eligible rule \
                         accepts one that is not in it — the routing table would then be \
                         narrower than what the probe actually reads",
                        v.name
                    );
                }
                Reads::Anything => assert!(
                    (recipe.eligible)("x.an-extension-declared-nowhere"),
                    "`{}` claims to read anything, but its own eligible rule rejects an \
                     arbitrary extension",
                    v.name
                ),
            }
        }
    }

    #[test]
    fn at_most_one_addressable_probe_may_read_anything() {
        let catchall: Vec<&str> = vocabularies()
            .filter(|v| addressable(v.at) && matches!(v.reads, Reads::Anything))
            .map(|v| v.name)
            .collect();
        assert!(
            catchall.len() <= 1,
            "for_extension derives its fallback by finding the one addressable probe that \
             reads anything; two candidates ({catchall:?}) means the fallback is decided by \
             iteration order instead of being provably unique"
        );
    }

    #[test]
    fn a_probe_with_no_whole_to_point_at_is_never_the_derived_fallback() {
        let name_map = vocabularies().find(|v| v.name == "name-map").unwrap();
        assert!(
            !addressable(name_map.at),
            "name-map's coordinate is (name, scope) with no `file` or `path` slot, so a \
             person pointing at a whole file can never mean this probe — it has to fall out \
             of `at`, not be excluded by an empty `reads` list somebody remembered to leave \
             blank"
        );
        assert_ne!(
            for_extension("an-extension-only-name-map-would-otherwise-catch"),
            Some("name-map")
        );
    }

    #[test]
    fn for_extension_now_answers_for_what_used_to_be_opaque() {
        assert_eq!(for_extension("ts"), Some("ast-map"));
        assert_eq!(
            for_extension("md"),
            Some("prose-map"),
            "prose-map's eligible rule was always `.md`; its routing table just never said so"
        );
        assert_eq!(
            for_extension("sh"),
            Some("addr-map"),
            "no probe declares .sh, but addr-map's eligible rule is `true` for every path, \
             and it is the only addressable probe that does — so it is the fallback, derived \
             rather than named"
        );
        assert_eq!(
            for_extension("literally-anything-unlisted"),
            Some("addr-map")
        );
    }

    struct Fixture {
        probe: &'static str,
        file: &'static str,
        body: &'static str,
        pos: &'static str,
    }

    const FIXTURES: [Fixture; 4] = [
        Fixture {
            probe: "ast-map",
            file: "a.rs",
            body: "use std::fmt;\npub struct X { pub a: u64 }\npub fn f() { g(); }\n",
            pos: r#"{"file": "a.rs"}"#,
        },
        Fixture {
            probe: "addr-map",
            file: "a.rs",
            body: "anything",
            pos: r#"{"path": "a.rs"}"#,
        },
        Fixture {
            probe: "name-map",
            file: "a.rs",
            body: "let needle = 1;\n",
            pos: r#"{"name": "needle"}"#,
        },
        Fixture {
            probe: "prose-map",
            file: "a.md",
            body: "# Top\n\nbody\n",
            pos: r#"{"file": "a.md"}"#,
        },
    ];

    #[test]
    fn every_matchable_key_is_one_the_probe_declares() {
        for (v, items) in [
            (ast_map(), &ast::ITEMS[..]),
            (named("addr-map"), &addr::ITEMS[..]),
            (named("name-map"), &name::ITEMS[..]),
            (named("prose-map"), &prose::ITEMS[..]),
        ] {
            for key in items {
                assert!(
                    v.at.contains(key),
                    "`{}` matches on `{key}` but does not declare it",
                    v.name
                );
            }
        }
    }

    fn named(name: &str) -> &'static Vocabulary {
        vocabularies()
            .find(|v| v.name == name)
            .unwrap_or_else(|| panic!("`{name}` is not linked in"))
    }

    fn ast_map() -> &'static Vocabulary {
        named("ast-map")
    }

    #[test]
    fn every_key_a_probe_declares_comes_back_from_a_real_run() {
        let reg = registry_uncached();
        for v in vocabularies() {
            let f = FIXTURES
                .iter()
                .find(|f| f.probe == v.name)
                .unwrap_or_else(|| {
                    panic!(
                        "`{}` has no fixture; then it can declare keys it never emits",
                        v.name
                    )
                });

            let dir = std::env::temp_dir().join(format!("gmr-vocab-{}", v.name));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join(f.file), f.body).unwrap();
            let look = |nth: usize| {
                let mut pos: Value = serde_json::from_str(f.pos).unwrap();
                pos["nth"] = nth.into();
                (reg[&ProbeName::new(v.name)].extract)(&Reach {
                    cwd: dir.clone(),
                    position: pos,
                    params: serde_json::json!({}),
                    budget: Budget::within(std::time::Duration::from_secs(30), 1 << 20),
                })
                .unwrap_or_else(|e| panic!("`{}` on its own fixture: {e}", v.name))
            };

            let tied = look(0)["candidates"].as_u64().unwrap() as usize;
            let mut seen = std::collections::BTreeSet::new();
            for nth in 0..tied {
                seen.extend(
                    look(nth)["at"]
                        .as_object()
                        .into_iter()
                        .flatten()
                        .map(|(k, _)| k.clone()),
                );
            }
            assert!(
                !seen.is_empty(),
                "`{}` matched nothing on its own fixture",
                v.name
            );
            for key in v.at {
                assert!(
                    seen.contains(*key),
                    "`{}` declares `at.{key}` but no candidate carries it; emitted {seen:?}",
                    v.name
                );
            }
        }
    }

    #[test]
    fn every_probe_stops_when_nobody_is_waiting_for_it_any_more() {
        let reg = registry_uncached();
        for f in &FIXTURES {
            let dir = std::env::temp_dir().join(format!("gmr-spent-{}", f.probe));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join(f.file), f.body).unwrap();

            let spent = Budget::until(
                std::time::Instant::now() - std::time::Duration::from_secs(1),
                1 << 20,
            );
            let outcome = (reg[&ProbeName::new(f.probe)].extract)(&Reach {
                cwd: dir.clone(),
                position: serde_json::from_str(f.pos).unwrap(),
                params: serde_json::json!({}),
                budget: spent,
            });

            assert!(
                matches!(outcome, Err(ExtractError::Spent(_))),
                "`{}` ran to completion on a budget that was already gone. A deadline the \
                 work never looks at is not cancellation: the transport gives up, the caller \
                 gets its error, and the thread carries on scanning the whole repository for \
                 nobody. This loop is over every fixture so a new probe is covered by \
                 existing, not by someone remembering",
                f.probe
            );
        }
    }
}
