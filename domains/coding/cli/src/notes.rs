use std::path::{Path, PathBuf};

use async_trait::async_trait;
use gmr::probe::Budget;
use gmr::{ContentError, MemorySource, ProviderId, Record, Ref, Version};

use crate::error::CliError;

const RESOLVED_THROUGH: &str = "git";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Claim {
    Says(serde_json::Value),
    Silent,
    Malformed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stated {
    pub record: Record,
    pub claim: Claim,
}

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

    pub fn name_of(&self, reference: &Ref) -> Option<String> {
        if reference.provider != self.id {
            return None;
        }
        let rel = reference.external_id.as_str();
        let inside = rel.strip_prefix(&format!("{}/", self.dir))?;
        Some(inside.strip_suffix(".md").unwrap_or(inside).to_owned())
    }

    pub fn declared(&self) -> Result<Vec<Stated>, CliError> {
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
        let versions = versions_of(&self.root, &paths)?;

        Ok(read
            .into_iter()
            .zip(versions)
            .map(|((rel, body), version)| {
                let (bytes, claim) = match body {
                    Ok(text) => {
                        let claim = stated_in(&text);
                        (text.into_bytes(), claim)
                    }
                    Err(why) => (Vec::new(), Claim::Malformed(why)),
                };
                Stated {
                    record: Record {
                        reference: Ref::new(self.id.as_str(), rel),
                        version,
                        bytes,
                    },
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
        Ok(self
            .declared()
            .map_err(|e| ContentError::new(e.to_string()))?
            .into_iter()
            .map(|d| d.record)
            .collect())
    }
}

fn versions_of(root: &Path, paths: &[&str]) -> Result<Vec<Version>, CliError> {
    gmr_provider::git::blob_versions(root, paths)
        .map(|hashes| hashes.into_iter().map(Version::new).collect())
        .map_err(|e| {
            CliError(format!(
                "cannot version the notes through `{RESOLVED_THROUGH}`: {e}.\n\
                 A version computed some other way would not be the one this provider hands \
                 back when the note is read, so every binding stamped with it reports the note \
                 as rewritten forever"
            ))
        })
}

fn stated_in(text: &str) -> Claim {
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
        let notes = Notes::at(dir.path(), "memories");
        notes
            .declared()
            .unwrap()
            .into_iter()
            .map(|d| (d.record.reference.external_id.to_string(), d.claim))
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

    #[tokio::test]
    async fn a_notes_version_is_the_one_its_provider_will_hand_back() {
        use gmr::ContentProvider;

        let dir = world(&[("memories/a.md", "---\nabout: x\n---")]);
        let declared = Notes::at(dir.path(), "memories").declared().unwrap();
        let read = gmr_provider::git::Git::new(dir.path())
            .fetch(
                &gmr::ExternalId::new("memories/a.md"),
                &Budget::within(std::time::Duration::from_secs(30), usize::MAX),
            )
            .await
            .unwrap()
            .unwrap();

        assert_eq!(
            declared[0].record.version, read.version,
            "sync stamps a binding with the version this source computed and `read` compares \
             it against the version the provider computes. Two ways of arriving at a version \
             means one repository state where they disagree, and there every note reports as \
             rewritten with a bound version nothing can retrieve"
        );
    }

    #[test]
    fn notes_that_cannot_be_versioned_are_refused_rather_than_versioned_some_other_way() {
        let dir = world(&[("memories/a.md", "---\nabout: x\n---")]);
        std::os::unix::fs::symlink("/nowhere/at/all", dir.path().join("memories/b.md")).unwrap();

        Notes::at(dir.path(), "memories").declared().expect_err(
            "when git cannot version a path there is no second way to compute one that the \
             provider would also arrive at, so the honest answer is to refuse",
        );
    }
}
