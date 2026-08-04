use std::collections::BTreeSet;

use crate::error::CliError;

#[derive(Debug)]
pub struct Shape {
    pub name: &'static str,
    pub rules: &'static [&'static str],
}

const ROSTER: Shape = Shape {
    name: "roster",
    rules: &[
        r#"obs.exact == false => { position: state.position, n: 0, matches: [], status: "coordinate-missed" }"#,
        r#"not exists(state.n) => { position: state.position, n: obs.candidates, matches: obs.matches, status: "captured" }"#,
        r#"obs.candidates > state.n => { position: state.position, n: obs.candidates, matches: obs.matches, was: state.matches, status: "added" }"#,
        r#"obs.candidates < state.n => { position: state.position, n: obs.candidates, matches: obs.matches, was: state.matches, status: "removed" }"#,
        r#"changed("matches") => { position: state.position, n: obs.candidates, matches: obs.matches, was: state.matches, status: "moved" }"#,
    ],
};

const OCCURRENCE: Shape = Shape {
    name: "occurrence",
    rules: &[
        r#"not exists(state.n) => { position: state.position, scope: obs.at.scope, n: obs.facts.occurrences, files: obs.facts.files, status: "captured" }"#,
        r#"obs.at.scope != state.scope => { position: state.position, scope: obs.at.scope, n: obs.facts.occurrences, files: obs.facts.files, was: state.scope, status: "entered-code" }"#,
        r#"obs.facts.occurrences != state.n => { position: state.position, scope: obs.at.scope, n: obs.facts.occurrences, files: obs.facts.files, was: state.n, status: "count-moved" }"#,
    ],
};

const FINGERPRINT: Shape = Shape {
    name: "fingerprint",
    rules: &[
        r#"not exists(state.fingerprint) => { position: state.position, fingerprint: obs.at.fingerprint, line: obs.facts.line, status: "captured" }"#,
        r#"obs.found == false => { position: state.position, fingerprint: state.fingerprint, status: "section-gone" }"#,
        r#"obs.at.fingerprint != state.fingerprint => { position: state.position, fingerprint: obs.at.fingerprint, line: obs.facts.line, was: state.fingerprint, status: "drifted" }"#,
    ],
};

pub const ALL: &[&Shape] = &[&ROSTER, &OCCURRENCE, &FINGERPRINT];

pub fn get(name: &str) -> Result<&'static Shape, CliError> {
    ALL.iter().copied().find(|s| s.name == name).ok_or_else(|| {
        CliError(format!(
            "unknown shape `{name}`; this build ships {}",
            ALL.iter().map(|s| s.name).collect::<Vec<_>>().join(" · ")
        ))
    })
}

pub fn rules_of(shape: &Shape) -> Vec<String> {
    shape.rules.iter().map(|r| (*r).to_owned()).collect()
}

/// A coord probe emits these unconditionally, so reading them needs nothing
/// declared in the probe's `obs` vocabulary. `at.*`/`facts.*` are the only
/// sub-paths gated on what the probe actually promises.
const COORD_REPORT_FIELDS: &[&str] = &[
    "schema",
    "extractor",
    "found",
    "matched",
    "missed",
    "candidates",
    "exact",
    "matches",
    "priority",
];

/// Must track `gmr_survey::COORD_REPORT_SCHEMA`. Kept as a literal rather than
/// a dependency: this is the one string the CLI needs to know about the coord
/// convention, not a reason to link the crate that defines it.
const COORD_SCHEMA: &str = "gmr.probe-coord.v1";

/// Every obs path a set of transition rules reads, computed from the parsed
/// guards and constructors — not maintained by hand. A rule table and what it
/// needs from a probe cannot drift apart when there is only one list.
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

/// What a probe's declared `obs` vocabulary makes known, in the terms rules
/// read. A coord-shaped probe's report-level fields are always known; only its
/// `at`/`facts` sub-paths are gated on what it actually promises. Any other
/// schema is read flat — `obs.pending` is known when `facts` declares `pending`.
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

/// Paths the rules read that the probe does not emit. Non-empty means refuse
/// to open: opening anyway yields silent garbage state or Unevaluable.
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

    #[test]
    fn every_shape_is_a_program_the_evaluator_accepts() {
        for shape in ALL {
            let transitions = crate::rules::transitions(&rules_of(shape))
                .unwrap_or_else(|e| panic!("shape `{}` does not parse: {e}", shape.name));
            reads_of(&transitions)
                .unwrap_or_else(|e| panic!("shape `{}`'s reads do not parse: {e}", shape.name));
        }
    }

    fn obs(schema: &str, at: &[&str], facts: &[&str]) -> crate::probes::Obs {
        crate::probes::Obs {
            schema: schema.to_owned(),
            at: at.iter().map(|s| s.to_string()).collect(),
            facts: facts.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn reads_of_shape(shape: &Shape) -> BTreeSet<String> {
        reads_of(&crate::rules::transitions(&rules_of(shape)).unwrap()).unwrap()
    }

    #[test]
    fn roster_reads_only_report_level_fields() {
        assert_eq!(
            reads_of_shape(&ROSTER),
            BTreeSet::from(["exact", "candidates", "matches"].map(String::from))
        );
    }

    #[test]
    fn roster_rides_any_coord_probe() {
        assert!(unmet(&reads_of_shape(&ROSTER), &obs(COORD_SCHEMA, &[], &[])).is_empty());
    }

    #[test]
    fn occurrence_needs_name_maps_vocabulary() {
        let reads = reads_of_shape(&OCCURRENCE);
        let name_map = obs(
            COORD_SCHEMA,
            &["name", "scope"],
            &["occurrences", "file_count", "files", "first"],
        );
        assert!(unmet(&reads, &name_map).is_empty());

        let ast_map = obs(
            COORD_SCHEMA,
            &["file", "kind", "vis", "name", "shape"],
            &["body", "line"],
        );
        assert_eq!(
            unmet(&reads, &ast_map),
            vec!["at.scope", "facts.files", "facts.occurrences"]
        );
    }

    #[test]
    fn fingerprint_takes_prose_map_but_not_addr_map() {
        let reads = reads_of_shape(&FINGERPRINT);
        assert!(
            unmet(
                &reads,
                &obs(
                    COORD_SCHEMA,
                    &["file", "heading", "fingerprint"],
                    &["line", "lines"]
                )
            )
            .is_empty()
        );
        // addr-map emits at.fingerprint too, but its facts hold only bytes.
        assert_eq!(
            unmet(
                &reads,
                &obs(COORD_SCHEMA, &["path", "name", "fingerprint"], &["bytes"])
            ),
            vec!["facts.line"]
        );
    }

    #[test]
    fn an_unknown_shape_names_what_this_build_ships() {
        let e = get("nope").unwrap_err();
        assert!(e.to_string().contains("roster"), "{e}");
    }

    /// The gap this module exists to close: explicit rules — the default shape
    /// an agent or a person writes by hand — get exactly the same check a named
    /// shape gets. There is one checker, not one for presets and none for the rest.
    #[test]
    fn a_hand_written_rule_reading_an_undeclared_field_is_caught() {
        let transitions =
            crate::rules::transitions(&["obs.sha != state.sha => { status: \"moved\" }".into()])
                .unwrap();
        let reads = reads_of(&transitions).unwrap();
        let script_probe = obs("gmr.probe.v1", &[], &["pending"]); // declares `pending`, not `sha`
        assert_eq!(unmet(&reads, &script_probe), vec!["sha"]);
    }

    /// A non-coord probe is read flat: `facts` names top-level fields directly,
    /// because there is no report-level envelope to gate sub-paths on.
    #[test]
    fn a_flat_probes_facts_are_known_at_the_top_level() {
        let transitions =
            crate::rules::transitions(&["obs.pending > 0 => { status: \"unapplied\" }".into()])
                .unwrap();
        let reads = reads_of(&transitions).unwrap();
        let script_probe = obs("gmr.probe.v1", &[], &["pending"]);
        assert!(unmet(&reads, &script_probe).is_empty());
    }
}
