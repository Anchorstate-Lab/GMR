//! Where derived candidates live between the scan that produced them and the
//! query that reads them, and the bookkeeping that says whether they are whole.
//!
//! Nothing calls this yet. It lands ahead of the extractors that will use it so
//! the two backends can be held against each other before anything depends on
//! either.

use std::collections::BTreeMap;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::matching::Want;
use crate::walk::hash;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Generation {
    probe: String,
    version: String,
    id: String,
}

impl Generation {
    pub fn of(probe: &str, version: &str) -> Self {
        Self {
            probe: probe.to_owned(),
            version: version.to_owned(),
            id: hash(&format!("{probe}\u{0}{version}")),
        }
    }

    pub fn probe(&self) -> &str {
        &self.probe
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn as_str(&self) -> &str {
        &self.id
    }
}

impl std::fmt::Display for Generation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{}", self.probe, &self.id[..12])
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Built {
    pub files: u64,
    pub rows: u64,
    pub sealed_at: Option<DateTime<Utc>>,
}

impl Built {
    pub fn whole(&self) -> bool {
        self.sealed_at.is_some()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub ord: u32,
    pub id: String,
    pub coord: BTreeMap<String, String>,
    pub facts: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Indexed {
    pub rel: String,
    pub hash: String,
    pub sort: String,
    pub rows: Vec<Row>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Located {
    pub rel: String,
    pub row: Row,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    Busy,
    Corrupt,
    Io,
    Absent,
    Foreign,
    Other,
}

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct IndexError {
    pub fault: Fault,
    pub message: String,
}

impl IndexError {
    pub fn new(fault: Fault, message: impl Into<String>) -> Self {
        Self {
            fault,
            message: message.into(),
        }
    }

    pub fn absent(of: &Generation) -> Self {
        Self::new(
            Fault::Absent,
            format!(
                "there is no index for {of}. A generation is opened by writing into it, and \
                 sealing one that was never opened would record a completeness nobody earned"
            ),
        )
    }

    pub fn foreign(what: &str, holds: &[String]) -> Self {
        Self::new(
            Fault::Foreign,
            format!(
                "{what} already holds {}, and none of it was written by an index. A derived \
                 store answers a shape it does not know by razing and rebuilding, which is only \
                 free when everything in the file can be recomputed — so it is refused here \
                 rather than applied to somebody else's database",
                holds.join(", ")
            ),
        )
    }

    pub fn retryable(&self) -> bool {
        self.fault == Fault::Busy
    }
}

pub fn under(rel: &str, root: &str) -> bool {
    match root {
        "" | "." => true,
        _ => rel
            .strip_prefix(root)
            .is_some_and(|rest| rest.starts_with('/')),
    }
}

pub fn touched(row: &Row, want: &Want) -> bool {
    want.iter().any(|(k, v)| row.coord.get(k) == Some(v))
}

pub fn sort_key(rel: &str) -> String {
    rel.replace('/', "\u{0}")
}

#[async_trait]
pub trait Index: Send + Sync {
    async fn built(&self, of: &Generation) -> Result<Option<Built>, IndexError>;

    async fn known(&self, of: &Generation) -> Result<BTreeMap<String, String>, IndexError>;

    async fn write(&self, of: &Generation, files: &[Indexed]) -> Result<(), IndexError>;

    async fn forget(&self, of: &Generation, gone: &[String]) -> Result<(), IndexError>;

    async fn seal(&self, of: &Generation, at: DateTime<Utc>) -> Result<(), IndexError>;

    async fn generations(&self) -> Result<Vec<(Generation, Built)>, IndexError>;

    async fn discard(&self, of: &Generation) -> Result<(), IndexError>;

    async fn rows(&self, of: &Generation, root: &str) -> Result<Vec<Located>, IndexError>;

    async fn union(
        &self,
        of: &Generation,
        root: &str,
        want: &Want,
    ) -> Result<Vec<Located>, IndexError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn row(name: &str) -> Row {
        Row {
            ord: 0,
            id: name.to_owned(),
            coord: [("name".to_owned(), name.to_owned())].into(),
            facts: json!({}),
        }
    }

    #[test]
    fn a_generation_is_the_probe_and_the_version_and_not_the_root() {
        let a = Generation::of("ast-map", "v1");
        assert_eq!(a, Generation::of("ast-map", "v1"));
        assert_ne!(a, Generation::of("ast-map", "v2"));
        assert_ne!(a, Generation::of("addr-map", "v1"));
        assert_eq!(
            a.as_str().len(),
            64,
            "a generation is a hash, so it is the same width whatever went into it"
        );
    }

    #[test]
    fn the_two_halves_of_a_generation_cannot_be_slid_into_each_other() {
        assert_ne!(
            Generation::of("ast", "map-v1"),
            Generation::of("ast-map", "v1"),
            "concatenating without a separator lets a longer name eat a shorter version, \
             and two different probes would then share one index"
        );
    }

    #[test]
    fn a_root_selects_what_is_under_it_and_not_what_merely_starts_with_it() {
        assert!(under("crates/gmr-core/src/lib.rs", "crates/gmr-core"));
        assert!(!under(
            "crates/gmr-core-extra/src/lib.rs",
            "crates/gmr-core"
        ));
        assert!(!under("crates/gmr-core", "crates/gmr-core"));
        assert!(under("anything/at/all.rs", ""));
        assert!(under("anything/at/all.rs", "."));
    }

    #[test]
    fn a_row_is_touched_when_any_one_wanted_pair_matches() {
        let want: Want = vec![
            ("name".to_owned(), "alpha".to_owned()),
            ("kind".to_owned(), "function".to_owned()),
        ];
        assert!(touched(&row("alpha"), &want));
        assert!(!touched(&row("beta"), &want));
    }

    fn laid_out() -> (tempfile::TempDir, Vec<String>) {
        let dir = tempfile::tempdir().unwrap();
        for rel in [
            "b.rs",
            "b/x.rs",
            "index.ts",
            "index/a.ts",
            "mod.rs",
            "mod/a.rs",
            "pkg.py",
            "pkg/__init__.py",
            "deep/a.rs",
            "deep/a/b.rs",
            "plain.rs",
            "other/one.rs",
        ] {
            let at = dir.path().join(rel);
            std::fs::create_dir_all(at.parent().unwrap()).unwrap();
            std::fs::write(&at, "x").unwrap();
        }

        let mut walked = Vec::new();
        crate::walk::visit(dir.path(), &mut |_, rel| {
            walked.push(rel.replace('\\', "/"));
            Ok(())
        })
        .unwrap();
        (dir, walked)
    }

    #[test]
    fn the_sort_key_reproduces_the_order_the_walk_hands_files_over_in() {
        let (_dir, walked) = laid_out();
        let mut keyed = walked.clone();
        keyed.sort_by_key(|rel| sort_key(rel));

        assert_eq!(
            walked, keyed,
            "the index only ever sorts by this key, so the day it disagrees with the walk \
             is the day `nth` starts naming a different candidate with nobody having \
             touched the code"
        );
    }

    #[test]
    fn sorting_the_same_paths_by_their_bytes_would_not_have_agreed() {
        let (_dir, walked) = laid_out();
        let mut by_bytes = walked.clone();
        by_bytes.sort();

        assert_ne!(
            walked, by_bytes,
            "a layout where a file and a directory share a stem is the whole reason this \
             key exists; if byte order happens to agree, the fixture stopped covering the \
             case and the test above proves nothing"
        );
    }

    #[test]
    fn a_generation_is_whole_only_once_it_has_a_time() {
        let mut built = Built {
            files: 3,
            rows: 9,
            sealed_at: None,
        };
        assert!(!built.whole());
        built.sealed_at = Some(DateTime::from_timestamp(1_700_000_000, 0).unwrap());
        assert!(built.whole());
    }
}
