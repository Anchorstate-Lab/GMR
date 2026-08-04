use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use gmr_core::{ContentHash, ProbeName, ProbeVersion, content_hash_of_bytes};
use serde::{Deserialize, Serialize};

use super::manifest::{FileEntry, MANIFEST_SCHEMA, Manifest, Platform};

pub const MANIFEST_FILE: &str = "manifest.json";

pub const INSTALL_FILE: &str = "installed.json";

pub const INSTALL_SCHEMA: &str = "gmr.probe-install.v2";

/// Probe name -> the artifact installed for it here. The name travels; the
/// artifact does not. Self-describing, so no assembly has to thread a table in.
#[derive(Debug, Default, Serialize, Deserialize)]
struct InstallIndex {
    schema: String,
    installed: BTreeMap<String, String>,
}

/// Content-addressed probe store: `<root>/<version>/manifest.json` plus the
/// files named by the manifest.
pub struct Artifacts {
    root: PathBuf,
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct ArtifactError(pub String);

fn bad(m: impl Into<String>) -> ArtifactError {
    ArtifactError(m.into())
}

/// A verified artifact. Holding one means the manifest and every listed file
/// matched byte-for-byte.
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

    fn dir(&self, version: &ProbeVersion) -> PathBuf {
        self.root.join(version.as_str())
    }

    fn index(&self) -> Result<InstallIndex, ArtifactError> {
        let path = self.root.join(INSTALL_FILE);
        let Ok(bytes) = std::fs::read(&path) else {
            return Ok(InstallIndex {
                schema: INSTALL_SCHEMA.to_owned(),
                installed: BTreeMap::new(),
            });
        };
        let index: InstallIndex = serde_json::from_slice(&bytes)
            .map_err(|e| bad(format!("{path:?} is not a valid install index: {e}")))?;
        if index.schema != INSTALL_SCHEMA {
            return Err(bad(format!(
                "{path:?} declares schema `{}`, but this build only accepts `{INSTALL_SCHEMA}`",
                index.schema
            )));
        }
        Ok(index)
    }

    /// Overwrites: refusing would leave this machine on the old binary forever.
    pub fn install(&self, name: &ProbeName, built: &ProbeVersion) -> Result<(), ArtifactError> {
        let mut index = self.index()?;
        index
            .installed
            .insert(name.as_str().to_owned(), built.as_str().to_owned());
        std::fs::create_dir_all(&self.root)
            .map_err(|e| bad(format!("cannot create {:?}: {e}", self.root)))?;
        let body = serde_json::to_vec_pretty(&index).expect("install index must serialize");
        std::fs::write(self.root.join(INSTALL_FILE), body)
            .map_err(|e| bad(format!("cannot write the install index: {e}")))
    }

    /// `None` is a real answer: a good name, simply not installed here.
    pub fn installed(&self, name: &ProbeName) -> Result<Option<ProbeVersion>, ArtifactError> {
        Ok(self
            .index()?
            .installed
            .get(name.as_str())
            .map(|v| ProbeVersion::new(v.clone())))
    }

    pub fn names(&self) -> Result<Vec<ProbeName>, ArtifactError> {
        Ok(self
            .index()?
            .installed
            .into_keys()
            .map(ProbeName::new)
            .collect())
    }

    /// Read the manifest, verify its own hash, then verify every file it names.
    ///
    /// Any mismatch is refused. An artifact with mismatched content is not an
    /// old version; it is a derivation rule we cannot name.
    pub fn resolve(&self, name: &ProbeName) -> Result<Resolved, ArtifactError> {
        let version = &self.installed(name)?.ok_or_else(|| {
            bad(format!(
                "no probe named `{name}` is installed here; this machine has \
                 {}",
                match self.names() {
                    Ok(ns) if !ns.is_empty() => ns
                        .iter()
                        .map(ProbeName::as_str)
                        .collect::<Vec<_>>()
                        .join(" · "),
                    _ => "none".to_owned(),
                }
            ))
        })?;
        let dir = self.dir(version);
        let path = dir.join(MANIFEST_FILE);
        let bytes = std::fs::read(&path).map_err(|e| {
            bad(format!(
                "`{name}` is installed as {version}, but its manifest is unreadable ({path:?}): {e}"
            ))
        })?;
        let manifest: Manifest = serde_json::from_slice(&bytes)
            .map_err(|e| bad(format!("{version}'s manifest is not valid: {e}")))?;

        if manifest.schema != MANIFEST_SCHEMA {
            return Err(bad(format!(
                "{version}'s manifest declares schema `{}`, but this build only accepts `{}`",
                manifest.schema, MANIFEST_SCHEMA
            )));
        }

        let earned = manifest.version();
        if &earned != version {
            return Err(bad(format!(
                "manifest hashes to {earned}, but it is stored under {version}; name and content disagree"
            )));
        }

        if manifest.entry().is_none() {
            return Err(bad(format!(
                "{version}'s entrypoint `{}` is not listed in the file manifest",
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
        return Err(bad(format!(
            "manifest path escapes the artifact root: `{rel}`"
        )));
    }
    let path = dir.join(rel);
    let bytes = std::fs::read(&path).map_err(|e| bad(format!("cannot read {path:?}: {e}")))?;
    let got = content_hash_of_bytes(&bytes);
    if &got != want {
        return Err(bad(format!(
            "`{rel}` has content hash {got}, but the manifest says {want}; refusing to execute"
        )));
    }
    Ok(())
}

/// Publish a directory as an artifact: hash each file, write the manifest, and
/// name the artifact by the manifest hash.
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
    files.sort_by(|a: &FileEntry, b| a.path.cmp(&b.path));

    if !files.iter().any(|f| f.path == entrypoint) {
        return Err(bad(format!("`{entrypoint}` is not in {from:?}")));
    }

    let manifest = Manifest {
        schema: MANIFEST_SCHEMA.to_owned(),
        kind,
        entrypoint: entrypoint.to_owned(),
        args,
        env,
        files,
        platform: Platform::host(),
        output_contract: gmr_core::OUTCOME_CONTRACT.to_owned(),
    };
    let version = manifest.version();

    let dir = artifacts.dir(&version);
    for file in &manifest.files {
        let dst = dir.join(&file.path);
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| bad(format!("cannot create {parent:?}: {e}")))?;
        }
        std::fs::copy(from.join(&file.path), &dst)
            .map_err(|e| bad(format!("cannot copy to {dst:?}: {e}")))?;
    }
    let body = serde_json::to_vec_pretty(&manifest).expect("manifest must serialize");
    std::fs::write(dir.join(MANIFEST_FILE), body)
        .map_err(|e| bad(format!("cannot write manifest for {version}: {e}")))?;

    Ok(version)
}

fn collect(base: &Path, dir: &Path, out: &mut Vec<FileEntry>) -> Result<(), ArtifactError> {
    let entries =
        std::fs::read_dir(dir).map_err(|e| bad(format!("cannot read directory {dir:?}: {e}")))?;
    for entry in entries {
        let entry = entry.map_err(|e| bad(format!("cannot read an entry in {dir:?}: {e}")))?;
        let path = entry.path();
        if path.is_dir() {
            collect(base, &path, out)?;
            continue;
        }
        let rel = path
            .strip_prefix(base)
            .map_err(|_| bad("path escaped the publish root"))?
            .to_string_lossy()
            .replace('\\', "/");
        let bytes = std::fs::read(&path).map_err(|e| bad(format!("cannot read {path:?}: {e}")))?;
        out.push(FileEntry {
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
