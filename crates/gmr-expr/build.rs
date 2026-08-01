use std::io::Write;

/// 求值器的版本必须挣来，跟探针一样：手写的版本串可以撒谎，改了比较语义
/// 忘了改常量，日志就会声称是同一个求值器。
fn main() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(root.join("src"))
        .expect("求值器读不到自己的源码")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "rs"))
        .filter(|p| p.file_name().is_some_and(|n| n != "version.rs"))
        .collect();
    files.push(root.join("Cargo.toml"));
    files.sort();

    let mut hasher = <sha2::Sha256 as sha2::Digest>::new();
    for path in &files {
        println!("cargo::rerun-if-changed={}", path.display());
        let bytes = std::fs::read(path).expect("求值器读不到自己的源码");
        sha2::Digest::update(&mut hasher, path.file_name().unwrap().as_encoded_bytes());
        sha2::Digest::update(&mut hasher, b"\0");
        sha2::Digest::update(&mut hasher, &bytes);
    }
    let digest = format!("{:x}", sha2::Digest::finalize(hasher));

    let out = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("version.rs");
    let mut f = std::fs::File::create(out).unwrap();
    writeln!(f, "pub const EVALUATOR_VERSION: &str = \"{digest}\";").unwrap();
}
