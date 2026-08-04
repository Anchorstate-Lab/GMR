use crate::error::CliError;

/// `reads` lists only paths beyond the coord report level; report-level
/// fields are emitted by every coord probe and need no declaring.
#[derive(Debug)]
pub struct Shape {
    pub name: &'static str,
    pub rules: &'static [&'static str],
    pub reads: &'static [&'static str],
}

const ROSTER: Shape = Shape {
    name: "roster",
    reads: &[],
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
    reads: &["at.scope", "facts.occurrences", "facts.files"],
    rules: &[
        r#"not exists(state.n) => { position: state.position, scope: obs.at.scope, n: obs.facts.occurrences, files: obs.facts.files, status: "captured" }"#,
        r#"obs.at.scope != state.scope => { position: state.position, scope: obs.at.scope, n: obs.facts.occurrences, files: obs.facts.files, was: state.scope, status: "entered-code" }"#,
        r#"obs.facts.occurrences != state.n => { position: state.position, scope: obs.at.scope, n: obs.facts.occurrences, files: obs.facts.files, was: state.n, status: "count-moved" }"#,
    ],
};

const FINGERPRINT: Shape = Shape {
    name: "fingerprint",
    reads: &["at.fingerprint", "facts.line"],
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

/// Paths the shape reads that the probe does not emit. Non-empty means refuse
/// to open: opening anyway yields silent garbage state or Unevaluable.
pub fn unmet(shape: &Shape, obs: &crate::probes::Obs) -> Vec<&'static str> {
    shape
        .reads
        .iter()
        .copied()
        .filter(|path| {
            let known = match path.split_once('.') {
                Some(("at", item)) => obs.at.iter().any(|k| k == item),
                Some(("facts", key)) => obs.facts.iter().any(|k| k == key),
                _ => false,
            };
            !known
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shape_is_a_program_the_evaluator_accepts() {
        for shape in ALL {
            crate::rules::transitions(&rules_of(shape))
                .unwrap_or_else(|e| panic!("shape `{}` does not parse: {e}", shape.name));
        }
    }

    fn obs(at: &[&str], facts: &[&str]) -> crate::probes::Obs {
        crate::probes::Obs {
            schema: "gmr.probe-coord.v1".to_owned(),
            at: at.iter().map(|s| s.to_string()).collect(),
            facts: facts.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn roster_rides_any_coord_probe() {
        assert!(unmet(&ROSTER, &obs(&[], &[])).is_empty());
    }

    #[test]
    fn occurrence_needs_name_maps_vocabulary() {
        let name_map = obs(
            &["name", "scope"],
            &["occurrences", "file_count", "files", "first"],
        );
        assert!(unmet(&OCCURRENCE, &name_map).is_empty());

        let ast_map = obs(&["file", "kind", "vis", "name", "shape"], &["body", "line"]);
        assert_eq!(
            unmet(&OCCURRENCE, &ast_map),
            vec!["at.scope", "facts.occurrences", "facts.files"]
        );
    }

    #[test]
    fn fingerprint_takes_prose_map_but_not_addr_map() {
        assert!(
            unmet(
                &FINGERPRINT,
                &obs(&["file", "heading", "fingerprint"], &["line", "lines"])
            )
            .is_empty()
        );
        // addr-map emits at.fingerprint too, but its facts hold only bytes.
        assert_eq!(
            unmet(
                &FINGERPRINT,
                &obs(&["path", "name", "fingerprint"], &["bytes"])
            ),
            vec!["facts.line"]
        );
    }

    #[test]
    fn an_unknown_shape_names_what_this_build_ships() {
        let e = get("nope").unwrap_err();
        assert!(e.to_string().contains("roster"), "{e}");
    }
}
