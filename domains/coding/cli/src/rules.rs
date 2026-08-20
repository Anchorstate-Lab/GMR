use std::collections::BTreeSet;

use gmr::{AnchorKey, Expr, Kind, ProbeName, ProbeRef, Rule, StatusId, Transitions};

use crate::error::CliError;

pub fn key(text: &str) -> Result<AnchorKey, CliError> {
    AnchorKey::try_new(text).map_err(|e| {
        CliError(format!(
            "`{text}` cannot be an anchor key ({e}).\n\
             The journal is append-only, so a key is refused here or never: once one entry \
             carries it, every later read has to accept it back."
        ))
    })
}

pub fn params(text: &str) -> Result<serde_json::Value, CliError> {
    serde_json::from_str(text).map_err(|e| CliError(format!("params is not valid JSON: {e}")))
}

pub fn probe(kind: Kind, name: &str, params: serde_json::Value) -> Result<ProbeRef, CliError> {
    let name = ProbeName::try_new(name).map_err(|e| {
        CliError(format!(
            "`{name}` is not a probe name ({e}).\n\
             `probes list` prints the names this build knows."
        ))
    })?;
    Ok(ProbeRef::new(kind, name, params))
}

pub fn rule(text: &str) -> Result<Rule, CliError> {
    let (when, to) = text.split_once("=>").ok_or_else(|| {
        CliError(format!(
            "transition rules must be written as `GUARD => NEW_STATE`; got `{text}`\n\
             example: changed(\"shape\") => {{ shape: obs.shape, status: \"drifted\" }}"
        ))
    })?;
    let (when, to) = (when.trim(), to.trim());
    if when.is_empty() || to.is_empty() {
        return Err(CliError(format!(
            "`{text}` has an empty guard or new state"
        )));
    }
    Ok(Rule {
        when: Expr::text(when),
        to: Expr::text(to),
    })
}

pub fn transitions(texts: &[String]) -> Result<Transitions, CliError> {
    texts
        .iter()
        .map(|t| rule(t))
        .collect::<Result<_, _>>()
        .map(Transitions)
}

pub fn terminal(names: &[String]) -> Result<BTreeSet<StatusId>, CliError> {
    names
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| {
            StatusId::try_new(s).map_err(|e| {
                CliError(format!(
                    "`{s}` cannot be a terminal status ({e}). \
                     A terminal status seals an anchor for good, and the base compares it \
                     by equality against whatever a rule produces — a status nothing can \
                     spell twice would never match"
                ))
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rule_splits_on_the_arrow() {
        let r = rule("changed(\"shape\") => { status: \"drifted\" }").unwrap();
        assert_eq!(r.when.source, "changed(\"shape\")");
        assert_eq!(r.to.source, "{ status: \"drifted\" }");
    }

    #[test]
    fn a_rule_without_an_arrow_says_what_it_wanted() {
        let e = rule("changed(\"shape\")").unwrap_err();
        assert!(e.0.contains("GUARD => NEW_STATE"));
    }

    #[test]
    fn the_arrow_inside_an_expression_still_splits_at_the_first_one() {
        assert!(rule("a => b => c").unwrap().to.source == "b => c");
    }

    #[test]
    fn a_key_no_store_could_hand_back_is_refused_at_the_door() {
        let e = key(&"k".repeat(400)).unwrap_err();
        assert!(e.0.contains("append-only"), "{}", e.0);
        assert!(
            key("crates/gmr-core/src/addr.rs#canonical_number_string").is_ok(),
            "the keys this repository already runs on have to keep passing"
        );
    }

    #[test]
    fn a_status_that_seals_an_anchor_has_to_be_one_a_rule_can_spell_again() {
        assert!(
            terminal(&["settled".into(), " expired ".into()])
                .unwrap()
                .len()
                == 2
        );
        assert!(
            terminal(&["".into(), "  ".into()]).unwrap().is_empty(),
            "an empty entry is somebody writing `--terminal a,,b`, not a status"
        );
        assert!(terminal(&["s".repeat(400)]).is_err());
    }
}
