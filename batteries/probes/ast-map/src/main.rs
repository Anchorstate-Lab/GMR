mod lang;

use std::collections::BTreeMap;
use std::path::Path;

use gmr_probe_coord as coord;
use serde_json::{json, Value};

const SELF_SRC: &str = include_str!("main.rs");
const LANG_SRC: &str = include_str!("lang.rs");

const ITEMS: [&str; 5] = ["file", "kind", "vis", "name", "shape"];

fn extractor() -> String {
    coord::hash(&format!("{SELF_SRC}{LANG_SRC}"))
}

fn squeeze(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn collect(path: &Path, rel: &str, out: &mut Vec<coord::Candidate>) -> Result<(), String> {
    let Some(table) = lang::for_path(rel) else {
        return Ok(());
    };
    let Ok(src) = std::fs::read_to_string(path) else {
        return Ok(());
    };
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&(table.language)()).map_err(|e| {
        format!(
            "cannot install the parser for {rel}: {e}; this is my failure, not the world's answer"
        )
    })?;
    let tree = parser.parse(&src, None).ok_or_else(|| {
        format!("{rel} did not parse into a tree; this is my failure, not the world's answer")
    })?;

    let mut cursor = tree.walk();
    let mut stack = vec![tree.root_node()];
    while let Some(node) = stack.pop() {
        for c in node.named_children(&mut cursor) {
            stack.push(c);
        }
        let Some(kind) = lang::normalize(table, node.kind()) else {
            continue;
        };
        let text = |n: tree_sitter::Node| src.get(n.byte_range()).map(squeeze).unwrap_or_default();
        let name = node
            .child_by_field_name("name")
            .map(text)
            .or_else(|| node.child_by_field_name("function").map(text))
            .unwrap_or_default();
        let shape = table
            .shape_fields
            .iter()
            .filter_map(|f| node.child_by_field_name(f).map(text))
            .collect::<Vec<_>>()
            .join(" ");
        let body = node
            .child_by_field_name("body")
            .map(text)
            .unwrap_or_default();
        let mut vc = node.walk();
        let vis = node
            .named_children(&mut vc)
            .find(|k| k.kind() == "visibility_modifier")
            .map(text)
            .unwrap_or_default();

        let c: BTreeMap<String, String> = [
            ("file", rel.to_owned()),
            ("kind", kind.to_owned()),
            ("vis", vis),
            ("name", name),
            ("shape", shape),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .collect();
        let facts = json!({ "body": coord::hash(&body), "line": node.start_position().row + 1 });
        out.push(coord::Candidate::new(c, facts));
    }
    Ok(())
}

fn walk(dir: &Path, base: &Path, out: &mut Vec<coord::Candidate>) -> Result<(), String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(());
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
            walk(&p, base, out)?;
        } else if let Ok(rel) = p.strip_prefix(base) {
            collect(&p, &rel.to_string_lossy(), out)?;
        }
    }
    Ok(())
}

fn probe(root: &Path, pos: &Value) -> Result<Value, String> {
    let want = coord::wanted(pos, &ITEMS)?;
    let mut cands = Vec::new();
    walk(root, root, &mut cands)?;
    if cands.is_empty() {
        return Err(format!(
            "{} contains no parseable nodes; the probe is likely pointed at the wrong directory",
            root.display()
        ));
    }
    coord::report(&extractor(), &want, coord::nth(pos), &cands)
}

fn main() -> std::process::ExitCode {
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
        let dir = std::env::temp_dir().join(format!("ast-map-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        for (path, body) in files {
            let p = dir.join(path);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, body).unwrap();
        }
        dir
    }

    const ONE: &str =
        "pub fn alpha(x: u8) -> u8 { x }\npub fn beta(s: &str) -> usize { s.len() }\n";

    fn at(dir: &Path, pos: Value) -> Value {
        probe(dir, &pos).unwrap()
    }

    #[test]
    fn an_exact_coordinate_matches_every_item() {
        let d = fixture("exact", &[("a.rs", ONE)]);
        let v = at(
            &d,
            json!({"file": "a.rs", "kind": "function", "name": "alpha"}),
        );
        assert_eq!(v["found"], true);
        assert_eq!(v["matched"], json!(["file", "kind", "name"]));
        assert_eq!(v["missed"], json!([]));
    }

    #[test]
    fn a_renamed_node_keeps_its_shape_and_reports_only_name_missed() {
        let d = fixture("renamed", &[("a.rs", ONE)]);
        let v = at(
            &d,
            json!({"file": "a.rs", "kind": "function", "name": "gone",
                   "shape": "(x: u8) u8"}),
        );
        assert_eq!(v["missed"], json!(["name"]));
        assert_eq!(v["candidates"], 1);
        assert_eq!(v["at"]["name"], "alpha");
    }

    #[test]
    fn a_moved_node_reports_only_file_missed_and_at_points_at_the_new_home() {
        let d = fixture("moved", &[("moved/b.rs", ONE)]);
        let v = at(
            &d,
            json!({"file": "a.rs", "kind": "function", "name": "alpha"}),
        );
        assert_eq!(v["missed"], json!(["file"]));
        assert_eq!(v["at"]["file"], "moved/b.rs");
    }

    #[test]
    fn a_contract_drift_reports_only_shape_missed() {
        let d = fixture(
            "drift",
            &[("a.rs", "pub fn alpha(x: u8, y: u8) -> u8 { x }\n")],
        );
        let v = at(
            &d,
            json!({"file": "a.rs", "kind": "function", "name": "alpha",
                   "shape": "(x: u8) u8"}),
        );
        assert_eq!(v["missed"], json!(["shape"]));
        assert_eq!(v["at"]["shape"], "(x: u8, y: u8) u8");
    }

    #[test]
    fn a_deletion_is_told_apart_from_a_rename_by_how_many_candidates_tied() {
        let d = fixture("deleted", &[("a.rs", ONE)]);
        let v = at(
            &d,
            json!({"file": "a.rs", "kind": "function", "name": "gone",
                   "shape": "(q: NoSuchType) Nothing"}),
        );
        assert_eq!(v["missed"], json!(["name", "shape"]));
        assert_eq!(v["candidates"], 2);
    }

    #[test]
    fn nothing_matching_at_all_is_found_false() {
        let d = fixture("absent", &[("a.rs", ONE)]);
        let v = at(&d, json!({"file": "nope.rs", "name": "nope"}));
        assert_eq!(v["found"], false);
        assert_eq!(v["at"], Value::Null);
    }

    #[test]
    fn an_empty_position_is_our_failure_not_the_worlds_answer() {
        let d = fixture("empty", &[("a.rs", ONE)]);
        assert!(probe(&d, &json!({})).is_err());
    }

    #[test]
    fn a_directory_with_nothing_parseable_is_our_failure_too() {
        let d = fixture("bare", &[("readme.txt", "not code")]);
        assert!(probe(&d, &json!({"name": "alpha"})).is_err());
    }

    #[test]
    fn the_extractor_hashes_its_own_source() {
        let d = fixture("version", &[("a.rs", ONE)]);
        let v = at(&d, json!({"name": "alpha"}));
        assert_eq!(v["extractor"], extractor());
        assert_eq!(v["extractor"].as_str().unwrap().len(), 64);
    }

    #[test]
    fn types_and_fields_normalize_across_their_native_kinds() {
        let d = fixture(
            "kinds",
            &[("a.rs", "pub struct S { pub n: u8 }\npub enum E { A }\n")],
        );
        assert_eq!(
            at(&d, json!({"kind": "type", "name": "S"}))["missed"],
            json!([])
        );
        assert_eq!(
            at(&d, json!({"kind": "type", "name": "E"}))["missed"],
            json!([])
        );
        assert_eq!(
            at(&d, json!({"kind": "field", "name": "n"}))["missed"],
            json!([])
        );
    }
}
