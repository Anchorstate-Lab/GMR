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

/// 探针 artifact 的清单：描述并锁死一次派生规则的闭包。
/// `ProbeVersion` 就是它的内容哈希 —— 版本是挣来的，不是声明的。
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
        let value = serde_json::to_value(self).expect("清单一定可序列化");
        ProbeVersion::new(content_hash_of(&value).into_inner())
    }

    pub fn entry(&self) -> Option<&FileEntry> {
        self.files.iter().find(|f| f.path == self.entrypoint)
    }
}

/// 派生规则的身份能不能被证明。证不出来不是失败，是必须说出来的那句话。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verifiability {
    /// 清单与每一份文件的内容都当场校验过。
    ContentAddressed,
    /// 只有一段声明可依；执行的东西证不出来。
    Declared,
    /// 连声明都锁不住结果。
    Unverifiable,
}

/// 这一次观测实际被什么算出来。由传输在执行时给出，不是锚自己算的。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Derivation {
    pub version: ProbeVersion,
    pub verifiability: Verifiability,
}

/// 锚上写的东西：指向哪个 artifact，带什么参数。它不是派生规则的身份。
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
    /// 「世界说没有」也是答案，也要有地址：否则两次 NotFound 之间换了派生
    /// 规则，比对会把它们当成同一件事。
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
            "同一个入口、不同的内容 —— 那是两条派生规则"
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
            "换 artifact 就是换声明"
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
        assert_ne!(f.address(&v1), f.address(&v2), "换探针 = 换派生规则");
        assert_ne!(f.address(&v1), g.address(&v1), "换内容 = 换事实");
    }

    #[test]
    fn an_absence_is_addressed_too_and_by_the_rule_that_looked() {
        let v1 = manifest("bin/p", "a").version();
        let v2 = manifest("bin/p", "b").version();
        assert_ne!(
            Outcome::NotFound.address(&v1),
            Outcome::NotFound.address(&v2),
            "换了探针还是没找到 —— 那是另一条规则说的没有"
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
