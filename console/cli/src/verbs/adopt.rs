use std::path::{Path, PathBuf};

use crate::error::CliError;

#[derive(Debug, serde::Serialize)]
pub struct Candidate {
    pub source: &'static str,
    pub coordinate: String,
    pub at: String,
    pub text: String,
}

const SKIPPED_DIRS: &[&str] = &[
    ".git",
    ".anchor",
    "target",
    "node_modules",
    "memories",
    "dist",
    "vendor",
];

fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), CliError> {
    let listing = std::fs::read_dir(dir)
        .map_err(|e| CliError(format!("cannot list {}: {e}", dir.display())))?;
    for entry in listing.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if !SKIPPED_DIRS.contains(&name.as_str()) && !name.starts_with('.') {
                walk(&path, out)?;
            }
            continue;
        }
        if name.contains(".min.") {
            continue;
        }
        out.push(path);
    }
    Ok(())
}

fn rel_of(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn from_comments(root: &Path, rel: &str, src: &str, min_words: usize) -> Vec<Candidate> {
    let _ = root;
    gmr_coding_pack::comments::adoptable(rel, src)
        .into_iter()
        .filter(|a| a.text.split_whitespace().count() >= min_words)
        .map(|a| Candidate {
            source: "comment",
            coordinate: match &a.symbol {
                Some(name) => format!("{}#{name}", a.file),
                None => a.file.clone(),
            },
            at: format!("{}:{}", a.file, a.line),
            text: a.text,
        })
        .collect()
}

fn doc_tokens(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(open) = rest.find('`') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('`') else { break };
        let token = &after[..close];
        if !token.is_empty()
            && !token.contains(char::is_whitespace)
            && token.contains('/')
            && token.contains('.')
        {
            out.push(token.to_owned());
        }
        rest = &after[close + 1..];
    }
    out
}

fn from_doc(root: &Path, rel: &str, src: &str) -> Vec<Candidate> {
    let mut out = Vec::new();
    let mut fenced = false;
    let mut heading = String::new();
    for (n, line) in src.lines().enumerate() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        if let Some(h) = line.strip_prefix('#') {
            heading = h.trim_start_matches('#').trim().to_owned();
            continue;
        }
        for token in doc_tokens(line) {
            let path_part = token.split('#').next().unwrap_or(&token);
            if !root.join(path_part).is_file() {
                continue;
            }
            out.push(Candidate {
                source: "doc",
                coordinate: token,
                at: match heading.is_empty() {
                    true => format!("{rel}:{}", n + 1),
                    false => format!("{rel} § {heading}"),
                },
                text: line.trim().to_owned(),
            });
        }
    }
    out
}

fn shell_quoted(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

pub fn run(root: &Path, paths: Vec<String>, min_words: usize, json: bool) -> Result<i32, CliError> {
    let mut files = Vec::new();
    let starts = match paths.is_empty() {
        true => vec![root.to_path_buf()],
        false => paths.iter().map(|p| root.join(p)).collect(),
    };
    for start in starts {
        match start.is_dir() {
            true => walk(&start, &mut files)?,
            false => files.push(start),
        }
    }
    files.sort();

    let mut candidates = Vec::new();
    for path in &files {
        let rel = rel_of(root, path);
        let Ok(src) = std::fs::read_to_string(path) else {
            continue;
        };
        if rel.ends_with(".md") {
            candidates.extend(from_doc(root, &rel, &src));
        } else {
            candidates.extend(from_comments(root, &rel, &src, min_words));
        }
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&candidates)?);
        return Ok(0);
    }

    if candidates.is_empty() {
        println!(
            "nothing to nominate: no comment reads like a constraint and no document \
             names a file that exists. A corpus starts with what you write next — \
             `gmr anchor <coordinate> -m '...'`"
        );
        return Ok(0);
    }

    println!(
        "{} candidate(s). Each line below is a judgment, not a batch: run the ones \
         that state a real constraint, delete the rest.\n",
        candidates.len()
    );
    let single_target = !paths.is_empty();
    let mut shown: std::collections::BTreeMap<&str, usize> = Default::default();
    for c in &candidates {
        let file = c.at.split([':', ' ']).next().unwrap_or(&c.at);
        let seen = shown.entry(file).or_default();
        *seen += 1;
        if !single_target && *seen == PER_FILE + 1 {
            let left = candidates
                .iter()
                .filter(|o| o.at.split([':', ' ']).next() == Some(file))
                .count()
                - PER_FILE;
            println!("# … {left} more in {file} — `gmr adopt {file}` lists them all\n");
            continue;
        }
        if !single_target && *seen > PER_FILE {
            continue;
        }
        println!("# {} — {}", c.at, c.source);
        match c.source {
            "comment" => println!(
                "gmr anchor {} -m {}\n",
                shell_quoted(&c.coordinate),
                shell_quoted(&c.text)
            ),
            _ => println!(
                "gmr anchor {} -m '<state the constraint this sentence claims: {}>'\n",
                shell_quoted(&c.coordinate),
                c.text
                    .replace('\'', "’")
                    .chars()
                    .take(120)
                    .collect::<String>()
            ),
        }
    }
    Ok(0)
}

const PER_FILE: usize = 5;

#[cfg(test)]
mod tests {
    use super::*;

    fn world() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(
            dir.path().join("src/auth.rs"),
            "// sessions expire after thirty minutes because the gateway drops idle streams\nfn create_session() {}\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("README.md"),
            "# Storage\n\nEverything durable goes through `src/auth.rs` and nothing else.\n\n```\n`src/fake.rs` inside a fence is not a claim\n```\n",
        )
        .unwrap();
        dir
    }

    fn candidates(root: &Path, min_words: usize) -> Vec<Candidate> {
        let mut files = Vec::new();
        walk(root, &mut files).unwrap();
        files.sort();
        let mut out = Vec::new();
        for path in &files {
            let rel = rel_of(root, path);
            let src = std::fs::read_to_string(path).unwrap();
            if rel.ends_with(".md") {
                out.extend(from_doc(root, &rel, &src));
            } else {
                out.extend(from_comments(root, &rel, &src, min_words));
            }
        }
        out
    }

    #[test]
    fn a_comment_and_a_doc_claim_each_become_one_candidate() {
        let dir = world();
        let found = candidates(dir.path(), 4);
        assert_eq!(found.len(), 2, "{found:?}");

        let comment = found.iter().find(|c| c.source == "comment").unwrap();
        assert_eq!(comment.coordinate, "src/auth.rs#create_session");
        assert_eq!(comment.at, "src/auth.rs:1");

        let doc = found.iter().find(|c| c.source == "doc").unwrap();
        assert_eq!(doc.coordinate, "src/auth.rs");
        assert_eq!(doc.at, "README.md § Storage");
    }

    #[test]
    fn a_fenced_mention_and_a_missing_path_nominate_nothing() {
        let dir = world();
        let found = candidates(dir.path(), 4);
        assert!(
            !found.iter().any(|c| c.coordinate.contains("fake")),
            "a code fence quotes, it does not claim"
        );
    }

    #[test]
    fn min_words_drops_what_cannot_be_a_constraint() {
        let dir = world();
        std::fs::write(dir.path().join("src/tiny.rs"), "// yes\nfn t() {}\n").unwrap();
        let found = candidates(dir.path(), 4);
        assert!(!found.iter().any(|c| c.at.starts_with("src/tiny.rs")));
    }
}
