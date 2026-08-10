use crate::error::CliError;

#[derive(Debug)]
pub struct Shape {
    pub name: &'static str,
    dims: &'static [Dim],
    watch: &'static [&'static str],
}

#[derive(Debug)]
pub struct Dim {
    pub name: &'static str,
    pub status: &'static str,
    reads: Reads,
}

#[derive(Debug)]
enum Reads {
    Now {
        guard: &'static str,
    },
    Since {
        field: &'static str,
        obs: &'static str,
        op: Cmp,
    },
}

#[derive(Debug, Clone, Copy)]
enum Cmp {
    Ne,
    Gt,
    Lt,
}

impl Cmp {
    fn as_str(self) -> &'static str {
        match self {
            Cmp::Ne => "!=",
            Cmp::Gt => ">",
            Cmp::Lt => "<",
        }
    }
}

const fn now(name: &'static str, status: &'static str, guard: &'static str) -> Dim {
    Dim {
        name,
        status,
        reads: Reads::Now { guard },
    }
}

const fn since(
    name: &'static str,
    status: &'static str,
    field: &'static str,
    obs: &'static str,
) -> Dim {
    Dim {
        name,
        status,
        reads: Reads::Since {
            field,
            obs,
            op: Cmp::Ne,
        },
    }
}

const fn compare(
    name: &'static str,
    status: &'static str,
    field: &'static str,
    obs: &'static str,
    op: Cmp,
) -> Dim {
    Dim {
        name,
        status,
        reads: Reads::Since { field, obs, op },
    }
}

const GONE: Dim = now(MISSING, "missing", "obs.exact == false");

const CONTRACT: Shape = Shape {
    name: "contract",
    dims: &[
        GONE,
        since("kind", "kind-changed", "form", "obs.at.form"),
        since("sig", "signature-changed", "sig", "obs.at.shape"),
        since("surface", "surface-changed", "surface", "obs.at.surface"),
        since("logic", "logic-changed", "body", "obs.facts.body"),
        since("place", "moved", "after", "obs.at.after"),
    ],
    watch: &["missing", "kind", "sig", "surface", "logic", "place"],
};

const ROSTER: Shape = Shape {
    name: "roster",
    dims: &[
        GONE,
        compare("grew", "grew", "count", "obs.candidates", Cmp::Gt),
        compare("shrank", "shrank", "count", "obs.candidates", Cmp::Lt),
        since("roll", "swapped", "roll", "obs.roll"),
    ],
    watch: &["missing", "grew", "shrank", "roll"],
};

const FINGERPRINT: Shape = Shape {
    name: "fingerprint",
    dims: &[
        GONE,
        since("drift", "drifted", "fingerprint", "obs.at.fingerprint"),
    ],
    watch: &["missing", "drift"],
};

pub const ALL: &[&Shape] = &[&CONTRACT, &ROSTER, &FINGERPRINT];

pub fn get(name: &str) -> Result<&'static Shape, CliError> {
    ALL.iter().copied().find(|s| s.name == name).ok_or_else(|| {
        CliError(format!(
            "unknown shape `{name}`; this build ships {}",
            ALL.iter().map(|s| s.name).collect::<Vec<_>>().join(" · ")
        ))
    })
}

pub fn rules_of(shape: &Shape) -> Vec<String> {
    expand(shape.dims)
}

pub fn of(transitions: &gmr::Transitions) -> Option<&'static Shape> {
    ALL.iter()
        .copied()
        .find(|s| crate::rules::transitions(&rules_of(s)).is_ok_and(|t| &t == transitions))
}

pub fn name_of(transitions: &gmr::Transitions) -> Option<&'static str> {
    of(transitions).map(|s| s.name)
}

pub fn watch_of(shape: &Shape) -> &'static [&'static str] {
    shape.watch
}

pub fn axes_of(shape: &Shape) -> Vec<&'static str> {
    shape.dims.iter().map(|d| d.name).collect()
}

pub const MISSING: &str = "missing";

pub const RETIRED: &[&str] = &[
    "added",
    "captured",
    "count-moved",
    "matches",
    "moved-file",
    "occurrence",
    "removed",
    "section-gone",
    "symbol",
];

#[cfg(test)]
fn vocabulary() -> std::collections::BTreeSet<&'static str> {
    let mut out = std::collections::BTreeSet::from([SETTLED, "absent"]);
    for shape in ALL {
        out.insert(shape.name);
        out.extend(shape.dims.iter().flat_map(|d| [d.name, d.status]));
    }
    out
}

fn object(fields: &[(String, String)]) -> String {
    let body: Vec<String> = fields.iter().map(|(k, v)| format!("{k}: {v}")).collect();
    format!("{{ {} }}", body.join(", "))
}

fn reading(dims: &[Dim]) -> String {
    let mut fields: Vec<(String, String)> = Vec::new();
    for d in dims {
        let Reads::Since { field, obs, .. } = d.reads else {
            continue;
        };
        if !fields.iter().any(|(k, _)| k == field) {
            fields.push((field.into(), obs.into()));
        }
    }
    object(&fields)
}

fn vector(dims: &[Dim], each: impl Fn(&Dim) -> String) -> String {
    object(
        &dims
            .iter()
            .map(|d| (d.name.to_owned(), each(d)))
            .collect::<Vec<_>>(),
    )
}

fn bit(d: &Dim) -> String {
    match d.reads {
        Reads::Now { guard } => guard.to_owned(),
        Reads::Since { field, obs, op } => format!(
            "state.v.{} or ({obs} {} state.baseline.{field})",
            d.name,
            op.as_str()
        ),
    }
}

fn opening(d: &Dim) -> String {
    match d.reads {
        Reads::Now { guard } => guard.to_owned(),
        Reads::Since { .. } => "false".into(),
    }
}

fn seen(dims: &[Dim], status: &str) -> String {
    object(&[
        ("position".into(), "state.position".into()),
        ("baseline".into(), "state.baseline".into()),
        ("now".into(), reading(dims)),
        ("v".into(), vector(dims, bit)),
        ("status".into(), format!("\"{status}\"")),
    ])
}

fn expand(dims: &[Dim]) -> Vec<String> {
    let standing = || dims.iter().filter(|d| matches!(d.reads, Reads::Now { .. }));
    let mut out = Vec::with_capacity(dims.len() + 3);

    out.push(format!(
        "not exists(state.baseline) and obs.exact => {}",
        object(&[
            ("position".into(), "state.position".into()),
            ("baseline".into(), reading(dims)),
            ("now".into(), reading(dims)),
            ("v".into(), vector(dims, opening)),
            ("status".into(), format!("\"{SETTLED}\"")),
        ])
    ));

    out.push(format!(
        "not exists(state.baseline) => {}",
        object(&[
            ("position".into(), "state.position".into()),
            ("v".into(), vector(dims, opening)),
            ("status".into(), "\"absent\"".into()),
        ])
    ));

    out.extend(standing().map(|d| {
        format!(
            "{} => {}",
            bit(d),
            object(&[
                ("position".into(), "state.position".into()),
                ("baseline".into(), "state.baseline".into()),
                ("now".into(), "state.now".into()),
                (
                    "v".into(),
                    vector(dims, |x| match x.reads {
                        Reads::Now { guard } => guard.to_owned(),
                        Reads::Since { .. } => format!("state.v.{}", x.name),
                    })
                ),
                ("status".into(), format!("\"{}\"", d.status)),
            ])
        )
    }));

    out.extend(
        dims.iter()
            .filter(|d| matches!(d.reads, Reads::Since { .. }))
            .map(|d| format!("{} => {}", bit(d), seen(dims, d.status))),
    );
    out.push(format!("true => {}", seen(dims, SETTLED)));
    out
}

pub const SETTLED: &str = "settled";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{COORD_SCHEMA, reads_of, unmet};
    use serde_json::Value;
    use std::collections::BTreeSet;

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
            BTreeSet::from(["exact", "candidates", "roll"].map(String::from))
        );
    }

    #[test]
    fn roster_rides_any_coord_probe() {
        assert!(unmet(&reads_of_shape(&ROSTER), &obs(COORD_SCHEMA, &[], &[])).is_empty());
    }

    #[test]
    fn fingerprint_takes_any_probe_that_names_a_fingerprint() {
        let reads = reads_of_shape(&FINGERPRINT);
        for at in [
            &["file", "heading", "fingerprint"][..],
            &["path", "name", "fingerprint"][..],
        ] {
            assert!(
                unmet(&reads, &obs(COORD_SCHEMA, at, &[])).is_empty(),
                "{at:?}"
            );
        }
        assert_eq!(
            unmet(
                &reads,
                &obs(COORD_SCHEMA, &["name", "scope"], &["occurrences"])
            ),
            vec!["at.fingerprint"]
        );
    }

    fn settled_state(shape: &Shape) -> Value {
        let mut obs = serde_json::Map::new();
        obs.insert("exact".into(), Value::Bool(true));
        let mut at = serde_json::Map::new();
        let mut facts = serde_json::Map::new();
        for r in reads_of_shape(shape) {
            match r.split_once('.') {
                Some(("at", k)) => drop(at.insert(k.into(), "x".into())),
                Some(("facts", k)) => drop(facts.insert(k.into(), "x".into())),
                _ => drop(obs.entry(r).or_insert(Value::from(0))),
            }
        }
        obs.insert("at".into(), Value::Object(at));
        obs.insert("facts".into(), Value::Object(facts));
        let obs = Value::Object(obs);
        let rules = rules_of(shape);
        let opened = step(&rules, &obs, &Value::Null);
        step(&rules, &obs, &opened)
    }

    #[test]
    fn state_carries_exactly_what_some_guard_compares_and_nothing_else() {
        for shape in ALL {
            let state = settled_state(shape);
            let top: BTreeSet<&str> = state
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect();
            assert_eq!(
                top,
                BTreeSet::from(["position", "baseline", "now", "v", "status"]),
                "`{}`: a field no guard reads still makes the state unequal, and \
                 `should_still` compares the whole state — so it writes a Transition \
                 with no bit lit. Facts that only inform a reader belong in the \
                 observation, which the journal already keeps",
                shape.name
            );

            let since: BTreeSet<&str> = shape
                .dims
                .iter()
                .filter_map(|d| match d.reads {
                    Reads::Since { field, .. } => Some(field),
                    Reads::Now { .. } => None,
                })
                .collect();
            for side in ["baseline", "now"] {
                let got: BTreeSet<&str> = state[side]
                    .as_object()
                    .unwrap_or_else(|| panic!("`{}`: {side} is not an object", shape.name))
                    .keys()
                    .map(String::as_str)
                    .collect();
                assert_eq!(got, since, "`{}`: {side}", shape.name);
            }

            let bits: BTreeSet<&str> = state["v"]
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect();
            assert_eq!(
                bits,
                shape.dims.iter().map(|d| d.name).collect::<BTreeSet<_>>(),
                "`{}`: one bit per dimension, no more and no fewer",
                shape.name
            );
        }
    }

    #[test]
    fn nothing_is_both_retired_and_shipping() {
        let live = vocabulary();
        for word in RETIRED {
            assert!(
                !live.contains(word),
                "`{word}` is on the tombstone list and also in this build's vocabulary; \
                 a note naming it would be reported as stale while being correct"
            );
        }
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

    struct Shot {
        shape: &'static str,
        axis: &'static str,
        moves: &'static [&'static str],
        probe: &'static str,
        file: &'static str,
        pos: &'static str,
        before: &'static str,
        after: &'static str,
    }

    const AST: &str = "ast-map";
    const PROSE: &str = "prose-map";

    const RANGE: &[Shot] = &[
        Shot {
            shape: "contract",
            axis: "sig",
            moves: &["sig"],
            probe: AST,
            file: "a.rs",
            pos: r#"{"file": "a.rs", "name": "f"}"#,
            before: "pub fn f(x: u64) -> u64 { x }",
            after: "pub fn f(x: u64, y: u64) -> u64 { x }",
        },
        Shot {
            shape: "contract",
            axis: "sig",
            moves: &["sig"],
            probe: AST,
            file: "a.rs",
            pos: r#"{"file": "a.rs", "name": "f"}"#,
            before: "pub fn f(x: u64) -> u64 { x }",
            after: "pub fn f(x: u64) -> u32 { x }",
        },
        Shot {
            shape: "contract",
            axis: "sig",
            moves: &["sig"],
            probe: AST,
            file: "a.rs",
            pos: r#"{"file": "a.rs", "name": "f"}"#,
            before: "pub fn f(x: u64) -> u64 { x }",
            after: "pub async fn f(x: u64) -> u64 { x }",
        },
        Shot {
            shape: "contract",
            axis: "sig",
            moves: &["sig"],
            probe: AST,
            file: "a.rs",
            pos: r#"{"file": "a.rs", "name": "f"}"#,
            before: "pub fn f(x: u64) -> u64 { x }",
            after: "pub unsafe fn f(x: u64) -> u64 { x }",
        },
        Shot {
            shape: "contract",
            axis: "sig",
            moves: &["sig"],
            probe: AST,
            file: "a.rs",
            pos: r#"{"file": "a.rs", "name": "f"}"#,
            before: "pub fn f<T>(x: T) -> T { x }",
            after: "pub fn f<T: Clone>(x: T) -> T { x }",
        },
        Shot {
            shape: "contract",
            axis: "surface",
            moves: &["surface"],
            probe: AST,
            file: "a.rs",
            pos: r#"{"file": "a.rs", "name": "f"}"#,
            before: "pub fn f(x: u64) -> u64 { x }",
            after: "fn f(x: u64) -> u64 { x }",
        },
        Shot {
            shape: "contract",
            axis: "logic",
            moves: &["logic"],
            probe: AST,
            file: "a.rs",
            pos: r#"{"file": "a.rs", "name": "f"}"#,
            before: "pub fn f(x: u64) -> u64 { helper(x) }",
            after: "pub fn f(x: u64) -> u64 { other(x) }",
        },
        Shot {
            shape: "contract",
            axis: "place",
            moves: &["place"],
            probe: AST,
            file: "a.rs",
            pos: r#"{"file": "a.rs", "name": "f"}"#,
            before: "pub fn a() {}\npub fn f(x: u64) -> u64 { x }",
            after: "pub fn a() {}\npub fn b() {}\npub fn f(x: u64) -> u64 { x }",
        },
        Shot {
            shape: "contract",
            axis: "surface",
            moves: &["surface"],
            probe: AST,
            file: "a.rs",
            pos: r#"{"file": "a.rs", "name": "f"}"#,
            before: "pub fn f(x: u64) -> u64 { x }",
            after: "#[deprecated]\npub fn f(x: u64) -> u64 { x }",
        },
        Shot {
            shape: "contract",
            axis: "missing",
            moves: &["missing"],
            probe: AST,
            file: "a.rs",
            pos: r#"{"file": "a.rs", "name": "f"}"#,
            before: "pub fn f(x: u64) -> u64 { x }",
            after: "pub fn gone(x: u64) -> u64 { x }",
        },
        Shot {
            shape: "contract",
            axis: "sig",
            moves: &["sig"],
            probe: AST,
            file: "a.rs",
            pos: r#"{"file": "a.rs", "name": "X"}"#,
            before: "pub struct X { pub a: u64 }",
            after: "pub struct X { pub a: u64, pub b: u8 }",
        },
        Shot {
            shape: "contract",
            axis: "sig",
            moves: &["sig"],
            probe: AST,
            file: "a.rs",
            pos: r#"{"file": "a.rs", "name": "X"}"#,
            before: "pub struct X { pub a: u64 }",
            after: "pub struct X { pub a: u32 }",
        },
        Shot {
            shape: "contract",
            axis: "logic",
            moves: &["logic"],
            probe: AST,
            file: "a.rs",
            pos: r#"{"file": "a.rs", "name": "X"}"#,
            before: "pub trait X { fn go(&self) -> u8 { 1 } }",
            after: "pub trait X { fn go(&self) -> u8 { 2 } }",
        },
        Shot {
            shape: "roster",
            axis: "missing",
            moves: &["missing"],
            probe: AST,
            file: "a.rs",
            pos: r#"{"file": "a.rs", "kind": "function"}"#,
            before: "pub fn a() {}",
            after: "pub struct A;",
        },
        Shot {
            shape: "roster",
            axis: "grew",
            moves: &["grew", "roll"],
            probe: AST,
            file: "a.rs",
            pos: r#"{"file": "a.rs", "kind": "function"}"#,
            before: "pub fn a() {}",
            after: "pub fn a() {}\npub fn b() {}",
        },
        Shot {
            shape: "roster",
            axis: "shrank",
            moves: &["shrank", "roll"],
            probe: AST,
            file: "a.rs",
            pos: r#"{"file": "a.rs", "kind": "function"}"#,
            before: "pub fn a() {}\npub fn b() {}",
            after: "pub fn a() {}",
        },
        Shot {
            shape: "roster",
            axis: "roll",
            moves: &["roll"],
            probe: AST,
            file: "a.rs",
            pos: r#"{"file": "a.rs", "kind": "function"}"#,
            before: "pub fn a() {}",
            after: "pub fn b() {}",
        },
        Shot {
            shape: "fingerprint",
            axis: "missing",
            moves: &["missing"],
            probe: PROSE,
            file: "a.md",
            pos: r#"{"file": "a.md", "heading": "H"}"#,
            before: "# H\n\nbody\n",
            after: "# Other\n\nbody\n",
        },
        Shot {
            shape: "fingerprint",
            axis: "drift",
            moves: &["drift"],
            probe: PROSE,
            file: "a.md",
            pos: r#"{"file": "a.md", "heading": "H"}"#,
            before: "# H\n\nbody\n",
            after: "# H\n\nrewritten\n",
        },
        Shot {
            shape: "contract",
            axis: "kind",
            moves: &["kind", "sig"],
            probe: AST,
            file: "a.rs",
            pos: r#"{"file": "a.rs", "name": "X"}"#,
            before: "pub struct X { pub a: u64 }",
            after: "pub enum X { A }",
        },
    ];

    fn fired(shot: &Shot, at: usize) -> Vec<String> {
        let dir = std::env::temp_dir().join(format!("gmr-range-{at}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join(shot.file);
        let reg = coding_extract::registry_uncached();
        let probe = &reg
            .get(&gmr::ProbeName::new(shot.probe))
            .unwrap_or_else(|| panic!("`{}` is not linked in", shot.probe))
            .extract;
        let pos: Value = serde_json::from_str(shot.pos).unwrap();
        let look = |src: &str| {
            std::fs::write(&file, src).unwrap();
            probe(&gmr_transport::inproc::Reach {
                cwd: dir.clone(),
                position: pos.clone(),
                params: serde_json::json!({}),
                budget: gmr_transport::inproc::Budget::within(
                    std::time::Duration::from_secs(30),
                    1 << 20,
                ),
            })
            .unwrap()
        };

        let rules = rules_of(get(shot.shape).unwrap());
        let opened = step(&rules, &look(shot.before), &Value::Null);
        assert!(
            set(&opened).is_empty(),
            "shot {at} did not open clean: {opened}"
        );
        set(&step(&rules, &look(shot.after), &opened))
    }

    #[test]
    fn what_an_axis_answers_decides_when_its_bit_falls() {
        for d in get("contract").unwrap().dims {
            let carries = bit(d).contains(&format!("state.v.{}", d.name));
            match d.reads {
                Reads::Now { .. } => assert!(
                    !carries,
                    "`{}` answers about now, so carrying its own last bit would \
                     keep it up after the condition stopped holding",
                    d.name
                ),
                Reads::Since { .. } => assert!(
                    carries,
                    "`{}` answers about drift since you confirmed, so dropping \
                     its own last bit hands the signal to whoever observed first",
                    d.name
                ),
            }
        }
    }

    #[test]
    fn every_axis_can_be_moved_and_moves_alone() {
        for (at, shot) in RANGE.iter().enumerate() {
            assert_eq!(
                fired(shot, at),
                shot.moves,
                "shot {at}: `{}` -> `{}`",
                shot.before,
                shot.after
            );
        }
    }

    #[test]
    fn no_axis_of_any_shape_is_left_off_the_range() {
        for shape in ALL {
            for axis in axes_of(shape) {
                assert!(
                    RANGE
                        .iter()
                        .any(|s| s.shape == shape.name && s.axis == axis),
                    "`{}`'s `{axis}` has no shot; an axis nothing is known to move \
                     is how a dead one hides",
                    shape.name
                );
            }
        }
    }

    fn sighted(sig: &str, body: &str, file: &str, line: i64) -> Value {
        serde_json::json!({
            "schema": COORD_SCHEMA, "extractor": "ast-map", "found": true,
            "matched": ["file", "name"], "missed": [],
            "at": {
                "file": file, "kind": "function", "form": "function_item",
                "vis": "pub", "surface": "pub", "after": "", "name": "f", "shape": sig,
            },
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
            "absent",
            "missing",
            "kind-changed",
            "signature-changed",
            "surface-changed",
            "logic-changed",
            "moved",
            "settled",
        ];
        assert_eq!(rules.len(), statuses.len() + 1);
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
            &[
                "file", "kind", "form", "vis", "surface", "after", "name", "shape",
            ],
            &["body", "line"],
        );
        assert!(unmet(&reads, &ast_map).is_empty());

        let name_map = obs(COORD_SCHEMA, &["name", "scope"], &["occurrences"]);
        assert_eq!(
            unmet(&reads, &name_map),
            [
                "at.after",
                "at.form",
                "at.shape",
                "at.surface",
                "facts.body"
            ]
        );
    }

    #[test]
    fn three_changes_at_once_all_land() {
        let r = contract();
        let s = step(&r, &sighted("(a) -> B", "body1", "a.rs", 1), &Value::Null);
        assert_eq!(s["status"], "settled");
        assert!(set(&s).is_empty(), "a first sighting drifts from nothing");

        let s = step(&r, &sighted("(a, b) -> B", "body2", "a.rs", 9), &s);
        assert_eq!(set(&s), ["sig", "logic"]);
        assert_eq!(s["status"], "signature-changed");
    }

    fn tweak(mut sighting: Value, key: &str, to: &str) -> Value {
        sighting["at"][key] = Value::String(to.to_owned());
        sighting
    }

    #[test]
    fn narrowing_the_public_surface_is_its_own_axis() {
        let r = contract();
        let seen = || sighted("(a) -> B", "body1", "a.rs", 1);
        let s = step(&r, &seen(), &Value::Null);
        let s = step(&r, &tweak(seen(), "surface", ""), &s);
        assert_eq!(
            set(&s),
            ["surface"],
            "pub -> private moves nothing else, and used to move nothing at all"
        );
        assert_eq!(s["status"], "surface-changed");
    }

    #[test]
    fn a_struct_that_becomes_an_enum_is_not_a_changed_implementation() {
        let r = contract();
        let was = tweak(sighted("a: u64", "", "a.rs", 1), "form", "struct_item");
        let now = tweak(sighted("A; B", "", "a.rs", 1), "form", "enum_item");
        let s = step(&r, &was, &Value::Null);
        let s = step(&r, &now, &s);
        assert_eq!(s["status"], "kind-changed");
        assert_eq!(set(&s), ["kind", "sig"]);
    }

    #[test]
    fn every_axis_hands_the_memory_back_unless_the_note_narrows_it() {
        let shape = get("contract").unwrap();
        let watch = watch_of(shape);
        assert_eq!(
            watch,
            axes_of(shape),
            "an axis worth accumulating is an axis worth reporting; a note that \
             wants less says so itself"
        );
    }

    #[test]
    fn the_vector_is_ordered_by_priority() {
        let s = step(
            &contract(),
            &sighted("(a) -> B", "body1", "a.rs", 1),
            &Value::Null,
        );
        let axes: Vec<String> = bits(&s).into_iter().map(|(k, _)| k).collect();
        assert_eq!(
            axes,
            ["missing", "kind", "sig", "surface", "logic", "place"]
        );
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
            "writing a null baseline would strand this anchor in `absent`, got {s}"
        );

        let s = step(&r, &sighted("(a) -> B", "body1", "a.rs", 1), &s);
        assert_eq!(s["status"], "settled");
        assert_eq!(s["baseline"], s["now"]);
        assert!(
            set(&s).is_empty(),
            "the first real sighting is the baseline"
        );
    }

    #[test]
    fn a_near_miss_is_not_the_target() {
        let r = contract();
        let s = step(&r, &sighted("(a) -> B", "body1", "a.rs", 1), &Value::Null);

        let mut renamed = sighted("(z) -> Q", "otherbody", "a.rs", 40);
        renamed["exact"] = Value::Bool(false);
        renamed["matched"] = serde_json::json!(["file"]);
        renamed["missed"] = serde_json::json!(["name"]);
        renamed["at"]["name"] = Value::String("g".into());

        let s = step(&r, &renamed, &s);
        assert_eq!(s["status"], "missing");
        assert_eq!(set(&s), ["missing"]);
        assert_eq!(
            s["now"]["sig"], "(a) -> B",
            "another object's reading must not become this anchor's"
        );
    }

    #[test]
    fn a_first_sighting_settles_rather_than_announcing_itself() {
        let r = contract();
        let first = step(&r, &sighted("(a) -> B", "body1", "a.rs", 1), &Value::Null);
        let again = step(&r, &sighted("(a) -> B", "body1", "a.rs", 1), &first);
        assert_eq!(
            first, again,
            "a distinct opening status would transition into settled on the very next \
             observation, handing back the memory for a change nobody made"
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

    fn fingerprint() -> Vec<String> {
        rules_of(get("fingerprint").unwrap())
    }

    fn section(heading: &str, print: &str, line: i64, exact: bool) -> Value {
        serde_json::json!({
            "schema": COORD_SCHEMA, "extractor": "prose-map", "found": true,
            "matched": if exact { vec!["file", "heading"] } else { vec!["file"] },
            "missed": if exact { vec![] } else { vec!["heading"] },
            "at": { "file": "CLAUDE.md", "heading": heading, "fingerprint": print },
            "facts": { "line": line, "lines": 12 },
            "candidates": 1, "matches": [], "exact": exact,
        })
    }

    #[test]
    fn a_fingerprint_never_captures_a_section_it_did_not_actually_match() {
        let r = fingerprint();
        let fell_back = section("一、这十三条", "bac58fed", 7, false);

        let first = step(&r, &fell_back, &Value::Null);
        assert_eq!(first["status"], "absent", "a miss is not a baseline");
        assert!(
            first.get("baseline").is_none(),
            "nothing may be pinned from a fallback: {first}"
        );

        assert_eq!(
            step(&r, &fell_back, &first),
            first,
            "and it stays absent rather than settling into the wrong section"
        );
    }

    #[test]
    fn a_fingerprint_that_matched_captures_and_still_notices_the_heading_leaving() {
        let r = fingerprint();
        let captured = step(&r, &section("四、红牌", "aaa", 40, true), &Value::Null);
        assert_eq!(captured["status"], "settled");
        assert_eq!(captured["baseline"]["fingerprint"], "aaa");

        let after = step(
            &r,
            &section("一、这十三条", "bac58fed", 7, false),
            &captured,
        );
        assert_eq!(after["status"], "missing");
        assert_eq!(
            after["baseline"]["fingerprint"], "aaa",
            "the baseline survives; the fallback must not overwrite it"
        );
    }
}
