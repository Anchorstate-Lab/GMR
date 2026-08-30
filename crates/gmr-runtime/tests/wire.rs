use std::time::Duration;

use gmr_runtime::{Instructions, Policy};

#[test]
fn an_instruction_states_its_span_in_milliseconds_and_says_so_in_the_name() {
    let asked: Instructions = serde_json::from_str(r#"{"max_staleness_ms": 60000}"#).unwrap();
    assert_eq!(
        asked,
        Instructions {
            max_staleness: Some(Duration::from_secs(60)),
            budget: None,
            reach: None,
        },
        "a Duration's own serde shape is {{secs, nanos}}, which no caller outside Rust \
         would write and none should have to read. The unit is in the name because a bare \
         number cannot carry one, and `Policy` had already settled on that spelling"
    );

    assert_eq!(
        serde_json::to_string(&asked).unwrap(),
        r#"{"max_staleness_ms":60000}"#,
        "and a span nobody asked for is absent rather than null: an instruction says what \
         it wants bounded, and silence is how it says the rest is unbounded"
    );
    assert_eq!(
        serde_json::to_string(&Instructions::default()).unwrap(),
        "{}",
        "so the default instruction is the empty object"
    );
}

#[test]
fn an_instruction_nobody_here_understands_is_refused_and_never_dropped() {
    let camel = serde_json::from_str::<Instructions>(r#"{"maxStaleness": 60000}"#)
        .expect_err("a field this side does not know is not a field to skip");
    assert!(
        camel.to_string().contains("maxStaleness"),
        "an instruction silently dropped is an answer served stale under a freshness bound \
         the caller believes they set. It has to be an error, and the error has to name \
         what it could not place: {camel}"
    );

    assert!(
        serde_json::from_str::<Instructions>(r#"{"max_staleness_ms": 60000, "budget_ms": 250}"#)
            .is_ok(),
        "both spans, spelled the one way, are accepted"
    );
}

#[test]
fn a_policy_states_what_it_means_to_change_and_inherits_the_rest() {
    let tuned: Policy = serde_json::from_str(r#"{"probe_budget_ms": 1500}"#).unwrap();
    assert_eq!(tuned.probe_budget_ms, 1500);
    assert_eq!(
        tuned.content_total_ms,
        Policy::default().content_total_ms,
        "a caller who names one bound is not thereby resetting eleven others to zero"
    );

    let back: Policy =
        serde_json::from_str(&serde_json::to_string(&Policy::default()).unwrap()).unwrap();
    assert_eq!(
        back.batch,
        Policy::default().batch,
        "and a policy survives the round trip whole"
    );

    let typo = serde_json::from_str::<Policy>(r#"{"probe_budget": 1500}"#)
        .expect_err("a bound that is not applied is worse than one that is refused");
    assert!(typo.to_string().contains("probe_budget"), "{typo}");
}
