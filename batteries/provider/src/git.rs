use std::path::{Path, PathBuf};
use std::process::Command;

use async_trait::async_trait;
use gmr_content::{ContentError, ContentProvider, Fetched, History};
use gmr_core::{ExternalId, ProviderId, Version};

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

    async fn fetch(&self, id: &ExternalId) -> Result<Option<Fetched>, ContentError> {
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
    ) -> Result<Option<Vec<u8>>, ContentError> {
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
    let out = Command::new("git")
        .args(["hash-object", "--", relative])
        .current_dir(root)
        .output()
        .map_err(|e| ContentError::new(format!("cannot run git: {e}")))?;

    if !out.status.success() {
        return Err(ContentError::new(format!(
            "cannot compute the version for `{relative}`: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}
