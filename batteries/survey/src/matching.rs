use std::collections::BTreeMap;

use serde_json::{Value, json};

pub const COORD_REPORT_SCHEMA: &str = "gmr.probe-coord.v1";

pub const REPORT: &[&str] = &[
    "schema",
    "extractor",
    "found",
    "matched",
    "missed",
    "candidates",
    "roll",
    "exact",
    "priority",
];

pub const PER_CANDIDATE: &[&str] = &["at", "facts"];

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Candidate {
    pub coord: BTreeMap<String, String>,
    pub facts: Value,
    id: String,
}

impl Candidate {
    pub fn new(id: impl Into<String>, coord: BTreeMap<String, String>, facts: Value) -> Self {
        Self {
            coord,
            facts,
            id: id.into(),
        }
    }
}

fn roll(tied: &[&Candidate]) -> String {
    let mut ids: Vec<&str> = tied.iter().map(|c| c.id.as_str()).collect();
    ids.sort_unstable();
    ids.join("\n")
}

pub type Want = Vec<(String, String)>;

pub fn wanted(pos: &Value, items: &[&str]) -> Result<Want, String> {
    let want: Want = items
        .iter()
        .filter_map(|i| {
            pos.get(*i)
                .and_then(Value::as_str)
                .map(|v| ((*i).to_owned(), v.to_owned()))
        })
        .collect();
    if want.is_empty() {
        return Err(format!(
            "the position has no coordinate fields; this probe needs at least one of {}",
            items.join("/")
        ));
    }
    Ok(want)
}

pub fn nth(pos: &Value) -> usize {
    pos.get("nth").and_then(Value::as_u64).unwrap_or(0) as usize
}

pub const MAX_BYTES: usize = 900_000;

pub fn report(
    extractor: &str,
    want: &Want,
    nth: usize,
    candidates: &[Candidate],
) -> Result<Value, String> {
    let vector = |c: &Candidate| -> Vec<bool> {
        want.iter()
            .map(|(k, v)| c.coord.get(k) == Some(v))
            .collect()
    };
    let best = candidates.iter().map(vector).max();
    let names = || want.iter().map(|(k, _)| k).collect::<Vec<_>>();

    let Some(best) = best.filter(|b| b.iter().any(|hit| *hit)) else {
        return Ok(json!({
            "schema": COORD_REPORT_SCHEMA,
            "extractor": extractor, "found": false,
            "matched": [], "missed": names(),
            "at": Value::Null, "facts": Value::Null,
            "candidates": 0, "roll": "", "exact": false,
            "priority": names(),
        }));
    };
    let tied: Vec<&Candidate> = candidates.iter().filter(|c| vector(c) == best).collect();
    let Some(pick) = tied.get(nth) else {
        return Err(format!(
            "the position has nth={nth}, but only {} equally good candidates exist.\n\
             Refusing to clamp: silently choosing another candidate would make the anchor watch a different object.\n\
             Tighten the coordinate or fix nth.",
            tied.len()
        ));
    };
    let pairs = || want.iter().zip(&best);
    let out = json!({
        "schema": COORD_REPORT_SCHEMA,
        "extractor": extractor, "found": true,
        "matched": pairs().filter(|(_, h)| **h).map(|((k, _), _)| k).collect::<Vec<_>>(),
        "missed": pairs().filter(|(_, h)| !**h).map(|((k, _), _)| k).collect::<Vec<_>>(),
        "at": pick.coord,
        "facts": pick.facts,
        "candidates": tied.len(),
        "roll": roll(&tied),
        "exact": best.iter().all(|hit| *hit),
        "priority": names(),
    });

    let size = out.to_string().len();
    if size > MAX_BYTES {
        return Err(format!(
            "coordinate is too broad: {} equally good candidates produce a {size}-byte report, above the {MAX_BYTES} limit.\n\
             Refusing to truncate: a truncated roll can hide which item disappeared. Tighten the coordinate.",
            tied.len()
        ));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(pairs: &[(&str, &str)]) -> Candidate {
        let id = pairs
            .iter()
            .find(|(k, _)| *k == "name")
            .map_or("anon", |(_, v)| *v)
            .to_owned();
        Candidate::new(
            id,
            pairs
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
            json!({}),
        )
    }

    fn want(pairs: &[(&str, &str)]) -> Want {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn an_earlier_item_outranks_a_later_one_at_the_same_count() {
        let w = want(&[("name", "moved"), ("scope", "core")]);
        let cands = [
            cand(&[("name", "keep"), ("scope", "core")]),
            cand(&[("name", "moved"), ("scope", "")]),
        ];
        let r = report("x", &w, 0, &cands).unwrap();
        assert_eq!(r["matched"], json!(["name"]));
        assert_eq!(r["missed"], json!(["scope"]));
        assert_eq!(r["candidates"], 1);
    }

    #[test]
    fn every_item_matching_leaves_missed_empty() {
        let r = report(
            "x",
            &want(&[("a", "1"), ("b", "2")]),
            0,
            &[cand(&[("a", "1"), ("b", "2")])],
        )
        .unwrap();
        assert_eq!(r["found"], true);
        assert_eq!(r["missed"], json!([]));
        assert_eq!(r["candidates"], 1);
    }

    #[test]
    fn one_item_missing_still_yields_the_best_candidate() {
        let r = report(
            "x",
            &want(&[("a", "1"), ("b", "2")]),
            0,
            &[
                cand(&[("a", "1"), ("b", "999")]),
                cand(&[("a", "9"), ("b", "9")]),
            ],
        )
        .unwrap();
        assert_eq!(r["matched"], json!(["a"]));
        assert_eq!(r["missed"], json!(["b"]));
        assert_eq!(r["candidates"], 1);
    }

    #[test]
    fn candidates_is_how_sure_the_probe_is() {
        let w = want(&[("file", "a"), ("name", "gone")]);
        let tied = [
            cand(&[("file", "a"), ("name", "x")]),
            cand(&[("file", "a"), ("name", "y")]),
        ];
        assert_eq!(report("x", &w, 0, &tied).unwrap()["candidates"], 2);
    }

    #[test]
    fn nothing_matching_at_all_is_found_false() {
        let r = report("x", &want(&[("a", "1")]), 0, &[cand(&[("a", "2")])]).unwrap();
        assert_eq!(r["found"], false);
        assert_eq!(r["at"], Value::Null);
    }

    #[test]
    fn nth_picks_among_ties() {
        let w = want(&[("a", "1")]);
        let tied = [
            cand(&[("a", "1"), ("id", "p")]),
            cand(&[("a", "1"), ("id", "q")]),
        ];
        assert_eq!(report("x", &w, 0, &tied).unwrap()["at"]["id"], "p");
        assert_eq!(report("x", &w, 1, &tied).unwrap()["at"]["id"], "q");
    }

    #[test]
    fn an_out_of_range_nth_is_refused_not_clamped() {
        let w = want(&[("a", "1")]);
        let tied = [
            cand(&[("a", "1"), ("id", "p")]),
            cand(&[("a", "1"), ("id", "q")]),
        ];
        let e = report("x", &w, 99, &tied).unwrap_err();
        assert!(e.contains("only 2 equally good candidates"), "{e}");
        assert!(
            e.contains("Refusing to clamp"),
            "silently choosing another candidate makes the anchor watch another object"
        );
    }

    #[test]
    fn the_order_of_the_items_is_the_priority_and_it_is_reported() {
        let w = want(&[("name", "assess"), ("file", "a.rs")]);
        let out = report(
            "x",
            &w,
            0,
            &[
                cand(&[("name", "assess"), ("file", "moved.rs")]),
                cand(&[("name", "renamed"), ("file", "a.rs")]),
            ],
        )
        .unwrap();
        assert_eq!(
            out["at"]["file"], "moved.rs",
            "the name is a stronger identity signal than the path"
        );
        assert_eq!(out["matched"], serde_json::json!(["name"]));
        assert_eq!(
            out["priority"],
            serde_json::json!(["name", "file"]),
            "priority should not only be hidden in argument order"
        );
    }

    #[test]
    fn an_underspecified_coordinate_lists_everything_that_tied() {
        let w = want(&[("kind", "function")]);
        let all = [
            cand(&[("kind", "function"), ("name", "alpha")]),
            cand(&[("kind", "function"), ("name", "beta")]),
            cand(&[("kind", "type"), ("name", "S")]),
        ];
        let r = report("x", &w, 0, &all).unwrap();
        assert_eq!(r["candidates"], 2);
        assert_eq!(r["roll"], "alpha\nbeta");
        let one = want(&[("kind", "function"), ("name", "alpha")]);
        let r = report("x", &one, 0, &all).unwrap();
        assert_eq!(r["candidates"], 1);
        assert_eq!(r["roll"], "alpha");
    }

    #[test]
    fn exact_says_whether_every_item_matched_or_this_is_a_fallback() {
        let w = want(&[("kind", "module"), ("vis", "pub")]);
        let hit = [cand(&[("kind", "module"), ("vis", "pub")])];
        assert_eq!(report("x", &w, 0, &hit).unwrap()["exact"], true);
        let fallback = [cand(&[("kind", "type"), ("vis", "pub")])];
        let r = report("x", &w, 0, &fallback).unwrap();
        assert_eq!(r["exact"], false);
        assert_eq!(r["missed"], json!(["kind"]));
    }

    #[test]
    fn the_roll_has_one_line_per_candidate() {
        let all = vec![
            cand(&[("kind", "function"), ("name", "b")]),
            cand(&[("kind", "function"), ("name", "a")]),
            cand(&[("kind", "function"), ("name", "a")]),
        ];
        let out = report("x", &want(&[("kind", "function")]), 0, &all).unwrap();
        let roll = out["roll"].as_str().unwrap();
        assert_eq!(
            roll.lines().count(),
            out["candidates"].as_u64().unwrap() as usize
        );
        assert_eq!(roll, "a\na\nb", "sorted, and a repeat is still two lines");
    }

    #[test]
    fn an_oversized_roster_is_refused_never_truncated() {
        let w = want(&[("kind", "function")]);
        let many: Vec<Candidate> = (0..20_000)
            .map(|i| {
                let name = format!("f{i}_padded_out_to_take_up_some_room_in_the_roll");
                Candidate::new(
                    name.clone(),
                    [
                        ("kind".to_owned(), "function".to_owned()),
                        ("name".to_owned(), name),
                    ]
                    .into(),
                    json!({ "body": "0".repeat(64) }),
                )
            })
            .collect();
        let e = report("x", &w, 0, &many).unwrap_err();
        assert!(e.contains("coordinate is too broad"), "{e}");
        assert!(e.contains("Refusing to truncate"), "{e}");
    }

    #[test]
    fn a_position_with_none_of_our_items_is_our_failure() {
        assert!(wanted(&json!({"other": "v"}), &["a", "b"]).is_err());
        assert!(wanted(&json!({}), &["a"]).is_err());
        assert!(wanted(&json!({"a": "v"}), &["a"]).is_ok());
    }

    #[test]
    fn no_candidates_at_all_is_found_false_not_a_panic() {
        assert_eq!(
            report("x", &want(&[("a", "1")]), 0, &[]).unwrap()["found"],
            false
        );
    }

    fn keys(v: &Value) -> Vec<String> {
        let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
        k.sort();
        k
    }

    #[test]
    fn both_branches_report_the_same_keys_and_they_are_the_declared_ones() {
        let w = want(&[("kind", "function")]);
        let hit = report("x", &w, 0, &[cand(&[("kind", "function")])]).unwrap();
        let miss = report("x", &w, 0, &[cand(&[("kind", "type")])]).unwrap();
        assert_eq!(
            miss["found"], false,
            "the second call must take found:false"
        );

        let mut declared: Vec<String> = REPORT
            .iter()
            .chain(PER_CANDIDATE)
            .map(|s| (*s).to_owned())
            .collect();
        declared.sort();
        assert_eq!(keys(&hit), declared);
        assert_eq!(
            keys(&miss),
            declared,
            "a key only one branch emits is Absent to any rule that reads it, \
             and which branch ran is exactly what such a rule is asking about"
        );
    }
}
