use std::collections::BTreeMap;
use std::path::Path;

use gmr_survey as coord;
use serde_json::{Value, json};

const VERSION: &str = env!("GMR_EXTRACTOR_PROSE");

pub(crate) const ITEMS: [&str; 3] = ["file", "heading", "fingerprint"];

fn heading(line: &str) -> Option<(usize, String)> {
    let t = line.trim_start();
    let level = t.bytes().take_while(|b| *b == b'#').count();
    if level == 0 || level > 6 {
        return None;
    }
    let rest = t[level..].trim();
    if rest.is_empty() {
        return None;
    }
    Some((level, rest.to_owned()))
}

fn squeeze(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn sections(rel: &str, src: &str, out: &mut Vec<coord::Candidate>) {
    let mut path: Vec<String> = Vec::new();
    let mut open: Option<(String, usize)> = None;
    let mut body: Vec<&str> = Vec::new();
    let mut fenced = false;

    let mut flush = |open: &mut Option<(String, usize)>, body: &mut Vec<&str>| {
        if let Some((h, line)) = open.take() {
            let text = body.join("\n");
            let c: BTreeMap<String, String> = [
                ("file", rel.to_owned()),
                ("heading", h),
                ("fingerprint", coord::hash(&squeeze(&text))),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect();
            out.push(coord::Candidate::new(
                format!("{}#{}", c["file"], c["heading"]),
                c,
                json!({ "line": line, "lines": body.len() }),
            ));
        }
        body.clear();
    };

    for (i, line) in src.lines().enumerate() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
        }
        match heading(line) {
            Some((level, title)) if !fenced => {
                flush(&mut open, &mut body);
                path.truncate(level.saturating_sub(1));
                while path.len() < level - 1 {
                    path.push(String::new());
                }
                path.push(title);
                open = Some((path.join(" > "), i + 1));
            }
            _ => body.push(line),
        }
    }
    flush(&mut open, &mut body);
}

fn collect(path: &Path, rel: &str, out: &mut Vec<coord::Candidate>) -> Result<(), String> {
    if rel.ends_with(".md")
        && let Ok(src) = std::fs::read_to_string(path)
    {
        sections(rel, &src, out);
    }
    Ok(())
}

pub fn probe(root: &Path, pos: &Value, cache: &coord::Cache) -> Result<Value, String> {
    let want = coord::wanted(pos, &ITEMS)?;
    let cands = coord::visit_cached(root, cache, "prose-map", collect)?;
    if cands.is_empty() {
        return Err(format!(
            "{} contains no Markdown sections; the probe is likely pointed at the wrong directory",
            root.display()
        ));
    }
    coord::report(VERSION, &want, coord::nth(pos), &cands)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str, files: &[(&str, &str)]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("prose-map-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        for (path, body) in files {
            let p = dir.join(path);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, body).unwrap();
        }
        dir
    }

    const DOC: &str =
        "# Top\nOpening\n\n## Red Flag\nDo not do this\n\n## Dead Concept\nDo not revive\n";

    fn at(dir: &Path, pos: Value) -> Value {
        probe(dir, &pos, &coord::Cache::disabled()).unwrap()
    }

    fn fp(dir: &Path, file: &str, heading: &str) -> String {
        at(dir, json!({"file": file, "heading": heading}))["at"]["fingerprint"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    #[test]
    fn a_heading_path_carries_its_ancestors() {
        let d = fixture("path", &[("a.md", DOC)]);
        let v = at(&d, json!({"file": "a.md", "heading": "Top > Red Flag"}));
        assert_eq!(v["missed"], json!([]));
        assert_eq!(v["at"]["heading"], "Top > Red Flag");
    }

    #[test]
    fn a_renamed_heading_is_found_by_its_unchanged_body() {
        let d = fixture("renamed", &[("a.md", DOC)]);
        let f = fp(&d, "a.md", "Top > Red Flag");
        let v = at(
            &d,
            json!({"file": "a.md", "heading": "Top > Old Name", "fingerprint": f}),
        );
        assert_eq!(v["missed"], json!(["heading"]));
        assert_eq!(v["candidates"], 1);
        assert_eq!(v["at"]["heading"], "Top > Red Flag");
    }

    #[test]
    fn a_drifted_body_reports_only_fingerprint_missed() {
        let d = fixture("drift", &[("a.md", DOC)]);
        let v = at(
            &d,
            json!({"file": "a.md", "heading": "Top > Red Flag",
                   "fingerprint": coord::hash("older text from before")}),
        );
        assert_eq!(v["missed"], json!(["fingerprint"]));
        assert_eq!(v["candidates"], 1);
    }

    #[test]
    fn a_moved_document_reports_only_file_missed() {
        let d = fixture("moved", &[("docs/b.md", DOC)]);
        let v = at(&d, json!({"file": "a.md", "heading": "Top > Red Flag"}));
        assert_eq!(v["missed"], json!(["file"]));
        assert_eq!(v["at"]["file"], "docs/b.md");
    }

    #[test]
    fn a_hash_inside_a_fence_is_not_a_heading() {
        let d = fixture(
            "fence",
            &[(
                "a.md",
                "# Real Heading\n\n```sh\n# This is a comment\n```\n",
            )],
        );
        let v = at(&d, json!({"heading": "# This is a comment"}));
        assert_eq!(v["found"], false);
        assert_eq!(
            at(&d, json!({"heading": "Real Heading"}))["missed"],
            json!([])
        );
    }

    #[test]
    fn an_empty_position_is_our_failure_not_the_worlds_answer() {
        let d = fixture("empty", &[("a.md", DOC)]);
        assert!(probe(&d, &json!({}), &coord::Cache::disabled()).is_err());
    }

    #[test]
    fn a_tree_with_no_markdown_is_our_failure_too() {
        let d = fixture("bare", &[("a.rs", "fn main() {}")]);
        assert!(probe(&d, &json!({"heading": "x"}), &coord::Cache::disabled()).is_err());
    }

    #[test]
    fn the_extractor_hashes_its_own_source() {
        let d = fixture("version", &[("a.md", DOC)]);
        let v = at(&d, json!({"heading": "Top"}));
        assert_eq!(v["extractor"], VERSION);
        assert_eq!(v["extractor"].as_str().unwrap().len(), 64);
    }
}
