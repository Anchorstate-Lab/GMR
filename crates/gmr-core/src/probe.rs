use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::addr::content_hash_of;
use crate::string_newtype;

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
#[serde(transparent)]
pub struct Declaration(pub Value);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Probe {
    pub kind: Kind,
    pub declaration: Declaration,
}

impl Probe {
    pub fn new(kind: Kind, declaration: Value) -> Self {
        Self {
            kind,
            declaration: Declaration(declaration),
        }
    }

    pub fn version(&self) -> ProbeVersion {
        let h = content_hash_of(&serde_json::json!({
            "kind": &self.kind,
            "declaration": &self.declaration,
        }));
        ProbeVersion::new(h.into_inner())
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

    pub fn address(&self, probe: &ProbeVersion) -> FactAddress {
        let h = content_hash_of(&serde_json::json!({
            "probe_version": probe,
            "facts": &self.0,
        }));
        FactAddress::new(h.into_inner())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Outcome {
    Found { facts: Facts },
    NotFound,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn probe(run: &str) -> Probe {
        Probe::new(Kind::new("shell"), json!({ "run": run }))
    }

    #[test]
    fn probe_version_is_its_content() {
        assert_eq!(probe("ls").version(), probe("ls").version());
        assert_ne!(probe("ls").version(), probe("ls -a").version());
    }

    #[test]
    fn kind_is_part_of_the_derivation_rule() {
        let shell = Probe::new(Kind::new("shell"), json!({ "run": "x" }));
        let file = Probe::new(Kind::new("file"), json!({ "run": "x" }));
        assert_ne!(shell.version(), file.version());
    }

    #[test]
    fn declaration_key_order_does_not_change_identity() {
        let a = Probe::new(Kind::new("shell"), json!({ "run": "x", "cwd": "." }));
        let b = Probe::new(Kind::new("shell"), json!({ "cwd": ".", "run": "x" }));
        assert_eq!(a.version(), b.version());
    }

    #[test]
    fn fact_address_covers_rule_and_content() {
        let v1 = probe("a").version();
        let v2 = probe("b").version();
        let f = Facts::new(json!({ "x": 1 }));
        let g = Facts::new(json!({ "x": 2 }));

        assert_eq!(f.address(&v1), f.address(&v1));
        assert_ne!(f.address(&v1), f.address(&v2), "换探针 = 换派生规则");
        assert_ne!(f.address(&v1), g.address(&v1), "换内容 = 换事实");
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
}
