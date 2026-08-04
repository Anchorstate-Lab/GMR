use crate::error::CliError;

/// `reads` 只列超出 coord 报告层的路径。报告层字段（found/exact/candidates/
/// matches/at/facts…）任何 coord 探针都吐，不必声明。
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

/// 探针吐得出的 obs 词表：坐标项与事实键，来自配方记录。
pub struct Vocabulary {
    pub at: Vec<String>,
    pub facts: Vec<String>,
}

/// shape 要读、而探针不吐的路径。非空即拒绝开锚。
pub fn unmet(shape: &Shape, vocab: &Vocabulary) -> Vec<&'static str> {
    shape
        .reads
        .iter()
        .copied()
        .filter(|path| {
            let known = match path.split_once('.') {
                Some(("at", item)) => vocab.at.iter().any(|k| k == item),
                Some(("facts", key)) => vocab.facts.iter().any(|k| k == key),
                _ => false,
            };
            !known
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vocab(at: &[&str], facts: &[&str]) -> Vocabulary {
        Vocabulary {
            at: at.iter().map(|s| s.to_string()).collect(),
            facts: facts.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn every_shape_is_a_program_the_evaluator_accepts() {
        for shape in ALL {
            let rules: Vec<String> = shape.rules.iter().map(|r| r.to_string()).collect();
            crate::rules::transitions(&rules)
                .unwrap_or_else(|e| panic!("shape `{}` does not parse: {e}", shape.name));
        }
    }

    #[test]
    fn roster_rides_any_coord_probe() {
        assert!(unmet(&ROSTER, &vocab(&[], &[])).is_empty());
    }

    #[test]
    fn occurrence_needs_name_maps_vocabulary() {
        let name_map = vocab(
            &["name", "scope"],
            &["occurrences", "file_count", "files", "first"],
        );
        assert!(unmet(&OCCURRENCE, &name_map).is_empty());

        let ast_map = vocab(&["file", "kind", "vis", "name", "shape"], &["body", "line"]);
        assert_eq!(
            unmet(&OCCURRENCE, &ast_map),
            vec!["at.scope", "facts.occurrences", "facts.files"]
        );
    }

    #[test]
    fn fingerprint_takes_prose_map_but_not_addr_map() {
        let prose_map = vocab(&["file", "heading", "fingerprint"], &["line", "lines"]);
        assert!(unmet(&FINGERPRINT, &prose_map).is_empty());

        // addr-map 也吐 at.fingerprint，但事实里只有 bytes。
        let addr_map = vocab(&["path", "name", "fingerprint"], &["bytes"]);
        assert_eq!(unmet(&FINGERPRINT, &addr_map), vec!["facts.line"]);
    }

    #[test]
    fn an_unknown_shape_names_what_this_build_ships() {
        let e = get("nope").unwrap_err();
        assert!(e.to_string().contains("roster"), "{e}");
    }
}
