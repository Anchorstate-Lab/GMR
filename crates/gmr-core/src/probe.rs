use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::addr::{ContentHash, content_hash_of};
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
    // Nobody names a probe with 64 hex digits; someone pasted a version here.
    if crate::addr::check_sha256_hex(s).is_ok() {
        return Err("that is a version, not a name".to_owned());
    }
    Ok(())
}

string_newtype! {
    /// A name, not a hash: it must survive an engine upgrade unchanged.
    ProbeName, check_probe_name
}

string_newtype! {
    ProbeVersion, crate::addr::check_sha256_hex
}

string_newtype! {
    FactAddress, crate::addr::check_sha256_hex
}

/// Whether the derivation rule's identity can be proven. Being unable to prove
/// it is not a failure — it is the sentence that has to be said out loud.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verifiability {
    /// The transport verified the declaration's content closure byte for byte.
    ContentAddressed,
    /// Only a declaration to go on; what actually ran cannot be proven.
    Declared,
    /// Not even the declaration pins the result down.
    Unverifiable,
}

/// What actually derived this observation, handed over by the transport.
///
/// `version` hashes every input that can change the output — sources, the
/// versions of what they parse with, the output contract. Not a binary's bytes:
/// those also move with platform and compiler, and a version that moves without
/// the behaviour moving is noise nothing can filter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Derivation {
    pub version: ProbeVersion,
    pub verifiability: Verifiability,
}

/// What the anchor wrote down. This is *not* [`Derivation`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeRef {
    pub kind: Kind,
    pub name: ProbeName,
    #[serde(default)]
    pub params: Value,
}

impl ProbeRef {
    pub fn new(kind: Kind, name: ProbeName, params: Value) -> Self {
        Self { kind, name, params }
    }

    pub fn declaration_hash(&self) -> ContentHash {
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

    // What earns a version is a transport's concern; these only need two
    // distinct opaque ones.
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
            a.declaration_hash(),
            b.declaration_hash(),
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
        assert_ne!(a.declaration_hash(), b.declaration_hash());
    }

    #[test]
    fn upgrading_the_engine_leaves_the_declaration_alone() {
        let probe = ProbeRef::new(
            Kind::new("builtin"),
            ProbeName::new("ast-map"),
            json!({ "root": "." }),
        );
        let before = probe.declaration_hash();
        // A release changes the extractor: the derivation moves, nothing else.
        let old = Outcome::Found {
            facts: Facts::new(json!({ "candidates": 3 })),
        };
        assert_ne!(old.address(&version("a")), old.address(&version("b")));
        assert_eq!(before, probe.declaration_hash());
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

        assert_eq!(f.address(&v1), f.address(&v1));
        assert_ne!(
            f.address(&v1),
            f.address(&v2),
            "new probe = new derivation rule"
        );
        assert_ne!(f.address(&v1), g.address(&v1), "new content = new fact");
    }

    #[test]
    fn an_absence_is_addressed_too_and_by_the_rule_that_looked() {
        let v1 = version("a");
        let v2 = version("b");
        assert_ne!(
            Outcome::NotFound.address(&v1),
            Outcome::NotFound.address(&v2),
            "still not found, but by a different rule — that is another rule's absence"
        );
    }

    #[test]
    fn an_absence_is_not_the_same_as_finding_nothing() {
        let v = version("a");
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
}
