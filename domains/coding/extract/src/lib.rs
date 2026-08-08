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
use gmr_transport::inproc::Registered;
use serde_json::Value;

/// What each probe can be asked about, and which coordinate items it answers.
/// Outside the version deliberately: it constrains which shapes fit, not what
/// the probe derives.
pub struct Vocabulary {
    pub name: &'static str,
    pub schema: &'static str,
    pub at: &'static [&'static str],
    pub facts: &'static [&'static str],
    /// File extensions this probe reads, so `about:` routes without the CLI
    /// knowing any language names. Empty means it is not reached that way.
    pub handles: &'static [&'static str],
}

type Probe = fn(&Path, &Value) -> Result<Value, String>;

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
            // Only ast-map: `about: f#name` yields {file, name}, and no other
            // probe's vocabulary matches that. prose-map wants a heading, so it
            // is named explicitly or `wanted` drops `name` and the anchor
            // silently watches the whole file.
            handles: &[
                "rs", "ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs", "py", "pyi", "go",
            ],
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
            handles: &[],
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
            handles: &[],
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
            handles: &[],
        },
        prose::probe,
        env!("GMR_EXTRACTOR_PROSE"),
    ),
];

const SCHEMA: &str = gmr_survey::COORD_REPORT_SCHEMA;

pub fn vocabularies() -> impl Iterator<Item = &'static Vocabulary> {
    PROBES.iter().map(|(v, _, _)| v)
}

pub fn for_extension(ext: &str) -> Option<&'static str> {
    vocabularies()
        .find(|v| v.handles.contains(&ext))
        .map(|v| v.name)
}

/// The portion to inspect comes from params, not from the process: params enter
/// the declaration hash, so the anchor says what it meant.
fn root_of(cwd: &Path, params: &Value) -> std::path::PathBuf {
    cwd.join(params.get("root").and_then(Value::as_str).unwrap_or("."))
}

pub fn registry() -> BTreeMap<ProbeName, Registered> {
    PROBES
        .iter()
        .map(|(v, probe, version)| {
            let probe = *probe;
            (
                ProbeName::new(v.name),
                Registered {
                    version: ProbeVersion::new(*version),
                    extract: Arc::new(move |cwd, pos, params| probe(&root_of(cwd, params), pos)),
                },
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let reg = registry();
        for v in vocabularies() {
            assert!(reg.contains_key(&ProbeName::new(v.name)), "{}", v.name);
        }
    }

    #[test]
    fn one_probe_owns_the_extensions_about_routes_by() {
        assert_eq!(for_extension("ts"), Some("ast-map"));
        assert_eq!(for_extension("md"), None);
    }

    /// `Vocabulary` lives in this file, outside the closure; the candidate map
    /// is built inside it. The two can drift, and when they do a shape reads
    /// `obs.at.<key>` no candidate carries — the rule faults, or the axis is
    /// simply never able to move and nobody finds out. Every declared key has
    /// to come back from a real run.
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
    fn every_key_a_probe_declares_comes_back_from_a_real_run() {
        let reg = registry();
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
            let out = (reg[&ProbeName::new(v.name)].extract)(
                &dir,
                &serde_json::from_str(f.pos).unwrap(),
                &serde_json::json!({}),
            )
            .unwrap_or_else(|e| panic!("`{}` on its own fixture: {e}", v.name));

            let mut seen = std::collections::BTreeSet::new();
            for m in out["matches"].as_array().into_iter().flatten() {
                seen.extend(
                    m["at"]
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
}
