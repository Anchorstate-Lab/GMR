use std::path::Path;

use gmr_content::ContentError;
use gmr_core::ExternalId;

pub(crate) fn read(root: &Path, id: &ExternalId) -> Result<Option<Vec<u8>>, ContentError> {
    let path = root.join(id.as_str());
    if !path.exists() {
        if !root.is_dir() {
            return Err(ContentError::new(format!(
                "`{}` is not a directory, so nothing under it can be read — including `{}`. \
                 This is a store that is not there, not a record that was deleted",
                root.display(),
                id.as_str()
            )));
        }
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

    #[test]
    fn a_root_that_is_not_there_is_our_failure_not_the_worlds_answer() {
        let dir = tempfile::tempdir().unwrap();

        let answer = read(&dir.path().join("never-created"), &ExternalId::new("a.md"));

        assert!(
            answer.is_err(),
            "a missing file inside a store that exists means the record was deleted. A \
             missing store means every record under it reads as deleted at once, and \
             `doctor` tells the reader to remove bindings that are all still fine. \
             claude-code reaches this every time: its memory directory is only created \
             once a session has written there, so a project without one used to answer \
             `gone` for every binding"
        );
    }
}
