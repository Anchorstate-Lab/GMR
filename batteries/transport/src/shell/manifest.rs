use std::collections::BTreeMap;

use gmr_core::{ContentHash, Kind, ProbeVersion, content_hash_of};
use serde::{Deserialize, Serialize};

pub const MANIFEST_SCHEMA: &str = "gmr.probe-artifact.v2";

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

/// What this machine has, and what rule it stands for. **Two hashes, two jobs:**
/// [`Manifest::address`] is the byte-exact identity of these files here, which
/// moves with the platform and is what verification checks;
/// [`Manifest::derivation`] is what the publisher earned from sources, is the
/// same everywhere, and is what the journal records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub schema: String,
    pub kind: Kind,
    /// The rule these bytes implement, earned from its sources by whoever
    /// published it. Two platforms' artifacts of one probe share it.
    pub derivation: ProbeVersion,
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
    /// Where it lives and what it must hash to. Local by design.
    pub fn address(&self) -> ProbeVersion {
        let value = serde_json::to_value(self).expect("a manifest always serialises");
        // Manifest's shape is fixed by its type, not by data — its nesting depth
        // is bounded regardless of how many files/args/env entries it holds.
        let hash =
            content_hash_of(&value).expect("a Manifest never exceeds canonicalization limits");
        ProbeVersion::new(hash.into_inner())
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
            derivation: ProbeVersion::new("d".repeat(64)),
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
    fn an_address_is_the_manifest_it_describes() {
        assert_eq!(
            manifest("bin/p", "a").address(),
            manifest("bin/p", "a").address()
        );
    }

    #[test]
    fn changing_the_bytes_changes_the_address() {
        assert_ne!(
            manifest("bin/p", "a").address(),
            manifest("bin/p", "b").address()
        );
    }

    /// Which is exactly why it cannot be the derivation: the same rule built for
    /// two machines would otherwise read as two rules, and no journal could be
    /// compared against another.
    #[test]
    fn the_platform_is_part_of_the_address() {
        let mut other = manifest("bin/p", "a");
        other.platform.arch = "x86_64".into();
        assert_ne!(manifest("bin/p", "a").address(), other.address());
        assert_eq!(manifest("bin/p", "a").derivation, other.derivation);
    }

    #[test]
    fn manifest_roundtrips_the_wire() {
        let m = manifest("bin/p", "a");
        let s = serde_json::to_string(&m).unwrap();
        assert_eq!(serde_json::from_str::<Manifest>(&s).unwrap(), m);
    }
}
