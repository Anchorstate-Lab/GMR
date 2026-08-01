use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::addr::{ContentHash, content_hash_of};
use crate::string_newtype;

pub const MANIFEST_SCHEMA: &str = "gmr.probe-artifact.v1";

pub const OUTCOME_CONTRACT: &str = "gmr.outcome.v1";

fn check_kind(s: &str) -> Result<(), String> {
    if s.is_empty() {
        return Err("must not be empty".to_owned());
    }
    if !s.bytes().all(|b| b.is_ascii_lowercase() || b == b'-') {
        return Err("expected lowercase ASCII letters or `-`".to_owned());
    }
    Ok(())
}

string_newtype! {
    Kind, check_kind
}

string_newtype! {
    ProbeVersion, crate::addr::check_sha256_hex
}

string_newtype! {
    FactAddress, crate::addr::check_sha256_hex
}

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
/// derivation rule.
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

/// Whether the derivation rule's identity can be proven. Being unable to prove
/// it is not a failure — it is the sentence that has to be said out loud.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verifiability {
    /// Manifest and every file it names were verified byte for byte.
    ContentAddressed,
    /// Only a declaration to go on; what actually ran cannot be proven.
    Declared,
    /// Not even the declaration pins the result down.
    Unverifiable,
}

/// What actually derived this observation. Handed over by the transport at call
/// time — not computed by the anchor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Derivation {
    pub version: ProbeVersion,
    pub verifiability: Verifiability,
}

/// What the anchor wrote down: which artifact, with which params. This is *not*
/// the identity of the derivation rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeRef {
    pub kind: Kind,
    pub artifact: ProbeVersion,
    #[serde(default)]
    pub params: Value,
}

impl ProbeRef {
    pub fn new(kind: Kind, artifact: ProbeVersion, params: Value) -> Self {
        Self {
            kind,
            artifact,
            params,
        }
    }

    pub fn declaration_hash(&self) -> ContentHash {
        content_hash_of(&serde_json::json!({
            "kind": &self.kind,
            "artifact": &self.artifact,
            "params": &self.params,
        }))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Facts(pub Value);

impl Facts {
    pub fn new(value: Value) -> Self {
        Self(value)
    }

    pub fn as_value(&self) -> &Value {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Outcome {
    Found { facts: Facts },
    NotFound,
}

impl Outcome {
    /// "The world says there is nothing" is an answer too, so it gets an address:
    /// otherwise swapping the derivation rule between two NotFounds compares equal.
    pub fn address(&self, derivation: &ProbeVersion) -> FactAddress {
        let facts = match self {
            Self::Found { facts } => facts.as_value(),
            Self::NotFound => &Value::Null,
        };
        let h = content_hash_of(&serde_json::json!({
            "derivation": derivation,
            "found": matches!(self, Self::Found { .. }),
            "facts": facts,
        }));
        FactAddress::new(h.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
    fn what_the_anchor_wrote_is_not_what_derived_the_facts() {
        let a = ProbeRef::new(
            Kind::new("shell"),
            manifest("bin/p", "a").version(),
            json!({ "kind": "function" }),
        );
        let b = ProbeRef::new(
            Kind::new("shell"),
            manifest("bin/p", "b").version(),
            json!({ "kind": "function" }),
        );
        assert_ne!(
            a.declaration_hash(),
            b.declaration_hash(),
            "swapping the artifact is swapping the declaration"
        );
    }

    #[test]
    fn params_are_part_of_the_declaration() {
        let v = manifest("bin/p", "a").version();
        let a = ProbeRef::new(Kind::new("shell"), v.clone(), json!({ "kind": "function" }));
        let b = ProbeRef::new(Kind::new("shell"), v, json!({ "kind": "module" }));
        assert_ne!(a.declaration_hash(), b.declaration_hash());
    }

    #[test]
    fn an_address_covers_the_rule_and_the_answer() {
        let v1 = manifest("bin/p", "a").version();
        let v2 = manifest("bin/p", "b").version();
        let f = Outcome::Found {
            facts: Facts::new(json!({ "x": 1 })),
        };
        let g = Outcome::Found {
            facts: Facts::new(json!({ "x": 2 })),
        };

        assert_eq!(f.address(&v1), f.address(&v1));
        assert_ne!(f.address(&v1), f.address(&v2), "new probe = new derivation rule");
        assert_ne!(f.address(&v1), g.address(&v1), "new content = new fact");
    }

    #[test]
    fn an_absence_is_addressed_too_and_by_the_rule_that_looked() {
        let v1 = manifest("bin/p", "a").version();
        let v2 = manifest("bin/p", "b").version();
        assert_ne!(
            Outcome::NotFound.address(&v1),
            Outcome::NotFound.address(&v2),
            "still not found, but by a different rule — that is another rule's absence"
        );
    }

    #[test]
    fn an_absence_is_not_the_same_as_finding_nothing() {
        let v = manifest("bin/p", "a").version();
        let empty = Outcome::Found {
            facts: Facts::new(Value::Null),
        };
        assert_ne!(Outcome::NotFound.address(&v), empty.address(&v));
    }

    #[test]
    fn outcome_roundtrips_the_wire() {
        for o in [
            Outcome::Found {
                facts: Facts::new(json!([1, 2])),
            },
            Outcome::NotFound,
        ] {
            let s = serde_json::to_string(&o).unwrap();
            assert_eq!(serde_json::from_str::<Outcome>(&s).unwrap(), o);
        }
    }

    #[test]
    fn manifest_roundtrips_the_wire() {
        let m = manifest("bin/p", "a");
        let s = serde_json::to_string(&m).unwrap();
        assert_eq!(serde_json::from_str::<Manifest>(&s).unwrap(), m);
    }
}
