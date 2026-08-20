use std::path::Path;

use gmr_core::{ProbeVersion, content_hash_of_bytes};

pub fn of_path(path: &Path) -> Option<ProbeVersion> {
    let mut acc = Vec::new();
    absorb(path, path, &mut acc)?;
    Some(ProbeVersion::of(content_hash_of_bytes(&acc)))
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
