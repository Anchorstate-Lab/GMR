use chrono::{DateTime, Utc};
use gmr_core::{Anchor, Observation, State};
use gmr_expr::{Ctx, Evaluated, Node};
use serde_json::Value;

pub(crate) enum Transitioned {
    To(State),
    Unchanged,
    Unevaluable(String),
}

pub(crate) fn compile(expr: &gmr_core::Expr) -> Result<Node, String> {
    let source = expr
        .source
        .as_str()
        .ok_or_else(|| "表达式不是一段文本".to_owned())?;
    gmr_expr::parse(source).map_err(|e| e.to_string())
}

pub(crate) fn transition(
    anchor: &Anchor,
    observation: &Observation,
    state: &State,
    at: DateTime<Utc>,
    entered_at: DateTime<Utc>,
) -> Transitioned {
    let empty = Value::Null;
    let obs = observation.facts().map(|f| f.as_value()).unwrap_or(&empty);
    let ctx = Ctx::new(obs, state.as_value()).at(at.timestamp(), entered_at.timestamp());

    for (i, rule) in anchor.transitions.iter().enumerate() {
        let n = i + 1;
        let guard = match compile(&rule.when) {
            Ok(node) => node,
            Err(e) => return Transitioned::Unevaluable(format!("第 {n} 条规则的守卫：{e}")),
        };

        match gmr_expr::eval(&guard, ctx) {
            Evaluated::Value(Value::Bool(false)) => continue,
            Evaluated::Absent => continue,
            Evaluated::Value(Value::Bool(true)) => {}
            Evaluated::Value(_) => {
                return Transitioned::Unevaluable(format!("第 {n} 条规则的守卫算出来不是真假值"));
            }
            Evaluated::Fault(f) => {
                return Transitioned::Unevaluable(format!("第 {n} 条规则的守卫：{}", f.class()));
            }
        }

        let body = match compile(&rule.to) {
            Ok(node) => node,
            Err(e) => return Transitioned::Unevaluable(format!("第 {n} 条规则的新状态：{e}")),
        };
        return match gmr_expr::eval(&body, ctx) {
            Evaluated::Value(v @ Value::Object(_)) => Transitioned::To(State::new(v)),
            Evaluated::Value(_) => {
                Transitioned::Unevaluable(format!("第 {n} 条规则的新状态不是一个对象"))
            }
            Evaluated::Absent => Transitioned::Unevaluable(format!("第 {n} 条规则算不出新状态")),
            Evaluated::Fault(f) => {
                Transitioned::Unevaluable(format!("第 {n} 条规则的新状态：{}", f.class()))
            }
        };
    }

    Transitioned::Unchanged
}

pub(crate) fn bind_warnings(anchor: &Anchor, observation: &Observation) -> Vec<String> {
    let empty = Value::Null;
    let obs = observation.facts().map(|f| f.as_value()).unwrap_or(&empty);
    let state = Value::Object(serde_json::Map::new());
    let ctx = Ctx::new(obs, &state);

    let mut out = Vec::new();
    for (i, rule) in anchor.transitions.iter().enumerate() {
        for (what, expr) in [("守卫", &rule.when), ("新状态", &rule.to)] {
            match compile(expr) {
                Err(e) => out.push(format!("第 {} 条规则的{what}不合法：{e}", i + 1)),
                Ok(node) => {
                    if let Some(w) = gmr_expr::bind(&node, ctx) {
                        out.push(format!("第 {} 条规则的{what}：{w}", i + 1));
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use gmr_core::{
        AnchorKey, Expr, Facts, Kind, Outcome, Probe, ProbeVersion, Retain, Rule, StatusId,
        Transitions, Versions,
    };
    use serde_json::json;

    fn rules(pairs: &[(&str, &str)]) -> Transitions {
        Transitions(
            pairs
                .iter()
                .map(|(w, t)| Rule {
                    when: Expr::text(*w),
                    to: Expr::text(*t),
                })
                .collect(),
        )
    }

    fn anchor(t: Transitions) -> Anchor {
        Anchor {
            key: AnchorKey::new("a"),
            probe: Probe::new(Kind::new("shell"), json!({ "run": "x" })),
            transitions: t,
            terminal: [StatusId::new("settled")].into_iter().collect(),
            retain: Retain::Tick,
            cadence_secs: None,
        }
    }

    fn seen(facts: Value) -> Observation {
        Observation {
            outcome: Outcome::Found {
                facts: Facts::new(facts),
            },
            fact_address: None,
            versions: Versions {
                probe: ProbeVersion::new("a".repeat(64)),
                evaluator: "eval-1".into(),
            },
        }
    }

    fn at(n: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(1_700_000_000 + n, 0).unwrap()
    }

    fn run(t: Transitions, facts: Value, state: Value) -> Transitioned {
        transition(&anchor(t), &seen(facts), &State::new(state), at(100), at(0))
    }

    fn to(r: Transitioned) -> Value {
        match r {
            Transitioned::To(s) => s.as_value().clone(),
            Transitioned::Unchanged => panic!("以为会转换"),
            Transitioned::Unevaluable(e) => panic!("算不出来：{e}"),
        }
    }

    #[test]
    fn the_first_matching_rule_wins() {
        let t = rules(&[
            ("obs.n > 10", "{ status: \"big\" }"),
            ("obs.n > 1", "{ status: \"small\" }"),
        ]);
        assert_eq!(
            to(run(t, json!({ "n": 50 }), json!({}))),
            json!({ "status": "big" })
        );
    }

    #[test]
    fn later_rules_still_get_their_turn() {
        let t = rules(&[
            ("obs.n > 10", "{ status: \"big\" }"),
            ("obs.n > 1", "{ status: \"small\" }"),
        ]);
        assert_eq!(
            to(run(t, json!({ "n": 5 }), json!({}))),
            json!({ "status": "small" })
        );
    }

    #[test]
    fn no_rule_matching_is_normal_not_a_failure() {
        let t = rules(&[("obs.n > 10", "{ status: \"big\" }")]);
        assert!(matches!(
            run(t, json!({ "n": 1 }), json!({})),
            Transitioned::Unchanged
        ));
    }

    #[test]
    fn an_undecidable_guard_skips_its_rule_rather_than_failing() {
        let t = rules(&[
            ("obs.ts > 100", "{ status: \"due\" }"),
            ("true", "{ status: \"waiting\" }"),
        ]);
        assert_eq!(
            to(run(t, json!({ "ts": null }), json!({}))),
            json!({ "status": "waiting" })
        );
    }

    #[test]
    fn a_faulting_guard_is_our_failure_and_must_be_loud() {
        let t = rules(&[("obs.gone > 1", "{ status: \"x\" }")]);
        let Transitioned::Unevaluable(e) = run(t, json!({ "here": 1 }), json!({})) else {
            panic!("该出声")
        };
        assert!(e.contains("no_such_field"), "{e}");
    }

    #[test]
    fn a_guard_that_is_not_a_predicate_is_a_failure() {
        let t = rules(&[("obs.n", "{ status: \"x\" }")]);
        assert!(matches!(
            run(t, json!({ "n": 5 }), json!({})),
            Transitioned::Unevaluable(_)
        ));
    }

    #[test]
    fn a_new_state_must_be_an_object() {
        let t = rules(&[("true", "42")]);
        assert!(matches!(
            run(t, json!({}), json!({})),
            Transitioned::Unevaluable(_)
        ));
    }

    #[test]
    fn the_state_carries_the_domains_own_accumulator() {
        let t = rules(&[("changed(\"shape\")", "{ shape: obs.shape, n: state.n + 1 }")]);
        assert_eq!(
            to(run(
                t,
                json!({ "shape": "(a,b)->c" }),
                json!({ "shape": "(a)->c", "n": 2 })
            )),
            json!({ "shape": "(a,b)->c", "n": 3 })
        );
    }

    #[test]
    fn time_is_readable_and_never_comes_from_a_clock() {
        let t = rules(&[("taken_at - entered_at > 30d", "{ status: \"stale\" }")]);
        let long = transition(
            &anchor(t.clone()),
            &seen(json!({})),
            &State::default(),
            at(2_592_001),
            at(0),
        );
        assert_eq!(to(long), json!({ "status": "stale" }));

        let short = transition(
            &anchor(t),
            &seen(json!({})),
            &State::default(),
            at(10),
            at(0),
        );
        assert!(matches!(short, Transitioned::Unchanged));
    }

    #[test]
    fn binding_warns_about_a_typo_without_refusing() {
        let a = anchor(rules(&[("obs.signatur == \"x\"", "{ status: \"y\" }")]));
        let w = bind_warnings(&a, &seen(json!({ "signature": "x" })));
        assert_eq!(w.len(), 1);
        assert!(w[0].contains("obs.signatur"), "{}", w[0]);
    }

    #[test]
    fn binding_stays_quiet_about_the_state_being_empty_at_open() {
        let a = anchor(rules(&[("state.n > 3", "{ n: state.n + 1 }")]));
        assert!(bind_warnings(&a, &seen(json!({}))).is_empty());
    }
}
