use std::path::Path;

use sha2::{Digest, Sha256};

pub fn hash(s: &str) -> String {
    format!("{:x}", Sha256::digest(s.as_bytes()))
}

/// Every file under `root`, in sorted order, as (absolute path, path relative
/// to root). Sorted so that two runs over an unchanged tree agree.
///
/// Skips dotfiles, `target` and `node_modules`: an extractor that walked into
/// them would report build output as if it were the repository.
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
}
