use std::collections::BTreeMap;

use gmr_core::{ContentHash, Kind, ProbeVersion, content_hash_of};
use serde::{Deserialize, Serialize};

pub const MANIFEST_SCHEMA: &str = "gmr.probe-artifact.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub sha256: ContentHash,
    #[serde(default)]
    pub executable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Platform {
    pub os: String,
    pub arch: String,
}

impl Platform {
    pub fn host() -> Self {
        Self {
            os: std::env::consts::OS.to_owned(),
            arch: std::env::consts::ARCH.to_owned(),
        }
    }
}

/// Manifest of a probe artifact: it describes and pins down the closure of one
/// derivation rule. This concept — entrypoint, argv, env, a file closure with
/// executable bits, and the OS/arch it was captured on — belongs to shell-style
/// transports specifically; an HTTP transport has no use for any of it.
/// `ProbeVersion` is its content hash — the version is earned, not declared.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub schema: String,
    pub kind: Kind,
    pub entrypoint: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    pub files: Vec<FileEntry>,
    pub platform: Platform,
    pub output_contract: String,
}

impl Manifest {
    pub fn version(&self) -> ProbeVersion {
        let value = serde_json::to_value(self).expect("a manifest always serialises");
        ProbeVersion::new(content_hash_of(&value).into_inner())
    }

    pub fn entry(&self) -> Option<&FileEntry> {
        self.files.iter().find(|f| f.path == self.entrypoint)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gmr_core::OUTCOME_CONTRACT;

    fn manifest(entry: &str, sha: &str) -> Manifest {
        Manifest {
            schema: MANIFEST_SCHEMA.to_owned(),
            kind: Kind::new("shell"),
            entrypoint: entry.to_owned(),
            args: vec!["--mode".into(), "contract".into()],
            env: BTreeMap::new(),
            files: vec![FileEntry {
                path: entry.to_owned(),
                sha256: ContentHash::new(sha.repeat(64)),
                executable: true,
            }],
            platform: Platform {
                os: "darwin".into(),
                arch: "arm64".into(),
            },
            output_contract: OUTCOME_CONTRACT.to_owned(),
        }
    }

    #[test]
    fn a_version_is_the_manifest_it_describes() {
        assert_eq!(
            manifest("bin/p", "a").version(),
            manifest("bin/p", "a").version()
        );
    }

    #[test]
    fn changing_the_bytes_changes_the_version() {
        assert_ne!(
            manifest("bin/p", "a").version(),
            manifest("bin/p", "b").version(),
            "same entrypoint, different bytes — those are two derivation rules"
        );
    }

    #[test]
    fn changing_the_args_changes_the_version() {
        let mut other = manifest("bin/p", "a");
        other.args = vec!["--mode".into(), "shape".into()];
        assert_ne!(manifest("bin/p", "a").version(), other.version());
    }

    #[test]
    fn the_platform_is_part_of_the_rule() {
        let mut other = manifest("bin/p", "a");
        other.platform.arch = "x86_64".into();
        assert_ne!(manifest("bin/p", "a").version(), other.version());
    }

    #[test]
    fn manifest_roundtrips_the_wire() {
        let m = manifest("bin/p", "a");
        let s = serde_json::to_string(&m).unwrap();
        assert_eq!(serde_json::from_str::<Manifest>(&s).unwrap(), m);
    }
}
