use std::collections::BTreeMap;
use std::path::Path;

use gmr_probe_coord as coord;
use serde_json::{json, Value};

const SELF_SRC: &str = include_str!("main.rs");

const ITEMS: [&str; 3] = ["path", "name", "fingerprint"];

fn extractor() -> String {
    coord::hash(SELF_SRC)
}

fn walk(dir: &Path, base: &Path, out: &mut Vec<coord::Candidate>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| e.path());
    for e in entries {
        let p = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || name == "target" || name == "node_modules" {
            continue;
        }
        if p.is_dir() {
            walk(&p, base, out);
            continue;
        }
        let (Ok(rel), Ok(bytes)) = (p.strip_prefix(base), std::fs::read(&p)) else {
            continue;
        };
        let fp = coord::hash(&String::from_utf8_lossy(&bytes));
        let c: BTreeMap<String, String> = [
            ("path", rel.to_string_lossy().into_owned()),
            ("name", name.clone()),
            ("fingerprint", fp),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .collect();
        out.push(coord::Candidate::new(c, json!({ "bytes": bytes.len() })));
    }
}

fn probe(root: &Path, pos: &Value) -> Result<Value, String> {
    let want = coord::wanted(pos, &ITEMS)?;
    let mut cands = Vec::new();
    walk(root, root, &mut cands);
    if cands.is_empty() {
        return Err(format!(
            "{} 底下一个文件都没有 —— 更可能是我站错了目录",
            root.display()
        ));
    }
    coord::report(&extractor(), &want, coord::nth(pos), &cands)
}

fn main() {
    coord::emit(
        coord::params()
            .and_then(|params| Ok((coord::root(&params), coord::position()?)))
            .and_then(|(root, pos)| probe(Path::new(&root), &pos)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str, files: &[(&str, &str)]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("addr-map-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        for (path, body) in files {
            let p = dir.join(path);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, body).unwrap();
        }
        dir
    }

    fn at(dir: &Path, pos: Value) -> Value {
        probe(dir, &pos).unwrap()
    }

    const BODY: &str = "锚定是设计工作的产出\n";

    #[test]
    fn an_address_that_is_still_there_misses_nothing() {
        let d = fixture("there", &[("doc/a.md", BODY)]);
        let v = at(
            &d,
            json!({"path": "doc/a.md", "fingerprint": coord::hash(BODY)}),
        );
        assert_eq!(v["missed"], json!([]));
        assert_eq!(v["facts"]["bytes"], BODY.len());
    }

    #[test]
    fn a_changed_body_reports_only_fingerprint_missed() {
        let d = fixture("changed", &[("doc/a.md", "改过了\n")]);
        let v = at(
            &d,
            json!({"path": "doc/a.md", "fingerprint": coord::hash(BODY)}),
        );
        assert_eq!(v["missed"], json!(["fingerprint"]));
        assert_eq!(v["candidates"], 1);
    }

    #[test]
    fn a_moved_file_is_found_by_its_fingerprint() {
        let d = fixture("moved", &[("docs/new.md", BODY)]);
        let v = at(
            &d,
            json!({"path": "doc/a.md", "fingerprint": coord::hash(BODY)}),
        );
        assert_eq!(v["missed"], json!(["path"]));
        assert_eq!(v["candidates"], 1);
        assert_eq!(v["at"]["path"], "docs/new.md");
    }

    #[test]
    fn the_same_address_outranks_a_coincidental_twin_elsewhere() {
        let d = fixture("twin", &[("doc/a.md", "改过了\n"), ("attic/copy.md", BODY)]);
        let v = at(
            &d,
            json!({"path": "doc/a.md", "fingerprint": coord::hash(BODY)}),
        );
        assert_eq!(v["missed"], json!(["fingerprint"]));
        assert_eq!(v["at"]["path"], "doc/a.md");
    }

    #[test]
    fn an_address_that_is_gone_leaves_both_items_missed() {
        let d = fixture("gone", &[("other.md", "别的东西\n")]);
        let v = at(
            &d,
            json!({"path": "doc/a.md", "fingerprint": coord::hash(BODY)}),
        );
        assert_eq!(v["found"], false);
    }

    #[test]
    fn an_empty_position_is_our_failure_not_the_worlds_answer() {
        let d = fixture("empty", &[("a.md", BODY)]);
        assert!(probe(&d, &json!({})).is_err());
    }

    #[test]
    fn an_empty_tree_is_our_failure_too() {
        let d = fixture("bare", &[]);
        assert!(probe(&d, &json!({"path": "a.md"})).is_err());
    }

    #[test]
    fn the_extractor_hashes_its_own_source() {
        let d = fixture("version", &[("a.md", BODY)]);
        let v = at(&d, json!({"path": "a.md"}));
        assert_eq!(v["extractor"], extractor());
        assert_eq!(v["extractor"].as_str().unwrap().len(), 64);
    }
}
