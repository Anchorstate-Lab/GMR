use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::addr::{CanonicalizeError, ContentHash, content_hash_of};
use crate::string_newtype;

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

fn check_probe_name(s: &str) -> Result<(), String> {
    if s.is_empty() {
        return Err("must not be empty".to_owned());
    }
    if !s
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err("expected lowercase ASCII letters, digits or `-`".to_owned());
    }
    if crate::addr::check_sha256_hex(s).is_ok() {
        return Err("that is a version, not a name".to_owned());
    }
    Ok(())
}

string_newtype! {
    ProbeName, check_probe_name
}

string_newtype! {
    ProbeVersion, crate::addr::check_sha256_hex
}

string_newtype! {
    FactAddress, crate::addr::check_sha256_hex
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verifiability {
    #[serde(alias = "content_addressed")]
    Closed,
    #[serde(alias = "declared", alias = "unverifiable")]
    Open,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Derivation {
    pub version: ProbeVersion,
    pub verifiability: Verifiability,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeRef {
    pub kind: Kind,
    #[serde(alias = "artifact")]
    pub name: ProbeName,
    #[serde(default)]
    pub params: Value,
}

impl ProbeRef {
    pub fn new(kind: Kind, name: ProbeName, params: Value) -> Self {
        Self { kind, name, params }
    }

    pub fn declaration_hash(&self) -> Result<ContentHash, CanonicalizeError> {
        content_hash_of(&serde_json::json!({
            "kind": &self.kind,
            "name": &self.name,
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
    pub fn address(&self, derivation: &ProbeVersion) -> Result<FactAddress, CanonicalizeError> {
        let facts = match self {
            Self::Found { facts } => facts.as_value(),
            Self::NotFound => &Value::Null,
        };
        let h = content_hash_of(&serde_json::json!({
            "derivation": derivation,
            "found": matches!(self, Self::Found { .. }),
            "facts": facts,
        }))?;
        Ok(FactAddress::new(h.into_inner()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn version(sha: &str) -> ProbeVersion {
        ProbeVersion::new(sha.repeat(64))
    }

    #[test]
    fn what_the_anchor_wrote_is_not_what_derived_the_facts() {
        let a = ProbeRef::new(
            Kind::new("builtin"),
            ProbeName::new("ast-map"),
            json!({ "kind": "function" }),
        );
        let b = ProbeRef::new(
            Kind::new("builtin"),
            ProbeName::new("name-map"),
            json!({ "kind": "function" }),
        );
        assert_ne!(
            a.declaration_hash().unwrap(),
            b.declaration_hash().unwrap(),
            "naming a different probe is a different declaration"
        );
    }

    #[test]
    fn params_are_part_of_the_declaration() {
        let n = ProbeName::new("ast-map");
        let a = ProbeRef::new(
            Kind::new("builtin"),
            n.clone(),
            json!({ "kind": "function" }),
        );
        let b = ProbeRef::new(Kind::new("builtin"), n, json!({ "kind": "module" }));
        assert_ne!(a.declaration_hash().unwrap(), b.declaration_hash().unwrap());
    }

    #[test]
    fn upgrading_the_engine_leaves_the_declaration_alone() {
        let probe = ProbeRef::new(
            Kind::new("builtin"),
            ProbeName::new("ast-map"),
            json!({ "root": "." }),
        );
        let before = probe.declaration_hash().unwrap();
        let old = Outcome::Found {
            facts: Facts::new(json!({ "candidates": 3 })),
        };
        assert_ne!(
            old.address(&version("a")).unwrap(),
            old.address(&version("b")).unwrap()
        );
        assert_eq!(before, probe.declaration_hash().unwrap());
    }

    #[test]
    fn a_probe_name_is_not_a_hash() {
        assert!(ProbeName::try_new("ast-map").is_ok());
        assert!(ProbeName::try_new("deploy-sha-2").is_ok());
        assert!(ProbeName::try_new("Ast_Map").is_err());
        assert!(ProbeName::try_new("").is_err());
        assert!(ProbeName::try_new("d9".repeat(32)).is_err());
    }

    #[test]
    fn an_address_covers_the_rule_and_the_answer() {
        let v1 = version("a");
        let v2 = version("b");
        let f = Outcome::Found {
            facts: Facts::new(json!({ "x": 1 })),
        };
        let g = Outcome::Found {
            facts: Facts::new(json!({ "x": 2 })),
        };

        assert_eq!(f.address(&v1).unwrap(), f.address(&v1).unwrap());
        assert_ne!(
            f.address(&v1).unwrap(),
            f.address(&v2).unwrap(),
            "new probe = new derivation rule"
        );
        assert_ne!(
            f.address(&v1).unwrap(),
            g.address(&v1).unwrap(),
            "new content = new fact"
        );
    }

    #[test]
    fn an_absence_is_addressed_too_and_by_the_rule_that_looked() {
        let v1 = version("a");
        let v2 = version("b");
        assert_ne!(
            Outcome::NotFound.address(&v1).unwrap(),
            Outcome::NotFound.address(&v2).unwrap(),
            "still not found, but by a different rule — that is another rule's absence"
        );
    }

    #[test]
    fn an_absence_is_not_the_same_as_finding_nothing() {
        let v = version("a");
        let empty = Outcome::Found {
            facts: Facts::new(Value::Null),
        };
        assert_ne!(
            Outcome::NotFound.address(&v).unwrap(),
            empty.address(&v).unwrap()
        );
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
