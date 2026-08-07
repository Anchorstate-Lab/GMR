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
                "file", "kind", "form", "vis", "name", "callee", "member", "shape",
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
}
