use std::collections::BTreeSet;

use crate::error::CliError;

#[derive(Debug)]
pub struct Shape {
    pub name: &'static str,
    body: Body,
}

#[derive(Debug)]
enum Body {
    Table(&'static [&'static str]),
    Vector(&'static [Dim]),
}

#[derive(Debug)]
pub struct Dim {
    pub name: &'static str,
    pub status: &'static str,
    field: &'static str,
    obs: &'static str,
}

const CONTRACT: Shape = Shape {
    name: "contract",
    body: Body::Vector(&[
        Dim {
            name: "sig",
            status: "signature-changed",
            field: "sig",
            obs: "obs.at.shape",
        },
        Dim {
            name: "logic",
            status: "logic-changed",
            field: "body",
            obs: "obs.facts.body",
        },
        Dim {
            name: "file",
            status: "moved-file",
            field: "file",
            obs: "obs.at.file",
        },
        Dim {
            name: "line",
            status: "moved-line",
            field: "line",
            obs: "obs.facts.line",
        },
    ]),
};

const ROSTER: Shape = Shape {
    name: "roster",
    body: Body::Table(&[
        r#"obs.exact == false => { position: state.position, n: 0, matches: [], status: "coordinate-missed" }"#,
        r#"not exists(state.n) => { position: state.position, n: obs.candidates, matches: obs.matches, status: "captured" }"#,
        r#"obs.candidates > state.n => { position: state.position, n: obs.candidates, matches: obs.matches, was: state.matches, status: "added" }"#,
        r#"obs.candidates < state.n => { position: state.position, n: obs.candidates, matches: obs.matches, was: state.matches, status: "removed" }"#,
        r#"changed("matches") => { position: state.position, n: obs.candidates, matches: obs.matches, was: state.matches, status: "moved" }"#,
    ]),
};

const OCCURRENCE: Shape = Shape {
    name: "occurrence",
    body: Body::Table(&[
        r#"not exists(state.n) => { position: state.position, scope: obs.at.scope, n: obs.facts.occurrences, files: obs.facts.files, status: "captured" }"#,
        r#"obs.at.scope != state.scope => { position: state.position, scope: obs.at.scope, n: obs.facts.occurrences, files: obs.facts.files, was: state.scope, status: "entered-code" }"#,
        r#"obs.facts.occurrences != state.n => { position: state.position, scope: obs.at.scope, n: obs.facts.occurrences, files: obs.facts.files, was: state.n, status: "count-moved" }"#,
    ]),
};

const FINGERPRINT: Shape = Shape {
    name: "fingerprint",
    body: Body::Table(&[
        r#"not exists(state.fingerprint) => { position: state.position, fingerprint: obs.at.fingerprint, line: obs.facts.line, status: "captured" }"#,
        r#"obs.found == false => { position: state.position, fingerprint: state.fingerprint, status: "section-gone" }"#,
        r#"obs.at.fingerprint != state.fingerprint => { position: state.position, fingerprint: obs.at.fingerprint, line: obs.facts.line, was: state.fingerprint, status: "drifted" }"#,
    ]),
};

const SYMBOL: Shape = Shape {
    name: "symbol",
    body: Body::Table(&[
        r#"obs.found == false => { position: state.position, status: "missing" }"#,
        r#"not exists(state.signature) => { position: state.position, signature: obs.at.shape, body: obs.facts.body, file: obs.at.file, line: obs.facts.line, status: "captured" }"#,
        r#"obs.at.shape != state.signature => { position: state.position, signature: obs.at.shape, body: obs.facts.body, file: obs.at.file, line: obs.facts.line, was: state.signature, status: "signature-changed" }"#,
        r#"obs.facts.body != state.body => { position: state.position, signature: obs.at.shape, body: obs.facts.body, file: obs.at.file, line: obs.facts.line, was: state.body, status: "logic-changed" }"#,
        r#"obs.at.file != state.file => { position: state.position, signature: obs.at.shape, body: obs.facts.body, file: obs.at.file, line: obs.facts.line, was: state.file, status: "moved-file" }"#,
        r#"obs.facts.line != state.line => { position: state.position, signature: obs.at.shape, body: obs.facts.body, file: obs.at.file, line: obs.facts.line, was: state.line, status: "moved-line" }"#,
    ]),
};

pub const ALL: &[&Shape] = &[&CONTRACT, &ROSTER, &OCCURRENCE, &FINGERPRINT, &SYMBOL];

pub fn get(name: &str) -> Result<&'static Shape, CliError> {
    ALL.iter().copied().find(|s| s.name == name).ok_or_else(|| {
        CliError(format!(
            "unknown shape `{name}`; this build ships {}",
            ALL.iter().map(|s| s.name).collect::<Vec<_>>().join(" · ")
        ))
    })
}

pub fn rules_of(shape: &Shape) -> Vec<String> {
    match shape.body {
        Body::Table(rules) => rules.iter().map(|r| (*r).to_owned()).collect(),
        Body::Vector(dims) => expand(dims),
    }
}

fn object(fields: &[(String, String)]) -> String {
    let body: Vec<String> = fields.iter().map(|(k, v)| format!("{k}: {v}")).collect();
    format!("{{ {} }}", body.join(", "))
}

fn reading(dims: &[Dim]) -> String {
    object(
        &dims
            .iter()
            .map(|d| (d.field.into(), d.obs.into()))
            .collect::<Vec<_>>(),
    )
}

fn vector(dims: &[Dim], missing: &str, each: impl Fn(&Dim) -> String) -> String {
    let mut fields = vec![("missing".to_owned(), missing.to_owned())];
    fields.extend(dims.iter().map(|d| (d.name.to_owned(), each(d))));
    object(&fields)
}

fn bit(d: &Dim) -> String {
    format!(
        "state.v.{} or ({} != state.baseline.{})",
        d.name, d.obs, d.field
    )
}

fn seen(dims: &[Dim], status: &str) -> String {
    object(&[
        ("position".into(), "state.position".into()),
        ("baseline".into(), "state.baseline".into()),
        ("now".into(), reading(dims)),
        ("v".into(), vector(dims, "false", bit)),
        ("status".into(), format!("\"{status}\"")),
    ])
}

fn expand(dims: &[Dim]) -> Vec<String> {
    let mut out = Vec::with_capacity(dims.len() + 4);

    out.push(format!(
        "not exists(state.baseline) and obs.found => {}",
        object(&[
            ("position".into(), "state.position".into()),
            ("baseline".into(), reading(dims)),
            ("now".into(), reading(dims)),
            ("v".into(), vector(dims, "false", |_| "false".into())),
            ("status".into(), "\"captured\"".into()),
        ])
    ));

    out.push(format!(
        "not exists(state.baseline) => {}",
        object(&[
            ("position".into(), "state.position".into()),
            ("v".into(), vector(dims, "true", |_| "false".into())),
            ("status".into(), "\"absent\"".into()),
        ])
    ));

    out.push(format!(
        "obs.found == false => {}",
        object(&[
            ("position".into(), "state.position".into()),
            ("baseline".into(), "state.baseline".into()),
            ("now".into(), "state.now".into()),
            (
                "v".into(),
                vector(dims, "true", |d| format!("state.v.{}", d.name))
            ),
            ("status".into(), "\"missing\"".into()),
        ])
    ));

    out.extend(
        dims.iter()
            .map(|d| format!("{} => {}", bit(d), seen(dims, d.status))),
    );
    out.push(format!("true => {}", seen(dims, SETTLED)));
    out
}

pub const SETTLED: &str = "settled";

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

const COORD_SCHEMA: &str = "gmr.probe-coord.v1";

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
    use serde_json::Value;

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

    #[test]
    fn a_hand_written_rule_reading_an_undeclared_field_is_caught() {
        let transitions =
            crate::rules::transitions(&["obs.sha != state.sha => { status: \"moved\" }".into()])
                .unwrap();
        let reads = reads_of(&transitions).unwrap();
        let script_probe = obs("gmr.probe.v1", &[], &["pending"]);
        assert_eq!(unmet(&reads, &script_probe), vec!["sha"]);
    }

    fn step(rules: &[String], obs: &serde_json::Value, state: &serde_json::Value) -> Value {
        for text in rules {
            let rule = crate::rules::rule(text).unwrap();
            let guard = gmr::expr::parse(&rule.when.source)
                .unwrap_or_else(|e| panic!("guard `{}` does not parse: {e}", rule.when.source));
            let ctx = gmr::expr::Ctx::new(obs, state);
            match gmr::expr::eval(&guard, ctx) {
                gmr::expr::Evaluated::Value(Value::Bool(true)) => {}
                gmr::expr::Evaluated::Value(Value::Bool(false)) | gmr::expr::Evaluated::Absent => {
                    continue;
                }
                other => panic!("guard `{}` is not a boolean: {other:?}", rule.when.source),
            }
            let body = gmr::expr::parse(&rule.to.source).unwrap();
            return match gmr::expr::eval(&body, ctx) {
                gmr::expr::Evaluated::Value(v @ Value::Object(_)) => v,
                other => panic!(
                    "new state of `{}` is not an object: {other:?}",
                    rule.to.source
                ),
            };
        }
        panic!("no rule matched; a vector shape must end in a `true` rule");
    }

    fn contract() -> Vec<String> {
        rules_of(get("contract").unwrap())
    }

    fn sighted(sig: &str, body: &str, file: &str, line: i64) -> Value {
        serde_json::json!({
            "schema": COORD_SCHEMA, "extractor": "ast-map", "found": true,
            "matched": ["file", "name"], "missed": [],
            "at": { "file": file, "kind": "function", "vis": "pub", "name": "f", "shape": sig },
            "facts": { "body": body, "line": line },
            "candidates": 1, "matches": [], "exact": true,
        })
    }

    fn unsighted() -> Value {
        serde_json::json!({
            "schema": COORD_SCHEMA, "extractor": "ast-map", "found": false,
            "matched": [], "missed": ["file", "name"],
            "at": Value::Null, "facts": Value::Null,
            "candidates": 0, "matches": [], "exact": false,
        })
    }

    fn bits(state: &Value) -> Vec<(String, bool)> {
        state["v"]
            .as_object()
            .expect("a vector shape always writes v")
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    v.as_bool()
                        .unwrap_or_else(|| panic!("v.{k} is not a bool: {v}")),
                )
            })
            .collect()
    }

    fn set(state: &Value) -> Vec<String> {
        bits(state)
            .into_iter()
            .filter(|(_, on)| *on)
            .map(|(k, _)| k)
            .collect()
    }

    #[test]
    fn every_generated_rule_is_a_program_the_evaluator_accepts() {
        let rules = contract();
        let statuses = [
            "captured",
            "absent",
            "missing",
            "signature-changed",
            "logic-changed",
            "moved-file",
            "moved-line",
            "settled",
        ];
        assert_eq!(rules.len(), statuses.len());
        for s in statuses {
            assert!(
                rules.iter().any(|r| r.contains(&format!("\"{s}\""))),
                "no rule can ever produce `{s}`"
            );
        }
        let transitions = crate::rules::transitions(&rules).unwrap();
        reads_of(&transitions).unwrap();
    }

    #[test]
    fn contract_rides_ast_map() {
        let reads = reads_of_shape(get("contract").unwrap());
        let ast_map = obs(
            COORD_SCHEMA,
            &["file", "kind", "vis", "name", "shape"],
            &["body", "line"],
        );
        assert!(unmet(&reads, &ast_map).is_empty());

        let name_map = obs(COORD_SCHEMA, &["name", "scope"], &["occurrences"]);
        assert_eq!(
            unmet(&reads, &name_map),
            ["at.file", "at.shape", "facts.body", "facts.line"]
        );
    }

    #[test]
    fn three_changes_at_once_all_land() {
        let r = contract();
        let s = step(&r, &sighted("(a) -> B", "body1", "a.rs", 1), &Value::Null);
        assert_eq!(s["status"], "captured");
        assert!(set(&s).is_empty(), "a first sighting drifts from nothing");

        let s = step(&r, &sighted("(a, b) -> B", "body2", "a.rs", 9), &s);
        assert_eq!(set(&s), ["sig", "logic", "line"]);
        assert_eq!(s["status"], "signature-changed");
    }

    #[test]
    fn the_vector_is_ordered_by_priority() {
        let s = step(
            &contract(),
            &sighted("(a) -> B", "body1", "a.rs", 1),
            &Value::Null,
        );
        let axes: Vec<String> = bits(&s).into_iter().map(|(k, _)| k).collect();
        assert_eq!(axes, ["missing", "sig", "logic", "file", "line"]);
    }

    #[test]
    fn a_bit_never_clears_on_its_own() {
        let r = contract();
        let s = step(&r, &sighted("(a) -> B", "body1", "a.rs", 1), &Value::Null);
        let s = step(&r, &sighted("(a, b) -> B", "body1", "a.rs", 1), &s);
        assert_eq!(set(&s), ["sig"]);

        let s = step(&r, &sighted("(a, b) -> B", "body2", "a.rs", 1), &s);
        assert_eq!(set(&s), ["sig", "logic"], "sig must not clear itself");

        let s = step(&r, &sighted("(a, b) -> B", "body2", "a.rs", 1), &s);
        assert_eq!(
            set(&s),
            ["sig", "logic"],
            "a quiet observation clears nothing"
        );
    }

    #[test]
    fn a_miss_does_not_poison_the_other_axes() {
        let r = contract();
        let s = step(&r, &sighted("(a) -> B", "body1", "a.rs", 1), &Value::Null);
        let s = step(&r, &unsighted(), &s);
        assert_eq!(s["status"], "missing");
        assert_eq!(
            set(&s),
            ["missing"],
            "a miss says nothing about the other axes"
        );
    }

    #[test]
    fn a_miss_heals_itself_and_carries_real_drift_across() {
        let r = contract();
        let s = step(&r, &sighted("(a) -> B", "body1", "a.rs", 1), &Value::Null);
        let s = step(&r, &sighted("(a, b) -> B", "body1", "a.rs", 1), &s);
        let s = step(&r, &unsighted(), &s);
        assert_eq!(set(&s), ["missing", "sig"]);

        let s = step(&r, &sighted("(a, b) -> B", "body1", "a.rs", 1), &s);
        assert_eq!(set(&s), ["sig"], "missing heals; sig stays accumulated");
    }

    #[test]
    fn a_miss_before_any_baseline_still_captures_later() {
        let r = contract();
        let s = step(&r, &unsighted(), &Value::Null);
        assert_eq!(s["status"], "absent");
        assert!(
            s.get("baseline").is_none(),
            "writing a null baseline would make `captured` unreachable, got {s}"
        );

        let s = step(&r, &sighted("(a) -> B", "body1", "a.rs", 1), &s);
        assert_eq!(s["status"], "captured");
        assert!(
            set(&s).is_empty(),
            "the first real sighting is the baseline"
        );
    }

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
