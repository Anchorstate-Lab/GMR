use std::path::{Path, PathBuf};
use std::process::Command;

use async_trait::async_trait;
use gmr_content::{ContentError, ContentProvider, Fetched, History, MemoryStore};
use gmr_core::{ExternalId, ProviderId, Version};
use gmr_probe::Budget;

pub struct Git {
    root: PathBuf,
    id: ProviderId,
}

impl Git {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            id: ProviderId::new("git"),
        }
    }
}

pub fn store(root: impl Into<PathBuf>) -> MemoryStore {
    MemoryStore::new(std::sync::Arc::new(Git::new(root)))
}

#[async_trait]
impl ContentProvider for Git {
    fn provider(&self) -> &ProviderId {
        &self.id
    }

    async fn fetch(
        &self,
        id: &ExternalId,
        budget: &Budget,
    ) -> Result<Option<Fetched>, ContentError> {
        crate::spend(budget)?;
        let Some(bytes) = crate::local_file::read(&self.root, id)? else {
            return Ok(None);
        };
        let version = blob_version_within(&self.root, id.as_str(), budget).await?;
        Ok(Some(Fetched {
            version: Version::new(version),
            bytes,
        }))
    }

    fn history(&self) -> Option<&dyn History> {
        Some(self)
    }
}

#[async_trait]
impl History for Git {
    async fn fetch_at(
        &self,
        _id: &ExternalId,
        version: &Version,
        budget: &Budget,
    ) -> Result<Option<Vec<u8>>, ContentError> {
        let mut command = Command::new("git");
        command
            .args(["cat-file", "blob", version.as_str()])
            .current_dir(&self.root);
        let out = crate::within(command, budget).await?;
        if !out.status.success() {
            return Ok(None);
        }
        Ok(Some(out.stdout))
    }
}

fn hash_object(root: &Path, relative: &[&str]) -> Command {
    let mut command = Command::new("git");
    command
        .args(["hash-object", "--"])
        .args(relative)
        .current_dir(root);
    command
}

fn hashes_in(out: &std::process::Output, wanted: usize) -> Result<Vec<String>, ContentError> {
    if !out.status.success() {
        return Err(ContentError::new(format!(
            "cannot compute the version for {wanted} path(s): {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    let hashes: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.trim().to_owned())
        .collect();
    match hashes.len() == wanted {
        true => Ok(hashes),
        false => Err(ContentError::new(format!(
            "git returned {} version(s) for {wanted} path(s)",
            hashes.len()
        ))),
    }
}

async fn blob_version_within(
    root: &Path,
    relative: &str,
    budget: &Budget,
) -> Result<String, ContentError> {
    let out = crate::within(hash_object(root, std::slice::from_ref(&relative)), budget).await?;
    hashes_in(&out, 1)?
        .pop()
        .ok_or_else(|| ContentError::new(format!("git said nothing about `{relative}`")))
}

const ARGS_PER_CALL: usize = 32 * 1024;

fn first_batch<'a>(paths: &'a [&'a str]) -> (&'a [&'a str], &'a [&'a str]) {
    let mut take = 0;
    let mut bytes = 0;
    while take < paths.len() && (take == 0 || bytes + paths[take].len() < ARGS_PER_CALL) {
        bytes += paths[take].len() + 1;
        take += 1;
    }
    paths.split_at(take)
}

pub fn blob_versions(root: &Path, relative: &[&str]) -> Result<Vec<String>, ContentError> {
    let mut out = Vec::with_capacity(relative.len());
    let mut rest = relative;
    while !rest.is_empty() {
        let (batch, tail) = first_batch(rest);
        let answer = hash_object(root, batch)
            .output()
            .map_err(|e| ContentError::new(format!("cannot run git: {e}")))?;
        out.extend(hashes_in(&answer, batch.len())?);
        rest = tail;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn versioned(root: &Path, name: &str, bytes: &str) {
        std::fs::write(root.join(name), bytes).unwrap();
        for args in [["init", "-q"].as_slice(), ["add", name].as_slice()] {
            Command::new("git")
                .args(args)
                .current_dir(root)
                .status()
                .unwrap();
        }
    }

    #[test]
    fn more_paths_than_one_command_line_holds_are_still_all_versioned() {
        let dir = tempfile::tempdir().unwrap();
        let names: Vec<String> = (0..200).map(|n| format!("{n:0>200}.md")).collect();
        for (n, name) in names.iter().enumerate() {
            std::fs::write(dir.path().join(name), format!("body {n}")).unwrap();
        }
        let paths: Vec<&str> = names.iter().map(String::as_str).collect();

        let versions = blob_versions(dir.path(), &paths).unwrap();

        assert_eq!(
            versions.len(),
            paths.len(),
            "every path handed in gets a version back or none does. A repository grows notes \
             until one `git hash-object` exceeds the argument limit, and from that day on \
             every verb that versions the whole corpus fails at once"
        );
        let last = paths.len() - 1;
        assert!(
            first_batch(&paths).0.len() <= last,
            "this corpus has to span more than one call or it proves nothing"
        );
        let alone = blob_versions(dir.path(), &paths[last..]).unwrap();
        assert_eq!(
            versions[last], alone[0],
            "the versions come back in the order the paths went in, across batch boundaries. \
             Off by one batch, every note is stamped with its neighbour's version — and each \
             then reports as rewritten with a bound version nothing can retrieve"
        );
    }

    #[test]
    fn no_paths_asks_git_nothing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(blob_versions(dir.path(), &[]).unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_record_too_big_for_a_pipe_is_read_back_not_waited_out() {
        let dir = tempfile::tempdir().unwrap();
        let big = "x".repeat(1024 * 1024);
        versioned(dir.path(), "big.md", &big);

        let git = Git::new(dir.path());
        let budget = Budget::within(Duration::from_secs(10), usize::MAX);
        let bound = git
            .fetch(&ExternalId::new("big.md"), &budget)
            .await
            .unwrap()
            .unwrap();

        let started = Instant::now();
        let back = git
            .fetch_at(&ExternalId::new("big.md"), &bound.version, &budget)
            .await;

        assert_eq!(
            back.unwrap().as_deref(),
            Some(big.as_bytes()),
            "reading a child's output only after it exits is a deadlock the moment the child \
             writes more than a pipe holds: it blocks on the write, never exits, and the \
             budget kills it. The record is fine and the store is local — but a note this \
             size reports as unreachable at binding time, and burns a whole call's budget \
             doing it"
        );
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "it answered, but only after waiting the deadline out"
        );
    }

    #[tokio::test]
    async fn a_call_still_running_when_the_budget_runs_out_is_killed() {
        let mut sleeper = Command::new("sleep");
        sleeper.arg("30");
        let budget = Budget::within(Duration::from_millis(50), usize::MAX);

        let started = Instant::now();
        let answer = crate::within(sleeper, &budget).await;

        assert!(
            answer.is_err(),
            "a budget that only decides whether to start is not a deadline: the call it let \
             through can then run for as long as it likes"
        );
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the budget was 50ms and the child asked for 30s; waiting it out means the bound \
             was never enforced, only reported"
        );
    }
}
