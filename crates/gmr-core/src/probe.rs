use std::collections::BTreeSet;

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
    admitted Kind, check_kind
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
    admitted ProbeName, check_probe_name
}

string_newtype! {
    minted ProbeVersion, crate::addr::check_sha256_hex
}

impl ProbeVersion {
    pub fn of(hash: ContentHash) -> Self {
        Self::new(hash.into_inner())
    }
}

string_newtype! {
    minted FactAddress, crate::addr::check_sha256_hex
}

impl FactAddress {
    pub fn of(hash: ContentHash) -> Self {
        Self::new(hash.into_inner())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Openness {
    HostEnv,
    Interpreter,
    Network,
    Clock,
    Implementation,
    Unknown,
}

impl Openness {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::HostEnv => "the host environment",
            Self::Interpreter => "the interpreter that runs it",
            Self::Network => "a remote system",
            Self::Clock => "when it was asked",
            Self::Implementation => "an implementation living somewhere else",
            Self::Unknown => "something nobody recorded",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verifiability {
    Closed,
    Open { over: BTreeSet<Openness> },
}

impl Verifiability {
    pub fn open(over: impl IntoIterator<Item = Openness>) -> Self {
        let over: BTreeSet<Openness> = over.into_iter().collect();
        match over.is_empty() {
            true => Self::Open {
                over: BTreeSet::from([Openness::Unknown]),
            },
            false => Self::Open { over },
        }
    }

    pub fn is_closed(&self) -> bool {
        matches!(self, Self::Closed)
    }

    pub fn over(&self) -> &BTreeSet<Openness> {
        static NONE: std::sync::LazyLock<BTreeSet<Openness>> =
            std::sync::LazyLock::new(BTreeSet::new);
        match self {
            Self::Closed => &NONE,
            Self::Open { over } => over,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum BareVerifiability {
    #[serde(alias = "content_addressed")]
    Closed,
    #[serde(alias = "declared", alias = "unverifiable")]
    Open,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum TaggedVerifiability {
    Closed,
    Open { over: BTreeSet<Openness> },
}

#[derive(Deserialize)]
#[serde(untagged)]
enum WireVerifiability {
    Bare(BareVerifiability),
    Tagged(TaggedVerifiability),
}

impl<'de> Deserialize<'de> for Verifiability {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(match WireVerifiability::deserialize(d)? {
            WireVerifiability::Bare(BareVerifiability::Closed)
            | WireVerifiability::Tagged(TaggedVerifiability::Closed) => Self::Closed,
            WireVerifiability::Bare(BareVerifiability::Open) => Self::Open {
                over: BTreeSet::from([Openness::Unknown]),
            },
            WireVerifiability::Tagged(TaggedVerifiability::Open { over }) => Self::open(over),
        })
    }
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
pub struct Facts(Value);

impl Facts {
    pub fn new(value: Value) -> Self {
        Self(value)
    }

    pub fn as_value(&self) -> &Value {
        &self.0
    }

    pub fn digested(&self) -> bool {
        digested(&self.0)
    }
}

fn digested(value: &Value) -> bool {
    match value {
        Value::Object(fields) => fields.values().all(digested),
        Value::Array(items) => items.iter().all(digested),
        Value::String(s) => crate::addr::check_sha256_hex(s).is_ok(),
        _ => false,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Outcome {
    Found { facts: Facts },
    NotFound,
}

impl Outcome {
    pub fn digested(&self) -> bool {
        match self {
            Self::Found { facts } => facts.digested(),
            Self::NotFound => true,
        }
    }

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
        Ok(FactAddress::of(h))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn version(sha: &str) -> ProbeVersion {
        ProbeVersion::try_new(sha.repeat(64)).expect("the fixture spells a hash")
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

    #[test]
    fn an_entry_written_before_the_open_surface_existed_stays_unknown() {
        for legacy in ["\"open\"", "\"declared\"", "\"unverifiable\""] {
            let read: Verifiability = serde_json::from_str(legacy).unwrap();
            assert_eq!(
                read,
                Verifiability::Open {
                    over: BTreeSet::from([Openness::Unknown])
                },
                "a row from before probes declared what they do not close over cannot be \
                 re-graded later, only blessed. `Unknown` is that row saying so out loud, \
                 which is the whole reason the field went in before M2 mints observations \
                 rather than after"
            );
        }
    }

    #[test]
    fn a_closed_probe_reads_back_closed_in_either_spelling() {
        for closed in ["\"closed\"", "\"content_addressed\""] {
            assert_eq!(
                serde_json::from_str::<Verifiability>(closed).unwrap(),
                Verifiability::Closed
            );
        }
    }

    #[test]
    fn an_open_surface_survives_the_wire() {
        let v = Verifiability::open([Openness::Network, Openness::Clock]);
        let s = serde_json::to_string(&v).unwrap();
        assert_eq!(serde_json::from_str::<Verifiability>(&s).unwrap(), v);
    }

    #[test]
    fn open_over_nothing_is_unknown_rather_than_a_quiet_closed() {
        assert_eq!(
            Verifiability::open([]),
            Verifiability::Open {
                over: BTreeSet::from([Openness::Unknown])
            },
            "an empty surface would read as `Open` with nothing outside it, which is \
             `Closed` said badly. Refusing the empty set keeps the rule that there is \
             no `counts as closed really` spelling"
        );
    }
}
