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

const FENCE: &str = "---";

fn fenced(line: &str) -> bool {
    line.trim_end() == FENCE
}

fn stated_in(text: &str) -> Claim {
    let mut lines = text.lines();
    if !lines.next().is_some_and(fenced) {
        return Claim::Silent;
    }
    let mut body = String::new();
    for line in lines {
        if fenced(line) {
            return match body.trim().is_empty() {
                true => Claim::Silent,
                false => match serde_yaml_ng::from_str::<serde_json::Value>(&body) {
                    Ok(value) => Claim::Says(value),
                    Err(e) => Claim::Malformed(format!("frontmatter is not valid YAML: {e}")),
                },
            };
        }
        body.push_str(line);
        body.push('\n');
    }
    Claim::Malformed("frontmatter is never closed by `---`".to_owned())
}

fn unreadable(at: &Path, e: &std::io::Error) -> CliError {
    CliError(format!(
        "cannot read `{}`: {e}. A directory that is not there holds no notes, which is an \
         answer; a directory that will not be read is our failure, and reporting it as empty \
         makes every anchor in this repository read as undeclared at once",
        at.display()
    ))
}

fn walk(root: &Path, at: &Path, out: &mut Vec<String>) -> Result<(), CliError> {
    let entries = match std::fs::read_dir(at) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(unreadable(at, &e)),
    };
    for entry in entries {
        let path = entry.map_err(|e| unreadable(at, &e))?.path();
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
    fn a_block_that_closes_immediately_is_closed() {
        let got = claims(&[("memories/a.md", "---\n---\n\n# just prose\n")]);
        assert_eq!(
            got[0].1,
            Claim::Silent,
            "`---` twice with nothing between them is the ordinary way to write a note that \
             claims nothing on purpose. Told it was never closed, its author goes looking for \
             a missing line that is right there"
        );
    }

    #[test]
    fn a_note_saved_with_windows_line_endings_still_states_what_it_is_about() {
        let got = claims(&[(
            "memories/a.md",
            "---\r\nabout: src/a.rs\r\nwatch: [sig]\r\n---\r\n\r\nbody\r\n",
        )]);
        assert_eq!(
            got[0].1,
            Claim::Says(serde_json::json!({ "about": "src/a.rs", "watch": ["sig"] })),
            "read as silence, this note names no anchor and nothing observes it — and the \
             only thing separating it from a note that works is which editor saved it"
        );
    }

    #[test]
    fn only_a_line_that_is_nothing_but_the_fence_closes_the_block() {
        let got = claims(&[(
            "memories/a.md",
            "---\nabout: src/a.rs\n--- not a fence\nwatch: [sig]\n---\n",
        )]);
        assert!(
            matches!(&got[0].1, Claim::Malformed(_)),
            "a line that merely starts with `---` used to end the block, and everything after \
             it — `watch:`, `anchors:`, whatever the note went on to declare — was dropped \
             with no complaint anywhere. Whatever this note is, it is not something to read \
             half of: {:?}",
            got[0].1
        );
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

    struct NoteStore {
        dir: tempfile::TempDir,
        provider: gmr_provider::git::Git,
        notes: Notes,
        written: std::sync::atomic::AtomicUsize,
    }

    impl NoteStore {
        fn new() -> Self {
            let dir = world(&[]);
            std::fs::create_dir_all(dir.path().join("memories")).unwrap();
            Self {
                provider: gmr_provider::git::Git::new(dir.path()),
                notes: Notes::at(dir.path(), "memories"),
                written: std::sync::atomic::AtomicUsize::new(0),
                dir,
            }
        }
    }

    #[async_trait]
    impl gmr::content::testkit::Corpus for NoteStore {
        fn provider(&self) -> &dyn gmr::ContentProvider {
            &self.provider
        }

        async fn holding(&self, bytes: &[u8]) -> gmr::ExternalId {
            let rel = format!(
                "memories/{}.md",
                self.written
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            );
            std::fs::write(self.dir.path().join(&rel), bytes).unwrap();
            gmr::ExternalId::new(rel)
        }

        async fn never_held(&self) -> gmr::ExternalId {
            gmr::ExternalId::new("memories/nobody-wrote-this.md")
        }

        async fn out_of_reach(&self) -> Box<dyn gmr::ContentProvider> {
            Box::new(gmr_provider::git::Git::new(
                self.dir.path().join("never-created"),
            ))
        }
    }

    #[async_trait]
    impl gmr::content::testkit::Listing for NoteStore {
        fn source(&self) -> &dyn MemorySource {
            &self.notes
        }
    }

    #[tokio::test]
    async fn the_note_directory_conforms_as_a_listing() {
        gmr::content::testkit::lists(&NoteStore::new())
            .await
            .unwrap();
    }

    #[test]
    fn a_notes_directory_that_is_not_there_holds_no_notes() {
        let dir = world(&[]);
        assert!(
            Notes::at(dir.path(), "memories")
                .declared()
                .unwrap()
                .is_empty(),
            "a repository that keeps no notes here is not broken, it is answering"
        );
    }

    #[test]
    fn a_notes_directory_that_will_not_be_listed_is_our_failure_not_an_empty_answer() {
        let dir = world(&[("memories", "this is a file, not a directory")]);

        Notes::at(dir.path(), "memories").declared().expect_err(
            "swallowed, this reads as a repository with no notes at all — and every anchor \
             standing in it then reports as undeclared, because no note declares anything \
             any more. `read_dir` fails for more reasons than absence, and the one thing the \
             reader must not be told is that their notes are gone",
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
