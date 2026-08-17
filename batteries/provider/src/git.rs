use std::path::{Path, PathBuf};
use std::process::Command;

use async_trait::async_trait;
use gmr_content::{ContentError, ContentProvider, Fetched, History};
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
        let version =
            blob_version(&self.root, id.as_str()).map_err(|e| ContentError::new(e.to_string()))?;
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
        crate::spend(budget)?;
        let out = Command::new("git")
            .args(["cat-file", "blob", version.as_str()])
            .current_dir(&self.root)
            .output()
            .map_err(|e| ContentError::new(format!("cannot run git: {e}")))?;
        if !out.status.success() {
            return Ok(None);
        }
        Ok(Some(out.stdout))
    }
}

pub fn blob_version(root: &Path, relative: &str) -> Result<String, ContentError> {
    blob_versions(root, std::slice::from_ref(&relative))?
        .pop()
        .ok_or_else(|| ContentError::new(format!("git said nothing about `{relative}`")))
}

pub fn blob_versions(root: &Path, relative: &[&str]) -> Result<Vec<String>, ContentError> {
    if relative.is_empty() {
        return Ok(Vec::new());
    }
    let out = Command::new("git")
        .args(["hash-object", "--"])
        .args(relative)
        .current_dir(root)
        .output()
        .map_err(|e| ContentError::new(format!("cannot run git: {e}")))?;

    if !out.status.success() {
        return Err(ContentError::new(format!(
            "cannot compute the version for {} path(s): {}",
            relative.len(),
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    let hashes: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.trim().to_owned())
        .collect();
    match hashes.len() == relative.len() {
        true => Ok(hashes),
        false => Err(ContentError::new(format!(
            "git returned {} version(s) for {} path(s)",
            hashes.len(),
            relative.len()
        ))),
    }
}
