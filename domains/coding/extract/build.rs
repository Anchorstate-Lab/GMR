use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const CLOSURE: [(&str, &[&str]); 4] = [
    (
        "ast",
        &[
            "tree-sitter",
            "tree-sitter-rust",
            "tree-sitter-typescript",
            "tree-sitter-python",
            "tree-sitter-go",
        ],
    ),
    ("addr", &[]),
    ("name", &[]),
    ("prose", &[]),
];

const SHARED_DIR: &str = "batteries/survey/src";

const WAIVED: [(&str, &str); 6] = [
    (
        "lib.rs",
        "module declarations and re-exports; holds no logic",
    ),
    (
        "cache.rs",
        "storage. Freshness cannot change a pure collect's answer: same bytes, same \
         candidates. What files reach collect at all is `eligible`, which lives in each \
         extractor and is hashed with it",
    ),
    (
        "narrow.rs",
        "proved output-preserving; hashing it would rebase every repository for an answer \
         identical byte for byte. See memories/survey-narrow.md",
    ),
    ("index.rs", "storage contract; no extractor reads it"),
    ("sqlite.rs", "storage backend; no extractor reads it"),
    ("testkit.rs", "storage backend; no extractor reads it"),
];

const EXTRA: [(&str, &str); 1] = [("ast", "lang.rs")];

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let root = workspace_root(&manifest);
    let locked = locked_versions(&root.join("Cargo.lock"));

    println!("cargo:rerun-if-changed=Cargo.toml");
    println!(
        "cargo:rerun-if-changed={}",
        root.join("Cargo.lock").display()
    );

    let mut shared = String::new();
    for rel in shared_files(&root) {
        let path = root.join(&rel);
        println!("cargo:rerun-if-changed={}", path.display());
        shared.push_str(&read(&path));
    }

    for (name, deps) in CLOSURE {
        let mut closure = String::new();
        closure.push_str(gmr_outcome_contract());
        closure.push_str(&shared);
        for (owner, extra) in EXTRA {
            if owner == name {
                let path = manifest.join("src").join(extra);
                println!("cargo:rerun-if-changed={}", path.display());
                closure.push_str(&read(&path));
            }
        }
        let own = manifest.join("src").join(format!("{name}.rs"));
        println!("cargo:rerun-if-changed={}", own.display());
        closure.push_str(&read(&own));
        for dep in deps {
            let version = locked
                .get(*dep)
                .unwrap_or_else(|| panic!("`{dep}` is in {name}'s closure but not in Cargo.lock"));
            closure.push_str(&format!("\n{dep} {version}"));
        }
        println!(
            "cargo:rustc-env=GMR_EXTRACTOR_{}={}",
            name.to_uppercase(),
            hex(&closure)
        );
    }
}

fn shared_files(root: &Path) -> Vec<String> {
    let dir = root.join(SHARED_DIR);
    println!("cargo:rerun-if-changed={}", dir.display());
    let listing = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot list {} for the closure: {e}", dir.display()));

    let mut present: Vec<String> = listing
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".rs"))
        .collect();
    present.sort();

    for (waived, why) in WAIVED {
        assert!(
            present.iter().any(|name| name == waived),
            "{SHARED_DIR}/{waived} is waived out of the closure ({why}) but no longer exists. \
             A waiver for a file nobody can find is a hole nobody can see"
        );
    }

    present
        .into_iter()
        .filter(|name| !WAIVED.iter().any(|(waived, _)| waived == name))
        .map(|name| format!("{SHARED_DIR}/{name}"))
        .collect()
}

fn gmr_outcome_contract() -> &'static str {
    "gmr.outcome.v1"
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("cannot read {} for the closure: {e}", path.display()))
}

fn workspace_root(from: &Path) -> PathBuf {
    from.ancestors()
        .find(|d| d.join("Cargo.lock").is_file())
        .unwrap_or_else(|| panic!("no Cargo.lock above {}", from.display()))
        .to_path_buf()
}

fn locked_versions(lock: &Path) -> BTreeMap<String, String> {
    let text = read(lock);
    let mut out = BTreeMap::new();
    let mut name = None;
    for line in text.lines() {
        let line = line.trim();
        if line == "[[package]]" {
            name = None;
        } else if let Some(v) = field(line, "name") {
            name = Some(v);
        } else if let Some(v) = field(line, "version")
            && let Some(n) = name.take()
        {
            out.insert(n, v);
        }
    }
    out
}

fn field(line: &str, key: &str) -> Option<String> {
    let rest = line
        .strip_prefix(key)?
        .trim_start()
        .strip_prefix('=')?
        .trim();
    Some(rest.trim_matches('"').to_owned())
}

fn hex(s: &str) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(s.as_bytes()))
}
