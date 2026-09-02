use crate::lang;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Adoptable {
    pub file: String,
    pub line: usize,
    pub symbol: Option<String>,
    pub text: String,
}

const TASK_MARKS: &[&str] = &["todo", "fixme", "xxx", "hack"];

const DIRECTIVE_MARKS: &[&str] = &[
    "noqa",
    "type:",
    "pylint",
    "mypy:",
    "ruff:",
    "eslint",
    "prettier",
    "@ts-",
    "biome-",
    "rustfmt::",
    "clippy::",
    "safety:",
    "coverage:",
    "nolint",
    "go:generate",
    "go:build",
    "!",
];

fn is_comment(kind: &str) -> bool {
    kind == "comment" || kind == "line_comment" || kind == "block_comment"
}

fn stripped(raw: &str) -> String {
    raw.lines()
        .map(|l| {
            l.trim()
                .trim_start_matches("///")
                .trim_start_matches("//!")
                .trim_start_matches("//")
                .trim_start_matches("/*")
                .trim_end_matches("*/")
                .trim_start_matches('*')
                .trim_start_matches('#')
                .trim()
        })
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_noise(text: &str, first_block: bool) -> bool {
    let lower = text.to_lowercase();
    if lower.contains("spdx-license") || (first_block && lower.starts_with("copyright")) {
        return true;
    }
    if TASK_MARKS
        .iter()
        .any(|m| lower.starts_with(m) || lower.starts_with(&format!("{m}:")))
    {
        return true;
    }
    DIRECTIVE_MARKS.iter().any(|m| lower.starts_with(m))
}

const NOMINATABLE: &[&str] = &["function", "type", "module", "constant", "field"];

fn name_of(table: &lang::Table, node: tree_sitter::Node, src: &str) -> Option<String> {
    let kind = lang::normalize(table, node.kind())?;
    if !NOMINATABLE.contains(&kind) {
        return None;
    }
    let text = |n: tree_sitter::Node| src.get(n.byte_range()).map(str::to_owned);
    table
        .names
        .iter()
        .find_map(|f| node.child_by_field_name(f).and_then(text))
        .filter(|n| !n.is_empty())
}

fn enclosing(table: &lang::Table, node: tree_sitter::Node, src: &str) -> Option<String> {
    let mut at = node.parent();
    while let Some(n) = at {
        if let Some(name) = name_of(table, n, src) {
            return Some(name);
        }
        at = n.parent();
    }
    None
}

fn following(table: &lang::Table, node: tree_sitter::Node, src: &str) -> Option<String> {
    let mut at = node.next_named_sibling();
    while let Some(n) = at {
        if is_comment(n.kind()) {
            at = n.next_named_sibling();
            continue;
        }
        return name_of(table, n, src).or_else(|| {
            let mut cursor = n.walk();
            n.named_children(&mut cursor)
                .find_map(|c| name_of(table, c, src))
        });
    }
    None
}

pub fn adoptable(rel: &str, src: &str) -> Vec<Adoptable> {
    let Some(table) = lang::for_path(rel) else {
        return Vec::new();
    };
    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(&(table.language)()).is_err() {
        return Vec::new();
    }
    let Some(tree) = parser.parse(src, None) else {
        return Vec::new();
    };

    let mut cursor = tree.walk();
    let mut stack = vec![tree.root_node()];
    let mut found: Vec<tree_sitter::Node> = Vec::new();
    while let Some(node) = stack.pop() {
        for c in node.named_children(&mut cursor) {
            stack.push(c);
        }
        if is_comment(node.kind()) {
            found.push(node);
        }
    }
    found.sort_by_key(|n| n.start_byte());

    let mut blocks: Vec<(tree_sitter::Node, usize, usize, String)> = Vec::new();
    for node in found {
        let text = src.get(node.byte_range()).unwrap_or_default();
        let start = node.start_position().row + 1;
        let end = node.end_position().row + 1;
        match blocks.last_mut() {
            Some((held, _, held_end, merged))
                if *held_end + 1 == start && held.parent() == node.parent() =>
            {
                *held = node;
                *held_end = end;
                merged.push('\n');
                merged.push_str(text);
            }
            _ => blocks.push((node, start, end, text.to_owned())),
        }
    }

    let mut out = Vec::new();
    for (i, (node, start, _, raw)) in blocks.iter().enumerate() {
        let text = stripped(raw);
        if text.is_empty() || is_noise(&text, i == 0 && *start == 1) {
            continue;
        }
        let symbol = following(table, *node, src).or_else(|| enclosing(table, *node, src));
        out.push(Adoptable {
            file: rel.to_owned(),
            line: *start,
            symbol,
            text,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn of(src: &str) -> Vec<Adoptable> {
        adoptable("x.rs", src)
    }

    #[test]
    fn a_comment_attaches_to_the_symbol_it_precedes() {
        let found = of("// sessions expire after thirty minutes\nfn create_session() {}\n");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].symbol.as_deref(), Some("create_session"));
        assert_eq!(found[0].text, "sessions expire after thirty minutes");
        assert_eq!(found[0].line, 1);
    }

    #[test]
    fn consecutive_lines_are_one_block() {
        let found =
            of("// the retry cap is three\n// because the gateway rate limits\nfn retry() {}\n");
        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0].text,
            "the retry cap is three because the gateway rate limits"
        );
    }

    #[test]
    fn a_body_comment_attaches_to_the_enclosing_symbol() {
        let found =
            of("fn f() {\n    // the order here is load-bearing for the fold\n    g();\n}\n");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].symbol.as_deref(), Some("f"));
    }

    #[test]
    fn a_leading_file_block_has_no_symbol() {
        let found = of("// this module owns the retry policy end to end\n\nuse std::fmt;\n");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].symbol, None);
    }

    #[test]
    fn license_tasks_and_directives_are_not_constraints() {
        let found = of(
            "// SPDX-License-Identifier: MIT\nfn a() {}\n\n// TODO: rewrite this\nfn b() {}\n\nfn c() {\n    // clippy::needless_borrow\n    d();\n}\n",
        );
        assert!(
            found.is_empty(),
            "a license is provenance, a task is not yet true, a directive speaks to a tool: {found:?}"
        );
    }

    #[test]
    fn python_comments_attach_the_same_way() {
        let found = adoptable(
            "x.py",
            "# the cache key must include the tenant\ndef cache_key():\n    pass\n",
        );
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].symbol.as_deref(), Some("cache_key"));
    }

    #[test]
    fn an_unknown_extension_nominates_nothing() {
        assert!(adoptable("x.zig", "// hello\n").is_empty());
    }
}
