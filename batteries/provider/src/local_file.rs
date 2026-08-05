//! Shared by every provider backed by a plain file on local disk: join
//! root + id, check existence, read bytes. What counts as a version, and
//! whether history is retrievable, differs per backend and stays there —
//! this only owns the one mechanical step every such backend repeats.

use std::path::Path;

use gmr_content::ContentError;
use gmr_core::ExternalId;

pub(crate) fn read(root: &Path, id: &ExternalId) -> Result<Option<Vec<u8>>, ContentError> {
    let path = root.join(id.as_str());
    if !path.exists() {
        return Ok(None);
    }
    std::fs::read(&path)
        .map(Some)
        .map_err(|e| ContentError::new(format!("cannot read `{}`: {e}", id.as_str())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_file_that_exists() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.md"), b"hi").unwrap();

        let bytes = read(dir.path(), &ExternalId::new("a.md")).unwrap();

        assert_eq!(bytes, Some(b"hi".to_vec()));
    }

    #[test]
    fn a_missing_file_is_none_not_an_error() {
        let dir = tempfile::tempdir().unwrap();

        let bytes = read(dir.path(), &ExternalId::new("absent.md")).unwrap();

        assert!(bytes.is_none());
    }
}
