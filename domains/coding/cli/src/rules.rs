use std::collections::BTreeSet;

use gmr::{Expr, Kind, ProbeRef, ProbeVersion, Rule, StatusId, Transitions};

use crate::error::CliError;

/// Probe written on the anchor: which artifact it points at and which params it carries.
pub fn probe(artifact: &str, params: &str) -> Result<ProbeRef, CliError> {
    let artifact = ProbeVersion::try_new(artifact).map_err(|e| {
        CliError(format!(
            "`{artifact}` is not an artifact version ({e}).\n\
             Publish one with `anchor publish <dir>`; it will print this value."
        ))
    })?;
    let params: serde_json::Value = serde_json::from_str(params)
        .map_err(|e| CliError(format!("params is not valid JSON: {e}")))?;
    Ok(ProbeRef::new(Kind::new("shell"), artifact, params))
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
        assert_eq!(r.when.source.as_str().unwrap(), "changed(\"shape\")");
        assert_eq!(r.to.source.as_str().unwrap(), "{ status: \"drifted\" }");
    }

    #[test]
    fn a_rule_without_an_arrow_says_what_it_wanted() {
        let e = rule("changed(\"shape\")").unwrap_err();
        assert!(e.0.contains("GUARD => NEW_STATE"));
    }

    #[test]
    fn the_arrow_inside_an_expression_still_splits_at_the_first_one() {
        assert!(rule("a => b => c").unwrap().to.source.as_str().unwrap() == "b => c");
    }
}
