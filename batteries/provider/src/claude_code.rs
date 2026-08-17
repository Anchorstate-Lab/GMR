use std::path::{Path, PathBuf};

use async_trait::async_trait;
use gmr_content::{ContentError, ContentProvider, Fetched};
use gmr_core::{ExternalId, ProviderId, Version, content_hash_of_bytes};
use gmr_probe::Budget;

pub struct ClaudeMemory {
    root: PathBuf,
    id: ProviderId,
}

impl ClaudeMemory {
    pub fn new(project_root: impl AsRef<Path>) -> Result<Self, ContentError> {
        Ok(Self {
            root: memory_dir(project_root.as_ref())?,
            id: ProviderId::new("claude-code"),
        })
    }

    #[cfg(test)]
    fn at(memory_dir: impl Into<PathBuf>) -> Self {
        Self {
            root: memory_dir.into(),
            id: ProviderId::new("claude-code"),
        }
    }
}

fn memory_dir(project_root: &Path) -> Result<PathBuf, ContentError> {
    if let Ok(over) = std::env::var("GMR_CLAUDE_MEMORY_DIR") {
        return Ok(PathBuf::from(over));
    }
    let home = std::env::var("HOME").map_err(|_| {
        ContentError::new("cannot find the claude-code memory directory: $HOME is not set")
    })?;
    let absolute = project_root.canonicalize().map_err(|e| {
        ContentError::new(format!("cannot resolve `{}`: {e}", project_root.display()))
    })?;
    let mangled = absolute.display().to_string().replace('/', "-");
    Ok(PathBuf::from(home)
        .join(".claude/projects")
        .join(mangled)
        .join("memory"))
}

#[async_trait]
impl ContentProvider for ClaudeMemory {
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
        let version = content_hash_of_bytes(&bytes);
        Ok(Some(Fetched {
            version: Version::new(version.into_inner()),
            bytes,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plenty() -> Budget {
        Budget::within(std::time::Duration::from_secs(30), usize::MAX)
    }

    #[tokio::test]
    async fn fetches_a_file_that_exists() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("feedback.md"), b"be terse").unwrap();
        let provider = ClaudeMemory::at(dir.path());

        let fetched = provider
            .fetch(&ExternalId::new("feedback.md"), &plenty())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(fetched.bytes, b"be terse");
    }

    #[tokio::test]
    async fn a_missing_file_is_none_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let provider = ClaudeMemory::at(dir.path());

        let fetched = provider
            .fetch(&ExternalId::new("absent.md"), &plenty())
            .await
            .unwrap();

        assert!(fetched.is_none());
    }

    #[tokio::test]
    async fn version_changes_when_the_content_does() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("note.md");
        let provider = ClaudeMemory::at(dir.path());

        std::fs::write(&path, b"first").unwrap();
        let v1 = provider
            .fetch(&ExternalId::new("note.md"), &plenty())
            .await
            .unwrap()
            .unwrap()
            .version;

        std::fs::write(&path, b"second").unwrap();
        let v2 = provider
            .fetch(&ExternalId::new("note.md"), &plenty())
            .await
            .unwrap()
            .unwrap()
            .version;

        assert_ne!(v1, v2);
    }

    #[test]
    fn this_provider_offers_no_history_at_all() {
        let dir = tempfile::tempdir().unwrap();
        let provider = ClaudeMemory::at(dir.path());

        assert!(provider.history().is_none());
    }

    #[test]
    fn env_override_bypasses_the_directory_guess() {
        unsafe {
            std::env::set_var("GMR_CLAUDE_MEMORY_DIR", "/tmp/wherever");
        }
        let dir = memory_dir(Path::new(".")).unwrap();
        unsafe {
            std::env::remove_var("GMR_CLAUDE_MEMORY_DIR");
        }
        assert_eq!(dir, PathBuf::from("/tmp/wherever"));
    }
}
