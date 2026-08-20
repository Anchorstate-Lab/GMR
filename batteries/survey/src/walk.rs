use std::path::Path;

use sha2::{Digest, Sha256};

pub fn hash(s: &str) -> String {
    format!("{:x}", Sha256::digest(s.as_bytes()))
}

pub fn sort_key(rel: &str) -> String {
    rel.replace('/', "\u{0}")
}

pub fn visit(
    root: &Path,
    each: &mut impl FnMut(&Path, &str) -> Result<(), String>,
) -> Result<(), String> {
    descend(root, root, each)
}

fn descend(
    dir: &Path,
    base: &Path,
    each: &mut impl FnMut(&Path, &str) -> Result<(), String>,
) -> Result<(), String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(());
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| e.path());
    for e in entries {
        let path = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || name == "target" || name == "node_modules" {
            continue;
        }
        if path.is_dir() {
            descend(&path, base, each)?;
        } else if let Ok(rel) = path.strip_prefix(base) {
            each(&path, &rel.to_string_lossy())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(files: &[&str]) -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        for f in files {
            let p = d.path().join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, "x").unwrap();
        }
        d
    }

    fn seen(d: &tempfile::TempDir) -> Vec<String> {
        let mut out = Vec::new();
        visit(d.path(), &mut |_, rel| {
            out.push(rel.replace('\\', "/"));
            Ok(())
        })
        .unwrap();
        out
    }

    #[test]
    fn build_output_is_not_the_repository() {
        let d = tree(&["src/a.rs", "target/b.rs", "node_modules/c.js", ".git/d"]);
        assert_eq!(seen(&d), vec!["src/a.rs"]);
    }

    #[test]
    fn the_order_is_the_same_on_every_run() {
        let d = tree(&["b.rs", "a.rs", "z/y.rs", "z/a.rs"]);
        assert_eq!(seen(&d), vec!["a.rs", "b.rs", "z/a.rs", "z/y.rs"]);
    }

    #[test]
    fn a_refusal_stops_the_walk() {
        let d = tree(&["a.rs", "b.rs"]);
        let e = visit(d.path(), &mut |_, rel| Err(format!("no: {rel}"))).unwrap_err();
        assert_eq!(e, "no: a.rs");
    }

    fn laid_out() -> (tempfile::TempDir, Vec<String>) {
        let dir = tempfile::tempdir().unwrap();
        for rel in [
            "b.rs",
            "b/x.rs",
            "index.ts",
            "index/a.ts",
            "mod.rs",
            "mod/a.rs",
            "pkg.py",
            "pkg/__init__.py",
            "deep/a.rs",
            "deep/a/b.rs",
            "plain.rs",
            "other/one.rs",
        ] {
            let at = dir.path().join(rel);
            std::fs::create_dir_all(at.parent().unwrap()).unwrap();
            std::fs::write(&at, "x").unwrap();
        }

        let mut walked = Vec::new();
        visit(dir.path(), &mut |_, rel| {
            walked.push(rel.replace('\\', "/"));
            Ok(())
        })
        .unwrap();
        (dir, walked)
    }

    #[test]
    fn the_sort_key_reproduces_the_order_the_walk_hands_files_over_in() {
        let (_dir, walked) = laid_out();
        let mut keyed = walked.clone();
        keyed.sort_by_key(|rel| sort_key(rel));

        assert_eq!(
            walked, keyed,
            "the index only ever sorts by this key, so the day it disagrees with the walk \
             is the day `nth` starts naming a different candidate with nobody having \
             touched the code"
        );
    }

    #[test]
    fn sorting_the_same_paths_by_their_bytes_would_not_have_agreed() {
        let (_dir, walked) = laid_out();
        let mut by_bytes = walked.clone();
        by_bytes.sort();

        assert_ne!(
            walked, by_bytes,
            "a layout where a file and a directory share a stem is the whole reason this \
             key exists; if byte order happens to agree, the fixture stopped covering the \
             case and the test above proves nothing"
        );
    }
}
