use std::path::{Path, PathBuf};

use async_trait::async_trait;
use gmr_content::{ContentError, ContentProvider, Fetched};
use gmr_core::{ExternalId, ProviderId, Version, content_hash_of_bytes};

/// Reads Claude Code's own per-project memory files as bindable content.
/// Read-only: this battery never writes into another product's data
/// directory, only observes it.
pub struct ClaudeMemory {
    root: PathBuf,
    id: ProviderId,
}

impl ClaudeMemory {
    /// `project_root` is the repository this binary runs against. The memory
    /// directory Claude Code keeps for it is derived from that path, the
    /// same way `Git` derives its root from it.
    pub fn new(project_root: impl AsRef<Path>) -> Result<Self, ContentError> {
        Ok(Self {
            root: memory_dir(project_root.as_ref())?,
            id: ProviderId::new("claude-code"),
        })
    }

    /// Test-only: points straight at a directory without going through
    /// `memory_dir`'s guess. Production callers use `GMR_CLAUDE_MEMORY_DIR`
    /// for the same purpose — that path is already covered by `new`, so this
    /// constructor has no reason to be public.
    #[cfg(test)]
    fn at(memory_dir: impl Into<PathBuf>) -> Self {
        Self {
            root: memory_dir.into(),
            id: ProviderId::new("claude-code"),
        }
    }
}

/// Claude Code's directory-naming convention for a project's data — the
/// absolute path with every `/` replaced by `-` — is an internal convention,
/// not a documented public contract. Verified empirically against this
/// machine's `~/.claude/projects/` (including a path whose own name already
/// contains hyphens, e.g. `.../moltbook-001` → `-...-moltbook-001`, so the
/// substitution isn't ambiguous going forward even though it isn't
/// reversible). `GMR_CLAUDE_MEMORY_DIR` overrides it outright if a future
/// Claude Code version changes the convention or the guess is otherwise
/// wrong.
fn memory_dir(project_root: &Path) -> Result<PathBuf, ContentError> {
    if let Ok(over) = std::env::var("GMR_CLAUDE_MEMORY_DIR") {
        return Ok(PathBuf::from(over));
    }
    let home = std::env::var("HOME").map_err(|_| {
        ContentError::new("cannot find the claude-code memory directory: $HOME is not set")
    })?;
    let absolute = project_root
        .canonicalize()
        .map_err(|e| ContentError::new(format!("cannot resolve `{}`: {e}", project_root.display())))?;
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

    async fn fetch(&self, id: &ExternalId) -> Result<Option<Fetched>, ContentError> {
        let path = self.root.join(id.as_str());
        if !path.exists() {
            return Ok(None);
        }
        let bytes = std::fs::read(&path)
            .map_err(|e| ContentError::new(format!("cannot read `{}`: {e}", id.as_str())))?;
        let version = content_hash_of_bytes(&bytes);
        Ok(Some(Fetched {
            version: Version::new(version.into_inner()),
            bytes,
        }))
    }

    /// Memory files carry no history of their own — an old version is simply
    /// not retrievable. `MemoryLens` already treats that as a legitimate
    /// answer (`retrievable: Some(false)`), not an error.
    async fn fetch_at(
        &self,
        _id: &ExternalId,
        _version: &Version,
    ) -> Result<Option<Vec<u8>>, ContentError> {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fetches_a_file_that_exists() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("feedback.md"), b"be terse").unwrap();
        let provider = ClaudeMemory::at(dir.path());

        let fetched = provider
            .fetch(&ExternalId::new("feedback.md"))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(fetched.bytes, b"be terse");
    }

    #[tokio::test]
    async fn a_missing_file_is_none_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let provider = ClaudeMemory::at(dir.path());

        let fetched = provider.fetch(&ExternalId::new("absent.md")).await.unwrap();

        assert!(fetched.is_none());
    }

    #[tokio::test]
    async fn version_changes_when_the_content_does() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("note.md");
        let provider = ClaudeMemory::at(dir.path());

        std::fs::write(&path, b"first").unwrap();
        let v1 = provider
            .fetch(&ExternalId::new("note.md"))
            .await
            .unwrap()
            .unwrap()
            .version;

        std::fs::write(&path, b"second").unwrap();
        let v2 = provider
            .fetch(&ExternalId::new("note.md"))
            .await
            .unwrap()
            .unwrap()
            .version;

        assert_ne!(v1, v2);
    }

    #[tokio::test]
    async fn fetch_at_honestly_reports_no_history() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("note.md"), b"content").unwrap();
        let provider = ClaudeMemory::at(dir.path());

        let bytes = provider
            .fetch_at(&ExternalId::new("note.md"), &Version::new("any"))
            .await
            .unwrap();

        assert!(bytes.is_none());
    }

    #[test]
    fn env_override_bypasses_the_directory_guess() {
        // SAFETY: single-threaded assertion around a process-global var; no
        // other test in this module touches GMR_CLAUDE_MEMORY_DIR.
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
