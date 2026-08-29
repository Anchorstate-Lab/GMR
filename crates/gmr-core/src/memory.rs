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

fn check_provider_id(s: &str) -> Result<(), String> {
    check_nonempty_128(s)?;
    let shaped = s
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !s.starts_with('-');
    match shaped {
        true => Ok(()),
        false => {
            Err("expected lowercase ASCII letters, digits or `-`, not starting with `-`".to_owned())
        }
    }
}

string_newtype! {
    admitted ProviderId, check_provider_id
}

string_newtype! {
    admitted ExternalId, check_nonempty_128
}

string_newtype! {
    admitted Version, check_nonempty_128
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

    pub fn parse(address: &str) -> Option<Self> {
        let (named, rest) = address.split_once(':')?;
        Some(Self {
            provider: ProviderId::try_new(named).ok()?,
            external_id: ExternalId::try_new(rest).ok()?,
        })
    }
}

impl std::fmt::Display for Ref {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.provider, self.external_id)
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Derived,
    SelfAttested,
    Adjudicated,
    Configured,
    Unknown,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Derived => "derived",
            Self::SelfAttested => "self_attested",
            Self::Adjudicated => "adjudicated",
            Self::Configured => "configured",
            Self::Unknown => "unknown",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "derived" => Some(Self::Derived),
            "self_attested" => Some(Self::SelfAttested),
            "adjudicated" => Some(Self::Adjudicated),
            "configured" => Some(Self::Configured),
            "unknown" => Some(Self::Unknown),
            _ => None,
        }
    }

    pub fn independent(self) -> bool {
        matches!(self, Self::Derived | Self::Adjudicated)
    }
}

impl std::fmt::Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_provider_name_has_a_shape_so_an_address_can_be_told_from_a_path() {
        for good in ["git", "mem0", "claude-code", "desk2"] {
            assert!(ProviderId::try_new(good).is_ok(), "{good}");
        }
        for bad in ["memories/a", "My Store", "Mem0", "notes.md", "-leading", ""] {
            assert!(
                ProviderId::try_new(bad).is_err(),
                "`{bad}` must not pass: whether `{bad}:rest` is an address or an id that \
                 happens to contain a colon has to be decidable from the text alone. \
                 Deciding it from which providers this run registered makes one string \
                 mean two different records in two builds, and the wrong one is written \
                 into an append-only table"
            );
        }
    }

    #[test]
    fn only_a_record_that_declared_itself_or_was_judged_stands_on_its_own() {
        assert!(Source::Derived.independent());
        assert!(Source::Adjudicated.independent());
        assert!(
            !Source::SelfAttested.independent(),
            "an agent asserting that its own memory is about a coordinate is the agent \
             vouching for itself. That is worth recording — it is the most accurate moment \
             the link can be made — but it is not evidence a reader can weigh against it"
        );
        assert!(
            !Source::Configured.independent(),
            "a recipe is written by whoever runs the agent; configuration is self-report \
             with a longer life, not a second opinion"
        );
        assert!(
            !Source::Unknown.independent(),
            "an assertion that predates this column may well have been judged by a person, \
             and this store has no way to tell. Counting it as independent would invent the \
             one fact the reader is relying on"
        );
    }

    #[test]
    fn a_source_survives_the_round_trip_the_store_puts_it_through() {
        for source in [
            Source::Derived,
            Source::SelfAttested,
            Source::Adjudicated,
            Source::Configured,
            Source::Unknown,
        ] {
            assert_eq!(
                Source::parse(source.as_str()),
                Some(source),
                "the store writes `as_str` and reads `parse` back; a pair that disagrees \
                 turns every assertion of one kind into an unreadable row"
            );
        }
    }

    #[test]
    fn binding_roundtrips_the_wire() {
        let b = Binding {
            reference: Ref::new("git", "memories/core-modules.md"),
            anchors: vec![AnchorKey::new("core::modules")],
        };
        let s = serde_json::to_string(&b).unwrap();
        assert_eq!(serde_json::from_str::<Binding>(&s).unwrap(), b);
    }

    #[test]
    fn a_record_may_bind_several_anchors() {
        let b = Binding {
            reference: Ref::new("git", "m.md"),
            anchors: vec![AnchorKey::new("a"), AnchorKey::new("b")],
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
