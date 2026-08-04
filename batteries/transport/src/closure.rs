use std::path::Path;

use gmr_core::{ProbeVersion, content_hash_of_bytes};

/// A file hashes to its content; a directory to every file under it, path and
/// bytes alike, so moving code between two of its files still moves the version.
///
/// `None` means the path could not be read — a caller that cannot say what a
/// rule is must not claim one.
pub fn of_path(path: &Path) -> Option<ProbeVersion> {
    let mut acc = Vec::new();
    absorb(path, path, &mut acc)?;
    Some(ProbeVersion::new(content_hash_of_bytes(&acc).into_inner()))
}

fn absorb(base: &Path, at: &Path, acc: &mut Vec<u8>) -> Option<()> {
    if at.is_dir() {
        let mut entries: Vec<_> = std::fs::read_dir(at).ok()?.flatten().collect();
        entries.sort_by_key(|e| e.path());
        for e in entries {
            absorb(base, &e.path(), acc)?;
        }
        return Some(());
    }
    let rel = at.strip_prefix(base).unwrap_or(at);
    acc.extend_from_slice(rel.to_string_lossy().as_bytes());
    acc.push(0);
    acc.extend_from_slice(&std::fs::read(at).ok()?);
    acc.push(0);
    Some(())
}
