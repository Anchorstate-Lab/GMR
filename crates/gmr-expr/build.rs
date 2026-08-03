use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};

/// The evaluator version has to be earned, same as a probe: a hand-written
/// version string can lie — change the comparison semantics, forget the constant,
/// and the log claims it was the same evaluator.
///
/// Own source is not the whole of that. What `Value`s compare equal is decided
/// by serde_json, so the resolved versions of the runtime dependency closure
/// are hashed too. Build-dependencies are excluded: they cannot reach a
/// comparison.
fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files: Vec<PathBuf> = std::fs::read_dir(root.join("src"))
        .expect("the evaluator cannot read its own source")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "rs"))
        .filter(|p| p.file_name().is_some_and(|n| n != "version.rs"))
        .collect();
    let manifest = root.join("Cargo.toml");
    files.push(manifest.clone());
    files.sort();

    let mut hasher = <sha2::Sha256 as sha2::Digest>::new();
    for path in &files {
        println!("cargo::rerun-if-changed={}", path.display());
        let bytes = std::fs::read(path).expect("the evaluator cannot read its own source");
        sha2::Digest::update(&mut hasher, path.file_name().unwrap().as_encoded_bytes());
        sha2::Digest::update(&mut hasher, b"\0");
        sha2::Digest::update(&mut hasher, &bytes);
    }

    for (name, version) in closure(&manifest, &lockfile(root)) {
        sha2::Digest::update(&mut hasher, name.as_bytes());
        sha2::Digest::update(&mut hasher, b"\0");
        sha2::Digest::update(&mut hasher, version.as_bytes());
        sha2::Digest::update(&mut hasher, b"\0");
    }
    let digest = format!("{:x}", sha2::Digest::finalize(hasher));

    let out = Path::new(&std::env::var("OUT_DIR").unwrap()).join("version.rs");
    let mut f = std::fs::File::create(out).unwrap();
    writeln!(f, "pub const EVALUATOR_VERSION: &str = \"{digest}\";").unwrap();
}

/// Refuses rather than degrading: a version that quietly stopped covering the
/// dependencies claims a guarantee it is no longer making.
fn lockfile(root: &Path) -> PathBuf {
    let mut dir = Some(root);
    while let Some(d) = dir {
        let lock = d.join("Cargo.lock");
        if lock.is_file() {
            println!("cargo::rerun-if-changed={}", lock.display());
            return lock;
        }
        dir = d.parent();
    }
    panic!("no Cargo.lock above {root:?}: the evaluator cannot earn a version it cannot compute");
}

/// The runtime dependency closure, name and resolved version.
fn closure(manifest: &Path, lock: &Path) -> BTreeSet<(String, String)> {
    let manifest: toml::Value =
        toml::from_str(&std::fs::read_to_string(manifest).expect("unreadable manifest"))
            .expect("unparseable manifest");
    let roots = manifest
        .get("dependencies")
        .and_then(toml::Value::as_table)
        .map(|t| t.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();

    let lock: toml::Value =
        toml::from_str(&std::fs::read_to_string(lock).expect("unreadable Cargo.lock"))
            .expect("unparseable Cargo.lock");
    let packages: BTreeMap<String, (String, Vec<String>)> = lock
        .get("package")
        .and_then(toml::Value::as_array)
        .map(|ps| {
            ps.iter()
                .filter_map(|p| {
                    let name = p.get("name")?.as_str()?.to_owned();
                    let version = p.get("version")?.as_str()?.to_owned();
                    let deps = p
                        .get("dependencies")
                        .and_then(toml::Value::as_array)
                        .map(|ds| {
                            ds.iter()
                                .filter_map(|d| Some(d.as_str()?.split(' ').next()?.to_owned()))
                                .collect()
                        })
                        .unwrap_or_default();
                    Some((name, (version, deps)))
                })
                .collect()
        })
        .unwrap_or_default();

    let mut out = BTreeSet::new();
    let mut queue = roots;
    while let Some(name) = queue.pop() {
        let Some((version, deps)) = packages.get(&name) else {
            continue;
        };
        if out.insert((name, version.clone())) {
            queue.extend(deps.iter().cloned());
        }
    }
    out
}
