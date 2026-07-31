use serde_json::Value;

use crate::ast::{BinOp, Node, Root, Step};
use crate::ctx::Ctx;
use crate::parse::json_number;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    NoSuchField,
    NotAnObject,
    NotAnArray,
    IndexOutOfRange,
    NotComparable,
    DividedByZero,
}

impl Fault {
    pub fn class(self) -> &'static str {
        match self {
            Self::NoSuchField => "no_such_field",
            Self::NotAnObject => "not_an_object",
            Self::NotAnArray => "not_an_array",
            Self::IndexOutOfRange => "index_out_of_range",
            Self::NotComparable => "not_comparable",
            Self::DividedByZero => "divided_by_zero",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Evaluated {
    Value(Value),
    Absent,
    Fault(Fault),
}

impl Evaluated {
    fn bool(b: bool) -> Self {
        Self::Value(Value::Bool(b))
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Value(Value::Bool(b)) => Some(*b),
            _ => None,
        }
    }
}

pub fn eval(node: &Node, ctx: Ctx<'_>) -> Evaluated {
    match node {
        Node::Lit(v) => Evaluated::Value(v.clone()),
        Node::Changed(name) => Evaluated::bool(ctx.changed(name)),
        Node::Path(p) => walk(
            root_value(p.root, &ctx),
            &p.steps,
            matches!(p.root, Root::State),
        ),
        Node::Exists(inner) => Evaluated::bool(matches!(eval(inner, ctx), Evaluated::Value(_))),
        Node::Not(inner) => match eval(inner, ctx) {
            Evaluated::Value(Value::Bool(b)) => Evaluated::bool(!b),
            Evaluated::Value(_) => Evaluated::Fault(Fault::NotComparable),
            other => other,
        },
        Node::Neg(inner) => match eval(inner, ctx) {
            Evaluated::Value(v) => match v.as_f64() {
                Some(n) => Evaluated::Value(json_number(-n)),
                None => Evaluated::Fault(Fault::NotComparable),
            },
            other => other,
        },
        Node::Binary { op, lhs, rhs } => binary(*op, lhs, rhs, ctx),
        Node::Object(fields) => object(fields, ctx),
        Node::Array(items) => array(items, ctx),
    }
}

fn array(items: &[Node], ctx: Ctx<'_>) -> Evaluated {
    let mut out = Vec::with_capacity(items.len());
    for node in items {
        match eval(node, ctx) {
            Evaluated::Value(v) => out.push(v),
            Evaluated::Absent => out.push(Value::Null),
            fault @ Evaluated::Fault(_) => return fault,
        }
    }
    Evaluated::Value(Value::Array(out))
}

fn object(fields: &[(String, Node)], ctx: Ctx<'_>) -> Evaluated {
    let mut map = serde_json::Map::with_capacity(fields.len());
    for (name, node) in fields {
        match eval(node, ctx) {
            Evaluated::Value(v) => {
                map.insert(name.clone(), v);
            }
            Evaluated::Absent => {
                map.insert(name.clone(), Value::Null);
            }
            fault @ Evaluated::Fault(_) => return fault,
        }
    }
    Evaluated::Value(Value::Object(map))
}

fn root_value(root: Root, ctx: &Ctx<'_>) -> Evaluated {
    match root {
        Root::Obs => value_or_absent(ctx.obs),
        Root::State => value_or_absent(ctx.state),
        Root::TakenAt => Evaluated::Value(Value::from(ctx.taken_at)),
        Root::EnteredAt => Evaluated::Value(Value::from(ctx.entered_at)),
    }
}

fn value_or_absent(v: &Value) -> Evaluated {
    if v.is_null() {
        Evaluated::Absent
    } else {
        Evaluated::Value(v.clone())
    }
}

fn walk(start: Evaluated, steps: &[Step], lenient: bool) -> Evaluated {
    let Evaluated::Value(mut cur) = start else {
        return if steps.is_empty() {
            start
        } else {
            Evaluated::Absent
        };
    };

    for step in steps {
        if cur.is_null() {
            return Evaluated::Absent;
        }
        cur = match step {
            Step::Field(name) => match &cur {
                Value::Object(map) => match map.get(name) {
                    Some(v) => v.clone(),
                    None if lenient => return Evaluated::Absent,
                    None => return Evaluated::Fault(Fault::NoSuchField),
                },
                _ => return Evaluated::Fault(Fault::NotAnObject),
            },
            Step::Index(i) => match &cur {
                Value::Array(items) => match items.get(*i) {
                    Some(v) => v.clone(),
                    None => return Evaluated::Fault(Fault::IndexOutOfRange),
                },
                _ => return Evaluated::Fault(Fault::NotAnArray),
            },
        };
    }

    value_or_absent(&cur)
}

fn binary(op: BinOp, lhs: &Node, rhs: &Node, ctx: Ctx<'_>) -> Evaluated {
    if matches!(op, BinOp::And | BinOp::Or) {
        return logic(op, lhs, rhs, ctx);
    }

    let (a, b) = match (eval(lhs, ctx), eval(rhs, ctx)) {
        (Evaluated::Fault(f), _) | (_, Evaluated::Fault(f)) => return Evaluated::Fault(f),
        (Evaluated::Absent, Evaluated::Absent) => return equality(op, true),
        (Evaluated::Absent, _) | (_, Evaluated::Absent) => return equality(op, false),
        (Evaluated::Value(a), Evaluated::Value(b)) => (a, b),
    };

    match op {
        BinOp::Eq => Evaluated::bool(a == b),
        BinOp::Ne => Evaluated::bool(a != b),
        BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => match order(&a, &b) {
            None => Evaluated::Fault(Fault::NotComparable),
            Some(o) => Evaluated::bool(match op {
                BinOp::Lt => o.is_lt(),
                BinOp::Le => o.is_le(),
                BinOp::Gt => o.is_gt(),
                _ => o.is_ge(),
            }),
        },
        _ => arithmetic(op, &a, &b),
    }
}

fn equality(op: BinOp, both_absent: bool) -> Evaluated {
    match op {
        BinOp::Eq => Evaluated::bool(both_absent),
        BinOp::Ne => Evaluated::bool(!both_absent),
        _ => Evaluated::Absent,
    }
}

fn logic(op: BinOp, lhs: &Node, rhs: &Node, ctx: Ctx<'_>) -> Evaluated {
    let left = eval(lhs, ctx);
    match (op, left.as_bool()) {
        (BinOp::And, Some(false)) => return Evaluated::bool(false),
        (BinOp::Or, Some(true)) => return Evaluated::bool(true),
        (_, Some(_)) => {}
        (_, None) => return left,
    }
    match eval(rhs, ctx) {
        right @ Evaluated::Value(Value::Bool(_)) => right,
        Evaluated::Value(_) => Evaluated::Fault(Fault::NotComparable),
        other => other,
    }
}

fn order(a: &Value, b: &Value) -> Option<std::cmp::Ordering> {
    match (a, b) {
        (Value::Number(_), Value::Number(_)) => a.as_f64()?.partial_cmp(&b.as_f64()?),
        (Value::String(x), Value::String(y)) => Some(x.cmp(y)),
        _ => None,
    }
}

fn arithmetic(op: BinOp, a: &Value, b: &Value) -> Evaluated {
    let (Some(x), Some(y)) = (a.as_f64(), b.as_f64()) else {
        return Evaluated::Fault(Fault::NotComparable);
    };
    let v = match op {
        BinOp::Add => x + y,
        BinOp::Sub => x - y,
        BinOp::Mul => x * y,
        _ if y == 0.0 => return Evaluated::Fault(Fault::DividedByZero),
        _ => x / y,
    };
    Evaluated::Value(json_number(v))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;
    use serde_json::json;

    fn run(src: &str, obs: Value) -> Evaluated {
        let state = json!({});
        eval(&parse(src).unwrap(), Ctx::new(&obs, &state))
    }

    fn truth(src: &str, obs: Value) -> Option<bool> {
        run(src, obs).as_bool()
    }

    fn with_state(src: &str, obs: Value, state: Value) -> Evaluated {
        eval(&parse(src).unwrap(), Ctx::new(&obs, &state))
    }

    #[test]
    fn bare_root_is_the_whole_fact() {
        assert_eq!(
            run("obs", json!({ "a": 1 })),
            Evaluated::Value(json!({ "a": 1 }))
        );
    }

    #[test]
    fn nested_fields_and_indices() {
        let obs = json!({ "params": [{ "type": "u8" }, { "type": "u16" }] });
        assert_eq!(
            run("obs.params[1].type", obs),
            Evaluated::Value(json!("u16"))
        );
    }

    #[test]
    fn the_state_is_readable_and_the_substrate_does_not_interpret_it() {
        assert_eq!(
            with_state("state.position", json!({}), json!({ "position": "a.rs" })),
            Evaluated::Value(json!("a.rs"))
        );
    }

    #[test]
    fn a_missing_state_field_is_absence_but_a_missing_obs_field_is_a_fault() {
        assert_eq!(
            with_state("state.shape", json!({}), json!({})),
            Evaluated::Absent
        );
        assert_eq!(
            run("obs.shape", json!({})),
            Evaluated::Fault(Fault::NoSuchField)
        );
    }

    #[test]
    fn null_is_the_worlds_absence_but_a_missing_key_is_our_failure() {
        assert_eq!(
            run("obs.deprecation", json!({ "deprecation": null })),
            Evaluated::Absent
        );
        assert_eq!(
            run("obs.deprecation", json!({})),
            Evaluated::Fault(Fault::NoSuchField)
        );
    }

    #[test]
    fn null_partway_down_is_absent_not_a_fault() {
        assert_eq!(run("obs.a.b.c", json!({ "a": null })), Evaluated::Absent);
    }

    #[test]
    fn shape_mismatches_are_faults() {
        assert_eq!(
            run("obs.a.b", json!({ "a": 1 })),
            Evaluated::Fault(Fault::NotAnObject)
        );
        assert_eq!(
            run("obs.a[0]", json!({ "a": { "x": 1 } })),
            Evaluated::Fault(Fault::NotAnArray)
        );
        assert_eq!(
            run("obs.a[5]", json!({ "a": [1] })),
            Evaluated::Fault(Fault::IndexOutOfRange)
        );
    }

    #[test]
    fn comparison_and_arithmetic() {
        assert_eq!(truth("obs.a > 1", json!({ "a": 2 })), Some(true));
        assert_eq!(truth("obs.a * 2 == 4", json!({ "a": 2 })), Some(true));
        assert_eq!(truth("obs.a == \"x\"", json!({ "a": "x" })), Some(true));
        assert_eq!(truth("not (obs.a > 1)", json!({ "a": 0 })), Some(true));
    }

    #[test]
    fn absence_propagates_through_operators() {
        assert_eq!(run("obs.a > 5", json!({ "a": null })), Evaluated::Absent);
        assert_eq!(run("obs.a + 1", json!({ "a": null })), Evaluated::Absent);
        assert_eq!(
            truth("obs.a == obs.b", json!({ "a": null, "b": null })),
            Some(true)
        );
        assert_eq!(
            truth("obs.a == obs.b", json!({ "a": null, "b": 1 })),
            Some(false)
        );
    }

    #[test]
    fn faults_propagate_and_only_exists_digests_them() {
        assert_eq!(
            run("obs.a > 5", json!({})),
            Evaluated::Fault(Fault::NoSuchField)
        );
        assert_eq!(
            run("obs.a == 5", json!({})),
            Evaluated::Fault(Fault::NoSuchField)
        );
        assert_eq!(truth("exists(obs.a)", json!({})), Some(false));
        assert_eq!(truth("exists(obs.a)", json!({ "a": null })), Some(false));
        assert_eq!(truth("exists(obs.a)", json!({ "a": 1 })), Some(true));
    }

    #[test]
    fn short_circuit_keeps_a_decided_side() {
        assert_eq!(truth("false and obs.missing > 1", json!({})), Some(false));
        assert_eq!(truth("true or obs.missing > 1", json!({})), Some(true));
        assert_eq!(
            run("true and obs.missing > 1", json!({})),
            Evaluated::Fault(Fault::NoSuchField)
        );
    }

    #[test]
    fn mismatched_types_do_not_guess() {
        assert_eq!(
            run("obs.a > 1", json!({ "a": "x" })),
            Evaluated::Fault(Fault::NotComparable)
        );
        assert_eq!(
            run("obs.a / 0", json!({ "a": 1 })),
            Evaluated::Fault(Fault::DividedByZero)
        );
    }

    #[test]
    fn time_based_hysteresis_flips_in_a_still_world() {
        let (obs, state) = (json!({}), json!({}));
        let node = parse("taken_at - entered_at > 30d").unwrap();

        let ctx = Ctx::new(&obs, &state).at(1_000_000, 1_000_000);
        assert_eq!(eval(&node, ctx).as_bool(), Some(false));

        let ctx = Ctx::new(&obs, &state).at(1_000_000 + 2_592_001, 1_000_000);
        assert_eq!(
            eval(&node, ctx).as_bool(),
            Some(true),
            "世界静止，滞回照样翻转"
        );
    }

    #[test]
    fn the_state_is_where_cross_observation_memory_lives() {
        let node = parse("(state.close - obs.close) / state.close > 0.05").unwrap();
        let (obs, state) = (json!({ "close": 90 }), json!({ "close": 100 }));
        assert_eq!(eval(&node, Ctx::new(&obs, &state)).as_bool(), Some(true));
    }

    #[test]
    fn counting_is_the_domains_job_and_it_is_expressible() {
        let node = parse("{ count: state.count + 1, status: \"drifted\" }").unwrap();
        let (obs, state) = (json!({}), json!({ "count": 2 }));
        assert_eq!(
            eval(&node, Ctx::new(&obs, &state)),
            Evaluated::Value(json!({ "count": 3, "status": "drifted" }))
        );
    }

    #[test]
    fn changed_is_sugar_for_comparing_obs_against_state() {
        let node = parse("changed(\"shape\")").unwrap();

        let (obs, state) = (json!({ "shape": "(a,b)->c" }), json!({ "shape": "(a)->c" }));
        assert_eq!(eval(&node, Ctx::new(&obs, &state)).as_bool(), Some(true));

        let (obs, state) = (json!({ "shape": "(a)->c" }), json!({ "shape": "(a)->c" }));
        assert_eq!(eval(&node, Ctx::new(&obs, &state)).as_bool(), Some(false));
    }

    #[test]
    fn a_direction_the_domain_never_stored_reads_as_changed() {
        let (obs, state) = (json!({ "shape": "(a)->c" }), json!({}));
        assert_eq!(
            eval(
                &parse("changed(\"shape\")").unwrap(),
                Ctx::new(&obs, &state)
            )
            .as_bool(),
            Some(true)
        );
    }

    #[test]
    fn arrays_build_and_treat_absence_and_faults_like_objects_do() {
        assert_eq!(run("[]", json!({})), Evaluated::Value(json!([])));
        assert_eq!(
            run("{ names: [], n: 0 }", json!({})),
            Evaluated::Value(json!({ "names": [], "n": 0 }))
        );
        assert_eq!(
            run("[obs.a, 2]", json!({ "a": 1 })),
            Evaluated::Value(json!([1, 2]))
        );
        assert_eq!(
            with_state("[state.nope]", json!({}), json!({})),
            Evaluated::Value(json!([null]))
        );
        assert_eq!(
            run("[obs.missing]", json!({})),
            Evaluated::Fault(Fault::NoSuchField)
        );
    }

    #[test]
    fn object_construction_builds_the_next_state() {
        let node = parse("{ position: obs.at, status: \"ok\" }").unwrap();
        let (obs, state) = (json!({ "at": "b.rs" }), json!({}));
        assert_eq!(
            eval(&node, Ctx::new(&obs, &state)),
            Evaluated::Value(json!({ "position": "b.rs", "status": "ok" }))
        );
    }

    #[test]
    fn partial_acceptance_is_written_out_in_the_open() {
        let node = parse("{ position: obs.position, shape: state.shape }").unwrap();
        let obs = json!({ "position": "b.rs", "shape": "(a,b)->c" });
        let state = json!({ "position": "a.rs", "shape": "(a)->c" });
        assert_eq!(
            eval(&node, Ctx::new(&obs, &state)),
            Evaluated::Value(json!({ "position": "b.rs", "shape": "(a)->c" }))
        );
    }

    #[test]
    fn absence_lands_in_the_state_as_null_but_a_fault_kills_the_whole_object() {
        let node = parse("{ x: obs.x }").unwrap();
        let (obs, state) = (json!({ "x": null }), json!({}));
        assert_eq!(
            eval(&node, Ctx::new(&obs, &state)),
            Evaluated::Value(json!({ "x": null }))
        );

        let (obs, state) = (json!({}), json!({}));
        assert_eq!(
            eval(&node, Ctx::new(&obs, &state)),
            Evaluated::Fault(Fault::NoSuchField)
        );
    }

    #[test]
    fn evaluation_is_deterministic() {
        let obs = json!({ "x": [1, 2, 3] });
        assert_eq!(run("obs.x[1]", obs.clone()), run("obs.x[1]", obs));
    }
}
