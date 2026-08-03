use serde::{Deserialize, Serialize};

use crate::anchor::AnchorKey;
use crate::string_newtype;

fn check_nonempty_128(s: &str) -> Result<(), String> {
    if s.is_empty() {
        return Err("must not be empty".to_owned());
    }
    if s.len() > 128 {
        return Err("must be at most 128 chars".to_owned());
    }
    Ok(())
}

string_newtype! {
    ProviderId, check_nonempty_128
}

string_newtype! {
    ExternalId, check_nonempty_128
}

string_newtype! {
    Version, check_nonempty_128
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Ref {
    pub provider: ProviderId,
    pub external_id: ExternalId,
}

impl Ref {
    pub fn new(provider: impl Into<String>, external_id: impl Into<String>) -> Self {
        Self {
            provider: ProviderId::new(provider),
            external_id: ExternalId::new(external_id),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LinkKind(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Link {
    pub to: Ref,
    pub kind: LinkKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Binding {
    pub reference: Ref,
    pub anchors: Vec<AnchorKey>,
    pub bound_version: Version,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binding_roundtrips_the_wire() {
        let b = Binding {
            reference: Ref::new("git", "memories/core-modules.md"),
            anchors: vec![AnchorKey::new("core::modules")],
            bound_version: Version::new("a".repeat(40)),
        };
        let s = serde_json::to_string(&b).unwrap();
        assert_eq!(serde_json::from_str::<Binding>(&s).unwrap(), b);
    }

    #[test]
    fn a_record_may_bind_several_anchors() {
        let b = Binding {
            reference: Ref::new("git", "m.md"),
            anchors: vec![AnchorKey::new("a"), AnchorKey::new("b")],
            bound_version: Version::new("v1"),
        };
        assert_eq!(b.anchors.len(), 2);
    }

    #[test]
    fn link_roundtrips_the_wire() {
        let l = Link {
            to: Ref::new("git", "memories/other.md"),
            kind: LinkKind("contradicts".into()),
        };
        let s = serde_json::to_string(&l).unwrap();
        assert_eq!(serde_json::from_str::<Link>(&s).unwrap(), l);
    }
}
