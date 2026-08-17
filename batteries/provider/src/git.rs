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
        let version = blob_version_within(&self.root, id.as_str(), budget)?;
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
        let out = crate::within(command, budget)?;
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

fn blob_version_within(
    root: &Path,
    relative: &str,
    budget: &Budget,
) -> Result<String, ContentError> {
    crate::within(hash_object(root, std::slice::from_ref(&relative)), budget)
        .and_then(|out| hashes_in(&out, 1))?
        .pop()
        .ok_or_else(|| ContentError::new(format!("git said nothing about `{relative}`")))
}

pub fn blob_versions(root: &Path, relative: &[&str]) -> Result<Vec<String>, ContentError> {
    if relative.is_empty() {
        return Ok(Vec::new());
    }
    let out = hash_object(root, relative)
        .output()
        .map_err(|e| ContentError::new(format!("cannot run git: {e}")))?;
    hashes_in(&out, relative.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn a_call_still_running_when_the_budget_runs_out_is_killed() {
        let mut sleeper = Command::new("sleep");
        sleeper.arg("30");
        let budget = Budget::within(Duration::from_millis(50), usize::MAX);

        let started = Instant::now();
        let answer = crate::within(sleeper, &budget);

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
