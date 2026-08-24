use std::collections::BTreeSet;

use crate::error::CliError;

pub use gmr_survey::matching::{COORD_REPORT_SCHEMA as COORD_SCHEMA, REPORT};

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

#[derive(Debug, Default)]
pub struct Writes {
    pub paths: BTreeSet<String>,
    pub opaque: BTreeSet<String>,
}

impl Writes {
    pub fn reaches(&self, path: &str) -> bool {
        if self.paths.contains(path) {
            return true;
        }
        let mut prefix = String::new();
        for part in path.split('.') {
            if !prefix.is_empty() {
                prefix.push('.');
            }
            prefix.push_str(part);
            if self.opaque.contains(&prefix) {
                return true;
            }
        }
        false
    }

    pub fn render(&self) -> String {
        self.paths.iter().cloned().collect::<Vec<_>>().join(" · ")
    }
}

pub fn writes_of(transitions: &gmr::Transitions) -> Result<Writes, CliError> {
    let mut out = Writes::default();
    for rule in transitions.iter() {
        let node = gmr::expr::parse(&rule.to.source)
            .map_err(|e| CliError(format!("`{}`: {e}", rule.to.source)))?;
        constructed(&node, "", &mut out);
    }
    Ok(out)
}

fn constructed(node: &gmr::expr::Node, prefix: &str, out: &mut Writes) {
    let gmr::expr::Node::Object(fields) = node else {
        if !prefix.is_empty() {
            out.opaque.insert(prefix.to_owned());
        }
        return;
    };
    for (key, value) in fields {
        let path = match prefix.is_empty() {
            true => key.clone(),
            false => format!("{prefix}.{key}"),
        };
        out.paths.insert(path.clone());
        constructed(value, &path, out);
    }
}

pub fn state_paths(node: &gmr::expr::Node) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    walk_state(node, &mut out);
    out
}

fn walk_state(node: &gmr::expr::Node, out: &mut BTreeSet<String>) {
    use gmr::expr::{Node, Root, Step};
    match node {
        Node::Path(p) if p.root == Root::State => {
            let fields: Vec<&str> = p
                .steps
                .iter()
                .map_while(|s| match s {
                    Step::Field(name) => Some(name.as_str()),
                    Step::Index(_) => None,
                })
                .collect();
            if !fields.is_empty() {
                out.insert(fields.join("."));
            }
        }
        Node::Path(_) | Node::Lit(_) | Node::Changed(_) => {}
        Node::Exists(x) | Node::Not(x) | Node::Neg(x) => walk_state(x, out),
        Node::Binary { lhs, rhs, .. } => {
            walk_state(lhs, out);
            walk_state(rhs, out);
        }
        Node::Object(fields) => fields.iter().for_each(|(_, v)| walk_state(v, out)),
        Node::Array(items) => items.iter().for_each(|v| walk_state(v, out)),
    }
}

fn known(obs: &crate::probes::Obs) -> BTreeSet<String> {
    if obs.schema == COORD_SCHEMA {
        let mut out: BTreeSet<String> = REPORT.iter().map(|s| (*s).to_owned()).collect();
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
            identity: Vec::new(),
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
