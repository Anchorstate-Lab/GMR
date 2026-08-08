use std::collections::BTreeSet;

use gmr::{Expr, Kind, ProbeName, ProbeRef, Rule, StatusId, Transitions};

use crate::error::CliError;

pub fn probe(kind: Kind, name: &str, params: &str) -> Result<ProbeRef, CliError> {
    let name = ProbeName::try_new(name).map_err(|e| {
        CliError(format!(
            "`{name}` is not a probe name ({e}).\n\
             `probes list` prints the names this build knows."
        ))
    })?;
    let params: serde_json::Value = serde_json::from_str(params)
        .map_err(|e| CliError(format!("params is not valid JSON: {e}")))?;
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

pub fn terminal(names: &[String]) -> BTreeSet<StatusId> {
    names
        .iter()
        .map(|s| StatusId::new(s.trim()))
        .filter(|s| !s.as_str().is_empty())
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
}
