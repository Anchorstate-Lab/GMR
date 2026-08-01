use crate::ast::Node;
use crate::ctx::Ctx;
use crate::eval::{Evaluated, Fault, eval};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    pub path: String,
    pub fault: Fault,
}

impl std::fmt::Display for Warning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "`{}` cannot be resolved against this observation ({})",
            self.path,
            self.fault.class()
        )
    }
}

pub fn bind(expr: &Node, sample: Ctx<'_>) -> Option<Warning> {
    match eval(expr, sample) {
        Evaluated::Value(_) | Evaluated::Absent => None,
        Evaluated::Fault(fault) => Some(Warning {
            path: expr.render(),
            fault,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;
    use serde_json::json;

    fn warn(src: &str, obs: serde_json::Value) -> Option<Warning> {
        let state = json!({});
        bind(&parse(src).unwrap(), Ctx::new(&obs, &state))
    }

    #[test]
    fn a_path_that_resolves_binds_clean() {
        assert!(warn("obs.a", json!({ "a": 1 })).is_none());
    }

    #[test]
    fn absent_binds_clean() {
        assert!(warn("obs.a", json!({ "a": null })).is_none());
    }

    #[test]
    fn a_typo_warns_rather_than_refusing() {
        let w = warn("obs.signatur", json!({ "signature": "x" })).unwrap();
        assert_eq!(w.fault, Fault::NoSuchField);
        assert!(w.to_string().contains("obs.signatur"));
    }

    #[test]
    fn a_typo_and_a_not_yet_existing_target_look_identical_here() {
        let typo = warn("obs.deprecatoin", json!({ "symbol": "assess" }));
        let not_yet = warn("obs.deprecation", json!({ "symbol": "assess" }));
        assert_eq!(typo.map(|w| w.fault), not_yet.map(|w| w.fault));
    }

    #[test]
    fn a_typo_inside_an_array_still_warns() {
        assert!(warn("{ n: [obs.signatur] }", json!({ "signature": "x" })).is_some());
        assert!(warn("{ n: [obs.signature] }", json!({ "signature": "x" })).is_none());
        assert!(warn("{ n: [] }", json!({})).is_none());
    }

    #[test]
    fn reading_a_state_field_never_warns() {
        assert!(warn("state.whatever", json!({})).is_none());
    }
}
