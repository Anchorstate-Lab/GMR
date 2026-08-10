use std::collections::BTreeMap;
use std::path::Path;

use gmr_probe::Budget;
use gmr_survey as coord;
use serde_json::{Value, json};

const VERSION: &str = env!("GMR_EXTRACTOR_ADDR");

pub(crate) const ITEMS: [&str; 3] = ["path", "name", "fingerprint"];

fn collect(path: &Path, rel: &str, out: &mut Vec<coord::Candidate>) -> Result<(), String> {
    let Ok(bytes) = std::fs::read(path) else {
        return Ok(());
    };
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let c: BTreeMap<String, String> = [
        ("path", rel.to_owned()),
        ("name", name),
        ("fingerprint", coord::hash(&String::from_utf8_lossy(&bytes))),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v))
    .collect();
    out.push(coord::Candidate::new(
        c["path"].clone(),
        c,
        json!({ "bytes": bytes.len() }),
    ));
    Ok(())
}

pub fn probe(
    root: &Path,
    pos: &Value,
    cache: &coord::Cache,
    budget: &Budget,
) -> Result<Value, coord::Halt> {
    let want = coord::wanted(pos, &ITEMS)?;
    let cands = coord::visit_cached(root, cache, "addr-map", budget, collect)?;
    if cands.is_empty() {
        return Err(coord::Halt::Refused(format!(
            "{} contains no files; the probe is likely pointed at the wrong directory",
            root.display()
        )));
    }
    Ok(coord::report(VERSION, &want, coord::nth(pos), &cands)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roomy() -> Budget {
        Budget::within(std::time::Duration::from_secs(600), 1 << 24)
    }

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
        probe(dir, &pos, &coord::Cache::disabled(), &roomy()).unwrap()
    }

    const BODY: &str = "Anchoring is an output of design work\n";

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
        let d = fixture("changed", &[("doc/a.md", "changed\n")]);
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
        let d = fixture(
            "twin",
            &[("doc/a.md", "changed\n"), ("attic/copy.md", BODY)],
        );
        let v = at(
            &d,
            json!({"path": "doc/a.md", "fingerprint": coord::hash(BODY)}),
        );
        assert_eq!(v["missed"], json!(["fingerprint"]));
        assert_eq!(v["at"]["path"], "doc/a.md");
    }

    #[test]
    fn an_address_that_is_gone_leaves_both_items_missed() {
        let d = fixture("gone", &[("other.md", "something else\n")]);
        let v = at(
            &d,
            json!({"path": "doc/a.md", "fingerprint": coord::hash(BODY)}),
        );
        assert_eq!(v["found"], false);
    }

    #[test]
    fn an_empty_position_is_our_failure_not_the_worlds_answer() {
        let d = fixture("empty", &[("a.md", BODY)]);
        assert!(probe(&d, &json!({}), &coord::Cache::disabled(), &roomy()).is_err());
    }

    #[test]
    fn an_empty_tree_is_our_failure_too() {
        let d = fixture("bare", &[]);
        assert!(
            probe(
                &d,
                &json!({"path": "a.md"}),
                &coord::Cache::disabled(),
                &roomy()
            )
            .is_err()
        );
    }

    #[test]
    fn the_extractor_hashes_its_own_source() {
        let d = fixture("version", &[("a.md", BODY)]);
        let v = at(&d, json!({"path": "a.md"}));
        assert_eq!(v["extractor"], VERSION);
        assert_eq!(v["extractor"].as_str().unwrap().len(), 64);
    }
}
