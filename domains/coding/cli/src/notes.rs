//! `memories/**.md` as a `MemorySource`.
//!
//! This lives in the domain rather than in a battery because everything it
//! decides is a domain decision: which directory holds notes, that only
//! `.md` counts, and that the grid a note speaks to GMR through is its
//! YAML frontmatter. It reads that grid and hands it over untouched — what
//! `about` or `watch` mean is not its business, which is why the same file
//! could serve a domain with an entirely different vocabulary.
//!
//! It names the provider its records resolve through, once, here. Everything
//! downstream carries the `Ref` it stamps rather than rebuilding one from a
//! constant of its own.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use gmr::probe::Budget;
use gmr::{Claim, ContentError, MemorySource, ProviderId, Record, Ref, Version};

use crate::error::CliError;

const RESOLVED_THROUGH: &str = "git";

pub struct Notes {
    root: PathBuf,
    dir: String,
    id: ProviderId,
}

impl Notes {
    pub fn at(root: &Path, dir: &str) -> Self {
        Self {
            root: root.to_path_buf(),
            dir: dir.to_owned(),
            id: ProviderId::new(RESOLVED_THROUGH),
        }
    }

    pub fn records(&self) -> Result<Vec<Record>, CliError> {
        let mut rels = Vec::new();
        walk(&self.root, &self.root.join(&self.dir), &mut rels)?;
        rels.sort();

        let read: Vec<(String, Result<String, String>)> = rels
            .into_iter()
            .map(|rel| {
                let body = std::fs::read_to_string(self.root.join(&rel))
                    .map_err(|e| format!("cannot read this file: {e}"));
                (rel, body)
            })
            .collect();

        let paths: Vec<&str> = read.iter().map(|(rel, _)| rel.as_str()).collect();
        let versions = versions_of(&self.root, &paths, &read);

        Ok(read
            .into_iter()
            .zip(versions)
            .map(|((rel, body), version)| {
                let (bytes, claim) = match body {
                    Err(why) => (Vec::new(), Claim::Malformed(why)),
                    Ok(text) => {
                        let claim = claim_of(&text);
                        (text.into_bytes(), claim)
                    }
                };
                Record {
                    reference: Ref::new(self.id.as_str(), rel),
                    version,
                    bytes,
                    claim,
                }
            })
            .collect())
    }
}

#[async_trait]
impl MemorySource for Notes {
    fn provider(&self) -> &ProviderId {
        &self.id
    }

    async fn list(&self, _budget: &Budget) -> Result<Vec<Record>, ContentError> {
        self.records().map_err(|e| ContentError::new(e.to_string()))
    }
}

fn versions_of(
    root: &Path,
    paths: &[&str],
    read: &[(String, Result<String, String>)],
) -> Vec<Version> {
    if let Ok(hashes) = gmr_provider::git::blob_versions(root, paths) {
        return hashes.into_iter().map(Version::new).collect();
    }
    read.iter()
        .map(|(_, body)| {
            let bytes = body.as_ref().map(String::as_bytes).unwrap_or_default();
            Version::new(gmr::core::content_hash_of_bytes(bytes).into_inner())
        })
        .collect()
}

fn claim_of(text: &str) -> Claim {
    let Some(rest) = text.strip_prefix("---\n") else {
        return Claim::Silent;
    };
    let Some(end) = rest.find("\n---") else {
        return Claim::Malformed("frontmatter is never closed by `---`".to_owned());
    };
    let body = &rest[..end];
    if body.trim().is_empty() {
        return Claim::Silent;
    }
    match serde_yaml_ng::from_str::<serde_json::Value>(body) {
        Ok(value) => Claim::Says(value),
        Err(e) => Claim::Malformed(format!("frontmatter is not valid YAML: {e}")),
    }
}

fn walk(root: &Path, at: &Path, out: &mut Vec<String>) -> Result<(), CliError> {
    let Ok(entries) = std::fs::read_dir(at) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(root, &path, out)?;
        } else if path.extension().is_some_and(|e| e == "md") {
            let rel = path.strip_prefix(root).unwrap_or(&path);
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (path, body) in files {
            let p = dir.path().join(path);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, body).unwrap();
        }
        dir
    }

    fn claims(files: &[(&str, &str)]) -> Vec<(String, Claim)> {
        let dir = world(files);
        Notes::at(dir.path(), "memories")
            .records()
            .unwrap()
            .into_iter()
            .map(|r| (r.reference.external_id.to_string(), r.claim))
            .collect()
    }

    #[test]
    fn a_note_with_frontmatter_hands_the_grid_over_without_reading_it() {
        let got = claims(&[(
            "memories/a.md",
            "---\nabout: src/a.rs\nwatch: [sig]\n---\nbody",
        )]);

        assert_eq!(
            got,
            vec![(
                "memories/a.md".to_owned(),
                Claim::Says(serde_json::json!({ "about": "src/a.rs", "watch": ["sig"] }))
            )],
            "`about` and `watch` are the domain's words. This file's job is to know that the \
             grid is the frontmatter block, not what is written in it"
        );
    }

    #[test]
    fn no_frontmatter_is_silence_not_an_error() {
        let got = claims(&[("memories/a.md", "just prose")]);
        assert_eq!(got[0].1, Claim::Silent);
    }

    #[test]
    fn frontmatter_that_is_opened_and_never_closed_is_malformed() {
        let got = claims(&[("memories/a.md", "---\nabout: src/a.rs\n")]);
        assert!(matches!(got[0].1, Claim::Malformed(_)), "{:?}", got[0].1);
    }

    #[test]
    fn an_empty_frontmatter_block_says_nothing_rather_than_being_broken() {
        let got = claims(&[("memories/a.md", "---\n\n---\nbody")]);
        assert_eq!(got[0].1, Claim::Silent);
    }

    #[test]
    fn only_markdown_under_the_notes_directory_is_listed() {
        let got = claims(&[
            ("memories/a.md", "---\nabout: x\n---"),
            ("memories/notes.txt", "---\nabout: x\n---"),
            ("elsewhere/b.md", "---\nabout: x\n---"),
            ("memories/deeper/c.md", "---\nabout: x\n---"),
        ]);
        let names: Vec<&str> = got.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(names, vec!["memories/a.md", "memories/deeper/c.md"]);
    }

    #[test]
    fn a_version_is_produced_even_where_git_cannot_run() {
        let dir = world(&[("memories/a.md", "---\nabout: x\n---")]);
        let records = Notes::at(dir.path(), "memories").records().unwrap();

        assert!(
            !records[0].version.as_str().is_empty(),
            "a tempdir is not a git repository, so the batch hash-object cannot run. The \
             fallback is not there to be right — nothing can bind without git either — it \
             is there so linting notes still works in a repository that has no git"
        );
    }
}
