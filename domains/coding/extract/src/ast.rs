use crate::lang;
use std::collections::BTreeMap;
use std::path::Path;

use gmr_survey as coord;
use serde_json::{Value, json};

const VERSION: &str = env!("GMR_EXTRACTOR_AST");

pub(crate) const ITEMS: [&str; 7] = ["file", "kind", "vis", "name", "callee", "member", "shape"];

fn squeeze(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn named_by_parent<'t>(
    table: &lang::Table,
    node: tree_sitter::Node<'t>,
) -> Option<tree_sitter::Node<'t>> {
    let parent = node.parent()?;
    if !table.name_from_parent.contains(&parent.kind()) {
        return None;
    }
    parent
        .child_by_field_name("name")
        .or_else(|| parent.child_by_field_name("key"))
}

fn visibility(
    table: &lang::Table,
    node: tree_sitter::Node,
    name: &str,
    text: &impl Fn(tree_sitter::Node) -> String,
) -> String {
    match &table.vis {
        lang::Vis::Absent => String::new(),
        lang::Vis::Child(kind) => {
            let mut c = node.walk();
            node.named_children(&mut c)
                .find(|k| k.kind() == *kind)
                .map(text)
                .unwrap_or_default()
        }
        lang::Vis::Ancestor { kind, label } => {
            let mut at = node.parent();
            while let Some(n) = at {
                if n.kind() == *kind {
                    return (*label).to_owned();
                }
                if n.child_by_field_name("body").is_some() {
                    break;
                }
                at = n.parent();
            }
            String::new()
        }
        lang::Vis::LeadingUpper(label) => {
            match name.chars().next().is_some_and(char::is_uppercase) {
                true => (*label).to_owned(),
                false => String::new(),
            }
        }
    }
}

fn members(node: tree_sitter::Node, src: &str) -> (String, String) {
    let Some(body) = node.child_by_field_name("body") else {
        return (String::new(), String::new());
    };
    let mut walk = body.walk();
    let (mut declared, mut implemented) = (Vec::new(), Vec::new());
    for m in body.named_children(&mut walk) {
        let inner = m.child_by_field_name("body");
        let head = inner.map_or(m.end_byte(), |b| b.start_byte());
        if let Some(t) = src.get(m.start_byte()..head) {
            declared.push(squeeze(t));
        }
        if let Some(t) = inner.and_then(|b| src.get(b.byte_range())) {
            implemented.push(squeeze(t));
        }
    }
    (declared.join("; "), implemented.join(" "))
}

fn attr_head(text: &str) -> &str {
    let rest = text.trim_start_matches(['#', '!', '[', '@']);
    let end = rest
        .find(|c: char| !c.is_alphanumeric() && c != '_')
        .unwrap_or(rest.len());
    &rest[..end]
}

fn attributes(table: &lang::Table, node: tree_sitter::Node, src: &str) -> Vec<String> {
    let lang::Attrs::Before(kind) = table.attrs else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut at = node.prev_named_sibling();
    while let Some(n) = at {
        if n.kind() != kind {
            break;
        }
        if let Some(t) = src.get(n.byte_range()) {
            let t = squeeze(t);
            if !lang::NOISE.contains(&attr_head(&t)) {
                out.push(t);
            }
        }
        at = n.prev_named_sibling();
    }
    out.reverse();
    out
}

fn naming(kind: &str) -> &'static str {
    match kind {
        "call" | "import" => "callee",
        "field" => "member",
        _ => "name",
    }
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
    let mut here: Vec<(usize, coord::Candidate)> = Vec::new();
    while let Some(node) = stack.pop() {
        for c in node.named_children(&mut cursor) {
            stack.push(c);
        }
        let Some(kind) = lang::normalize(table, node.kind()) else {
            continue;
        };
        let text = |n: tree_sitter::Node| src.get(n.byte_range()).map(squeeze).unwrap_or_default();
        let name = table
            .names
            .iter()
            .find_map(|f| node.child_by_field_name(f).map(text))
            .or_else(|| named_by_parent(table, node).map(text))
            .filter(|n| !n.is_empty())
            .unwrap_or_default();
        let (declared, implemented) = match kind == "type" {
            true => members(node, &src),
            false => (String::new(), String::new()),
        };
        let mut sig = Vec::new();
        let mut walk = node.walk();
        sig.extend(
            node.children(&mut walk)
                .filter(|c| table.shape_kinds.contains(&c.kind()))
                .map(text),
        );
        sig.extend(
            table
                .shape_fields
                .iter()
                .filter_map(|f| node.child_by_field_name(f).map(text)),
        );
        if !declared.is_empty() {
            sig.push(declared);
        }
        let body = match kind == "type" {
            true => implemented,
            false => table
                .body_fields
                .iter()
                .find_map(|f| node.child_by_field_name(f))
                .map(text)
                .unwrap_or_default(),
        };
        let vis = visibility(table, node, &name, &text);
        let mut surface = attributes(table, node, &src);
        if !vis.is_empty() {
            surface.insert(0, vis.clone());
        }

        let c: BTreeMap<String, String> = [
            ("file", rel.to_owned()),
            ("kind", kind.to_owned()),
            ("form", node.kind().to_owned()),
            ("vis", vis),
            ("surface", surface.join(" ")),
            (naming(kind), name),
            ("shape", sig.join(" ")),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .collect();
        let facts = json!({ "body": coord::hash(&body), "line": node.start_position().row + 1 });
        here.push((
            node.start_byte(),
            coord::Candidate::new(format!("{kind}:{}", c[naming(kind)]), c, facts),
        ));
    }

    let mut named: Vec<(usize, String)> = here
        .iter()
        .filter_map(|(at, c)| c.coord.get("name").map(|n| (*at, n.clone())))
        .collect();
    named.sort();
    for (at, c) in &mut here {
        let before = named.partition_point(|(s, _)| s < at);
        let after = match before {
            0 => String::new(),
            n => named[n - 1].1.clone(),
        };
        c.coord.insert("after".to_owned(), after);
    }
    out.extend(here.into_iter().map(|(_, c)| c));
    Ok(())
}

pub fn probe(root: &Path, pos: &Value, cache: &coord::Cache) -> Result<Value, String> {
    let want = coord::wanted(pos, &ITEMS)?;
    let cands = coord::visit_cached(root, cache, "ast-map", collect)?;
    if cands.is_empty() {
        return Err(format!(
            "{} contains no parseable nodes; the probe is likely pointed at the wrong directory",
            root.display()
        ));
    }
    coord::report(VERSION, &want, coord::nth(pos), &cands)
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
        probe(dir, &pos, &coord::Cache::disabled()).unwrap()
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
    fn a_call_site_does_not_tie_with_the_definition_it_points_at() {
        let d = fixture(
            "mentions",
            &[(
                "a.rs",
                "pub fn alpha() -> u8 { 1 }\npub fn beta() -> u8 { alpha() }\n",
            )],
        );
        let v = at(&d, json!({"file": "a.rs", "name": "alpha"}));
        assert_eq!(v["candidates"], 1);
        assert_eq!(v["at"]["kind"], "function");

        let calls = at(&d, json!({"file": "a.rs", "callee": "alpha"}));
        assert_eq!(calls["candidates"], 1);
        assert_eq!(calls["at"]["kind"], "call");
    }

    #[test]
    fn a_field_does_not_tie_with_a_function_of_the_same_name() {
        let d = fixture(
            "member",
            &[(
                "a.rs",
                "pub struct S { pub reason: u8 }\nimpl S { pub fn reason(&self) -> u8 { 0 } }\n",
            )],
        );
        let v = at(&d, json!({"file": "a.rs", "name": "reason"}));
        assert_eq!(v["candidates"], 1);
        assert_eq!(v["at"]["kind"], "function");
        assert_eq!(
            at(&d, json!({"file": "a.rs", "member": "reason"}))["at"]["kind"],
            "field"
        );
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
        assert!(probe(&d, &json!({}), &coord::Cache::disabled()).is_err());
    }

    #[test]
    fn a_directory_with_nothing_parseable_is_our_failure_too() {
        let d = fixture("bare", &[("readme.txt", "not code")]);
        assert!(probe(&d, &json!({"name": "alpha"}), &coord::Cache::disabled()).is_err());
    }

    #[test]
    fn the_extractor_hashes_its_own_source() {
        let d = fixture("version", &[("a.rs", ONE)]);
        let v = at(&d, json!({"name": "alpha"}));
        assert_eq!(v["extractor"], VERSION);
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
            at(&d, json!({"kind": "field", "member": "n"}))["missed"],
            json!([])
        );
    }

    const TS: &str = "export function alpha(x: number): string { return \"a\"; }\n\
                      function beta() {}\n\
                      export const gamma = (y: string) => y.length;\n";

    #[test]
    fn typescript_reads_export_off_the_ancestor_not_a_child() {
        let d = fixture("ts-vis", &[("a.ts", TS)]);
        let v = at(
            &d,
            json!({"file": "a.ts", "kind": "function", "name": "alpha"}),
        );
        assert_eq!(v["missed"], json!([]));
        assert_eq!(v["at"]["vis"], "export");

        let v = at(
            &d,
            json!({"file": "a.ts", "kind": "function", "name": "beta"}),
        );
        assert_eq!(v["at"]["vis"], "");
    }

    #[test]
    fn an_arrow_function_borrows_its_name_from_the_declarator() {
        let d = fixture("ts-arrow", &[("a.ts", TS)]);
        let v = at(
            &d,
            json!({"file": "a.ts", "kind": "function", "name": "gamma"}),
        );
        assert_eq!(v["missed"], json!([]));
        assert_eq!(v["at"]["vis"], "export");
    }

    #[test]
    fn typescript_shape_drift_reads_like_rusts() {
        let d = fixture("ts-shape", &[("a.ts", TS)]);
        let v = at(
            &d,
            json!({"file": "a.ts", "kind": "function", "name": "alpha",
                   "shape": "(x: number) : number"}),
        );
        assert_eq!(v["missed"], json!(["shape"]));
        assert_eq!(v["at"]["shape"], "(x: number) : string");
    }

    #[test]
    fn tsx_covers_plain_javascript_too() {
        let d = fixture(
            "jsx",
            &[("a.js", "export function alpha(x) { return x; }\n")],
        );
        let v = at(
            &d,
            json!({"file": "a.js", "kind": "function", "name": "alpha"}),
        );
        assert_eq!(v["missed"], json!([]));
        assert_eq!(v["at"]["vis"], "export");
    }

    #[test]
    fn python_has_no_visibility_so_vis_stays_empty() {
        let d = fixture(
            "py",
            &[(
                "a.py",
                "def alpha(x):\n    return x\n\ndef _beta():\n    pass\n",
            )],
        );
        for name in ["alpha", "_beta"] {
            let v = at(
                &d,
                json!({"file": "a.py", "kind": "function", "name": name}),
            );
            assert_eq!(v["missed"], json!([]), "{name}");
            assert_eq!(v["at"]["vis"], "", "{name}");
        }
    }

    #[test]
    fn go_derives_export_from_the_leading_letter() {
        let d = fixture(
            "go",
            &[(
                "a.go",
                "package p\nfunc Alpha(x int) int { return x }\nfunc beta() {}\n",
            )],
        );
        let v = at(
            &d,
            json!({"file": "a.go", "kind": "function", "name": "Alpha"}),
        );
        assert_eq!(v["missed"], json!([]));
        assert_eq!(v["at"]["vis"], "export");

        let v = at(
            &d,
            json!({"file": "a.go", "kind": "function", "name": "beta"}),
        );
        assert_eq!(v["at"]["vis"], "");
    }

    fn shape_of(tag: &str, src: &str, name: &str) -> String {
        let d = fixture(&format!("shape-{tag}"), &[("a.rs", src)]);
        at(&d, json!({"file": "a.rs", "name": name}))["at"]["shape"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    #[test]
    fn a_modifier_that_breaks_every_caller_is_part_of_the_signature() {
        let plain = shape_of("plain", "pub fn f(x: u64) -> u64 { x }", "f");
        for (tag, src) in [
            ("async", "pub async fn f(x: u64) -> u64 { x }"),
            ("unsafe", "pub unsafe fn f(x: u64) -> u64 { x }"),
            ("const", "pub const fn f(x: u64) -> u64 { x }"),
        ] {
            assert_ne!(shape_of(tag, src, "f"), plain, "{src}");
        }
    }

    #[test]
    fn tightening_a_bound_is_part_of_the_signature() {
        let loose = shape_of("loose", "pub fn f<T>(x: T) -> T { x }", "f");
        let tight = shape_of("tight", "pub fn f<T: Clone>(x: T) -> T { x }", "f");
        let tighter = shape_of("tighter", "pub fn f<T: Clone + Send>(x: T) -> T { x }", "f");
        assert_ne!(loose, tight);
        assert_ne!(tight, tighter);
    }

    #[test]
    fn a_type_signs_its_members_and_implements_only_their_bodies() {
        let one = shape_of("one", "pub struct X { a: u64 }", "X");
        assert!(!one.is_empty(), "a type's signature cannot be empty");
        assert_ne!(
            shape_of("two", "pub struct X { a: u64, b: String }", "X"),
            one
        );
        assert_ne!(shape_of("retyped", "pub struct X { a: u32 }", "X"), one);

        let d = fixture("members", &[("a.rs", "pub struct X { a: u64 }")]);
        let plain = at(&d, json!({"file": "a.rs", "name": "X"}));
        let d = fixture("members2", &[("a.rs", "pub struct X { a: u64, b: u8 }")]);
        let more = at(&d, json!({"file": "a.rs", "name": "X"}));
        assert_eq!(
            plain["facts"]["body"], more["facts"]["body"],
            "a struct carries no implementation, so adding a field is not a logic change"
        );
    }

    #[test]
    fn a_trait_separates_what_it_declares_from_what_it_implements() {
        let d = fixture(
            "tr1",
            &[("a.rs", "pub trait X { fn go(&self) { let a = 1; } }")],
        );
        let one = at(&d, json!({"file": "a.rs", "name": "X"}));
        let d = fixture(
            "tr2",
            &[("a.rs", "pub trait X { fn go(&self) { let b = 2; } }")],
        );
        let two = at(&d, json!({"file": "a.rs", "name": "X"}));
        assert_eq!(one["at"]["shape"], two["at"]["shape"]);
        assert_ne!(one["facts"]["body"], two["facts"]["body"]);
    }

    #[test]
    fn a_constant_is_a_coordinate_something_can_be_anchored_to() {
        let d = fixture(
            "consts",
            &[(
                "a.rs",
                "pub const WIRE: &str = \"v1\";\nstatic COUNT: u8 = 3;\npub fn f() {}\n",
            )],
        );
        let v = at(&d, json!({"file": "a.rs", "name": "WIRE"}));
        assert_eq!(
            v["missed"],
            json!([]),
            "a const the world still has must not read as absent"
        );
        assert_eq!(v["at"]["kind"], "constant");
        assert_eq!(v["at"]["vis"], "pub");
        assert_eq!(
            at(&d, json!({"file": "a.rs", "name": "COUNT"}))["at"]["kind"],
            "constant"
        );

        let moved = fixture(
            "consts-moved",
            &[("a.rs", "pub const WIRE: &str = \"v2\";\n")],
        );
        assert_ne!(
            v["facts"]["body"],
            at(&moved, json!({"file": "a.rs", "name": "WIRE"}))["facts"]["body"],
            "changing what a constant says is the whole reason to anchor one"
        );
    }

    #[test]
    fn a_constants_declared_type_is_part_of_its_shape() {
        let a = fixture("const-ty-a", &[("a.rs", "pub const X: u8 = 1;")]);
        let b = fixture("const-ty-b", &[("a.rs", "pub const X: u64 = 1;")]);
        assert_ne!(
            at(&a, json!({"file": "a.rs", "name": "X"}))["at"]["shape"],
            at(&b, json!({"file": "a.rs", "name": "X"}))["at"]["shape"]
        );
    }

    #[test]
    fn form_tells_a_struct_from_an_enum_where_kind_cannot() {
        let d = fixture("form1", &[("a.rs", "pub struct X { a: u64 }")]);
        let s = at(&d, json!({"file": "a.rs", "name": "X"}));
        let d = fixture("form2", &[("a.rs", "pub enum X { A }")]);
        let e = at(&d, json!({"file": "a.rs", "name": "X"}));
        assert_eq!(s["at"]["kind"], e["at"]["kind"]);
        assert_ne!(s["at"]["form"], e["at"]["form"]);
    }
}
