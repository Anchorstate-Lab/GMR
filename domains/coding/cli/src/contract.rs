use std::collections::BTreeSet;

use crate::error::CliError;

const COORD_REPORT_FIELDS: &[&str] = &[
    "schema",
    "extractor",
    "found",
    "matched",
    "missed",
    "candidates",
    "roll",
    "exact",
    "matches",
    "priority",
];

pub const COORD_SCHEMA: &str = "gmr.probe-coord.v1";

pub fn reads_of(transitions: &gmr::Transitions) -> Result<BTreeSet<String>, CliError> {
    let mut out = BTreeSet::new();
    for rule in transitions.iter() {
        for expr in [&rule.when, &rule.to] {
            let node = gmr::expr::parse(&expr.source)
                .map_err(|e| CliError(format!("`{}`: {e}", expr.source)))?;
            out.extend(node.reads_obs());
        }
    }
    Ok(out)
}

fn known(obs: &crate::probes::Obs) -> BTreeSet<String> {
    if obs.schema == COORD_SCHEMA {
        let mut out: BTreeSet<String> = COORD_REPORT_FIELDS
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        out.extend(obs.at.iter().map(|k| format!("at.{k}")));
        out.extend(obs.facts.iter().map(|k| format!("facts.{k}")));
        out
    } else {
        obs.at.iter().chain(&obs.facts).cloned().collect()
    }
}

pub fn unmet(reads: &BTreeSet<String>, obs: &crate::probes::Obs) -> Vec<String> {
    let known = known(obs);
    reads
        .iter()
        .filter(|r| !known.contains(*r))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obs(schema: &str, at: &[&str], facts: &[&str]) -> crate::probes::Obs {
        crate::probes::Obs {
            schema: schema.to_owned(),
            at: at.iter().map(|s| s.to_string()).collect(),
            facts: facts.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn a_hand_written_rule_reading_an_undeclared_field_is_caught() {
        let transitions =
            crate::rules::transitions(&["obs.sha != state.sha => { status: \"moved\" }".into()])
                .unwrap();
        let reads = reads_of(&transitions).unwrap();
        let script_probe = obs("gmr.probe.v1", &[], &["pending"]);
        assert_eq!(unmet(&reads, &script_probe), vec!["sha"]);
    }

    #[test]
    fn a_coord_probes_report_level_fields_are_known_without_being_declared() {
        let transitions = crate::rules::transitions(&[
            "obs.exact == false => { status: \"gone\" }".into(),
            "obs.roll != state.roll => { status: \"swapped\" }".into(),
        ])
        .unwrap();
        let reads = reads_of(&transitions).unwrap();
        assert!(unmet(&reads, &obs(COORD_SCHEMA, &[], &[])).is_empty());
    }
}
