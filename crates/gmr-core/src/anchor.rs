use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::addr::{ContentHash, content_hash_of};
use crate::probe::ProbeRef;
use crate::string_newtype;

pub const POSITION: &str = "position";

pub const STATUS: &str = "status";

fn check_key(s: &str) -> Result<(), String> {
    if s.is_empty() {
        return Err("must not be empty".to_owned());
    }
    if s.len() > 128 {
        return Err("must be at most 128 chars".to_owned());
    }
    Ok(())
}

fn check_status(s: &str) -> Result<(), String> {
    if s.is_empty() {
        return Err("must not be empty".to_owned());
    }
    if s.len() > 64 {
        return Err("must be at most 64 chars".to_owned());
    }
    Ok(())
}

string_newtype! {
    AnchorKey, check_key
}

string_newtype! {
    StatusId, check_status
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Expr {
    pub source: String,
    pub hash: ContentHash,
}

impl Expr {
    pub fn text(source: impl Into<String>) -> Self {
        let source = source.into();
        let hash = content_hash_of(&Value::String(source.clone()))
            .expect("Value::String never exceeds canonicalization limits");
        Self { source, hash }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    pub when: Expr,
    pub to: Expr,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Transitions(pub Vec<Rule>);

impl Transitions {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Rule> {
        self.0.iter()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct State(pub Value);

impl State {
    pub fn new(v: Value) -> Self {
        Self(v)
    }

    pub fn position(&self) -> &Value {
        self.0.get(POSITION).unwrap_or(&Value::Null)
    }

    pub fn status(&self) -> Option<StatusId> {
        self.0
            .get(STATUS)
            .and_then(Value::as_str)
            .map(StatusId::new)
    }

    pub fn as_value(&self) -> &Value {
        &self.0
    }
}

impl Default for State {
    fn default() -> Self {
        Self(Value::Object(serde_json::Map::new()))
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Retain {
    #[default]
    Tick,
    Full,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunSettings {
    #[serde(default)]
    pub retain: Retain,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cadence_secs: Option<u64>,
}

impl RunSettings {
    pub fn retains_full(&self) -> bool {
        matches!(self.retain, Retain::Full)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Superseded {
    pub key: AnchorKey,
    pub rationale: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Anchor {
    pub key: AnchorKey,
    pub probe: ProbeRef,
    pub transitions: Transitions,
    #[serde(default)]
    pub terminal: BTreeSet<StatusId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<Superseded>,
}

impl Anchor {
    pub fn is_terminal(&self, state: &State) -> bool {
        state.status().is_some_and(|s| self.terminal.contains(&s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn anchor(terminal: &[&str]) -> Anchor {
        Anchor {
            key: AnchorKey::new("a"),
            probe: crate::probe::ProbeRef::new(
                crate::probe::Kind::new("shell"),
                crate::probe::ProbeName::new("p"),
                json!({}),
            ),
            transitions: Transitions::default(),
            terminal: terminal.iter().map(|s| StatusId::new(*s)).collect(),
            supersedes: None,
        }
    }

    #[test]
    fn expression_identity_is_its_hash() {
        assert_eq!(Expr::text("obs.a").hash, Expr::text("obs.a").hash);
        assert_ne!(Expr::text("obs.a").hash, Expr::text("obs.b").hash);
    }

    #[test]
    fn position_is_a_slot_the_substrate_carries_but_never_reads_into() {
        let s = State::new(json!({ POSITION: { "file": "a.rs", "symbol": "assess" } }));
        assert_eq!(s.position(), &json!({ "file": "a.rs", "symbol": "assess" }));
    }

    #[test]
    fn a_state_without_a_position_is_legal() {
        assert_eq!(State::default().position(), &Value::Null);
    }

    #[test]
    fn terminal_is_decided_by_the_status_slot_alone() {
        let a = anchor(&["settled", "expired"]);
        assert!(a.is_terminal(&State::new(json!({ STATUS: "settled" }))));
        assert!(!a.is_terminal(&State::new(json!({ STATUS: "drifted" }))));
    }

    #[test]
    fn a_state_with_no_status_is_never_terminal() {
        let a = anchor(&["settled"]);
        assert!(!a.is_terminal(&State::default()));
        assert!(!a.is_terminal(&State::new(json!({ POSITION: "somewhere" }))));
    }

    #[test]
    fn the_substrate_does_not_read_into_the_status() {
        let a = anchor(&["расчёт"]);
        assert!(a.is_terminal(&State::new(json!({ STATUS: "расчёт" }))));
    }

    #[test]
    fn states_compare_by_content() {
        let a = State::new(json!({ "status": "ok", "count": 2 }));
        let b = State::new(json!({ "count": 2, "status": "ok" }));
        assert_eq!(a, b);
    }

    #[test]
    fn anchor_roundtrips_the_wire() {
        let a = anchor(&["settled"]);
        let s = serde_json::to_string(&a).unwrap();
        assert_eq!(serde_json::from_str::<Anchor>(&s).unwrap(), a);
    }
}
