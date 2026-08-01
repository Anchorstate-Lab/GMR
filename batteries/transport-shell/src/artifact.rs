use std::path::{Path, PathBuf};

use gmr_core::{ContentHash, Manifest, ProbeVersion, content_hash_of_bytes};

pub const MANIFEST_FILE: &str = "manifest.json";

/// 按内容地址存放的探针仓库：`<root>/<version>/manifest.json` 加清单点名的
/// 那些文件。
pub struct Artifacts {
    root: PathBuf,
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct ArtifactError(pub String);

fn bad(m: impl Into<String>) -> ArtifactError {
    ArtifactError(m.into())
}

/// 校验过的 artifact：拿到它就意味着清单和每一份文件的字节都对上了。
pub struct Resolved {
    pub manifest: Manifest,
    pub root: PathBuf,
}

impl Resolved {
    pub fn entrypoint(&self) -> PathBuf {
        self.root.join(&self.manifest.entrypoint)
    }
}

impl Artifacts {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn dir(&self, version: &ProbeVersion) -> PathBuf {
        self.root.join(version.as_str())
    }

    /// 读清单、校验清单自己的哈希、再逐个校验它点名的文件。
    ///
    /// 任何一步对不上都是拒绝：一个内容对不上的 artifact 不是「旧版本」，
    /// 是一条我们无法命名的派生规则。
    pub fn resolve(&self, version: &ProbeVersion) -> Result<Resolved, ArtifactError> {
        let dir = self.dir(version);
        let path = dir.join(MANIFEST_FILE);
        let bytes = std::fs::read(&path)
            .map_err(|e| bad(format!("读不到 {} 的清单（{path:?}）：{e}", version)))?;
        let manifest: Manifest = serde_json::from_slice(&bytes)
            .map_err(|e| bad(format!("{version} 的清单不是合法清单：{e}")))?;

        if manifest.schema != gmr_core::MANIFEST_SCHEMA {
            return Err(bad(format!(
                "{version} 的清单写着 schema `{}`，本代只认 `{}`",
                manifest.schema,
                gmr_core::MANIFEST_SCHEMA
            )));
        }

        let earned = manifest.version();
        if &earned != version {
            return Err(bad(format!(
                "清单算出来是 {earned}，但它躺在 {version} 底下 —— 名字和内容对不上"
            )));
        }

        if manifest.entry().is_none() {
            return Err(bad(format!(
                "{version} 的入口 `{}` 不在文件清单里",
                manifest.entrypoint
            )));
        }

        for file in &manifest.files {
            verify(&dir, &file.path, &file.sha256)?;
        }

        Ok(Resolved {
            manifest,
            root: dir,
        })
    }
}

fn verify(dir: &Path, rel: &str, want: &ContentHash) -> Result<(), ArtifactError> {
    if rel.split('/').any(|p| p == ".." || p.is_empty()) || rel.starts_with('/') {
        return Err(bad(format!("清单里的路径越界了：`{rel}`")));
    }
    let path = dir.join(rel);
    let bytes = std::fs::read(&path).map_err(|e| bad(format!("读不到 {path:?}：{e}")))?;
    let got = content_hash_of_bytes(&bytes);
    if &got != want {
        return Err(bad(format!(
            "`{rel}` 的内容是 {got}，清单说的是 {want} —— 拒绝执行"
        )));
    }
    Ok(())
}

/// 把一棵目录发布成一个 artifact：逐文件哈希、写清单、以清单哈希命名。
pub fn publish(
    artifacts: &Artifacts,
    from: &Path,
    kind: gmr_core::Kind,
    entrypoint: &str,
    args: Vec<String>,
    env: std::collections::BTreeMap<String, String>,
) -> Result<ProbeVersion, ArtifactError> {
    let mut files = Vec::new();
    collect(from, from, &mut files)?;
    files.sort_by(|a: &gmr_core::FileEntry, b| a.path.cmp(&b.path));

    if !files.iter().any(|f| f.path == entrypoint) {
        return Err(bad(format!("`{entrypoint}` 不在 {from:?} 里")));
    }

    let manifest = Manifest {
        schema: gmr_core::MANIFEST_SCHEMA.to_owned(),
        kind,
        entrypoint: entrypoint.to_owned(),
        args,
        env,
        files,
        platform: gmr_core::Platform::host(),
        output_contract: gmr_core::OUTCOME_CONTRACT.to_owned(),
    };
    let version = manifest.version();

    let dir = artifacts.dir(&version);
    for file in &manifest.files {
        let dst = dir.join(&file.path);
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent).map_err(|e| bad(format!("建不了 {parent:?}：{e}")))?;
        }
        std::fs::copy(from.join(&file.path), &dst)
            .map_err(|e| bad(format!("拷不动 {dst:?}：{e}")))?;
    }
    let body = serde_json::to_vec_pretty(&manifest).expect("清单一定可序列化");
    std::fs::write(dir.join(MANIFEST_FILE), body)
        .map_err(|e| bad(format!("写不了 {version} 的清单：{e}")))?;

    Ok(version)
}

fn collect(
    base: &Path,
    dir: &Path,
    out: &mut Vec<gmr_core::FileEntry>,
) -> Result<(), ArtifactError> {
    let entries = std::fs::read_dir(dir).map_err(|e| bad(format!("读不动 {dir:?}：{e}")))?;
    for entry in entries {
        let entry = entry.map_err(|e| bad(format!("读不动 {dir:?} 里的一项：{e}")))?;
        let path = entry.path();
        if path.is_dir() {
            collect(base, &path, out)?;
            continue;
        }
        let rel = path
            .strip_prefix(base)
            .map_err(|_| bad("路径跑出了发布根"))?
            .to_string_lossy()
            .replace('\\', "/");
        let bytes = std::fs::read(&path).map_err(|e| bad(format!("读不到 {path:?}：{e}")))?;
        out.push(gmr_core::FileEntry {
            path: rel,
            sha256: content_hash_of_bytes(&bytes),
            executable: is_executable(&path),
        });
    }
    Ok(())
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(_: &Path) -> bool {
    true
}
