use std::collections::BTreeMap;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub const POSITION_ENV: &str = "GMR_POSITION";

pub fn hash(s: &str) -> String {
    format!("{:x}", Sha256::digest(s.as_bytes()))
}

pub struct Candidate {
    pub coord: BTreeMap<String, String>,
    pub facts: Value,
}

impl Candidate {
    pub fn new(coord: BTreeMap<String, String>, facts: Value) -> Self {
        Self { coord, facts }
    }
}

pub fn position() -> Result<Value, String> {
    let raw = std::env::var(POSITION_ENV).unwrap_or_default();
    if raw.trim().is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str(&raw).map_err(|e| format!("{POSITION_ENV} 不是 JSON：{e}"))
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
            "{POSITION_ENV} 里一项坐标都没有 —— 这个探针要一个点位，{} 至少给一项",
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
            "extractor": extractor, "found": false,
            "matched": [], "missed": names(),
            "at": Value::Null, "facts": Value::Null,
            "candidates": 0, "matches": [], "exact": false,
        }));
    };
    let tied: Vec<&Candidate> = candidates.iter().filter(|c| vector(c) == best).collect();
    let pick = tied[nth.min(tied.len() - 1)];
    let pairs = || want.iter().zip(&best);
    let out = json!({
        "extractor": extractor, "found": true,
        "matched": pairs().filter(|(_, h)| **h).map(|((k, _), _)| k).collect::<Vec<_>>(),
        "missed": pairs().filter(|(_, h)| !**h).map(|((k, _), _)| k).collect::<Vec<_>>(),
        "at": pick.coord,
        "facts": pick.facts,
        "candidates": tied.len(),
        "exact": best.iter().all(|hit| *hit),
        "matches": tied.iter()
            .map(|c| json!({ "at": c.coord, "facts": c.facts }))
            .collect::<Vec<_>>(),
    });

    let size = out.to_string().len();
    if size > MAX_BYTES {
        return Err(format!(
            "坐标太宽：命中 {} 个，报告有 {size} 字节，超过上限 {MAX_BYTES}。\n\
             **不截断** —— 一份被截掉的名册正好藏起「少了哪一条」。把坐标写细一点。",
            tied.len()
        ));
    }
    Ok(out)
}

pub fn emit(result: Result<Value, String>) -> ! {
    match result {
        Ok(v) => {
            println!("{v}");
            std::process::exit(0)
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(pairs: &[(&str, &str)]) -> Candidate {
        Candidate::new(
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
    fn nth_picks_among_ties_and_never_runs_off_the_end() {
        let w = want(&[("a", "1")]);
        let tied = [
            cand(&[("a", "1"), ("id", "p")]),
            cand(&[("a", "1"), ("id", "q")]),
        ];
        assert_eq!(report("x", &w, 1, &tied).unwrap()["at"]["id"], "q");
        assert_eq!(report("x", &w, 99, &tied).unwrap()["at"]["id"], "q");
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
        let names: Vec<&str> = r["matches"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["at"]["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, vec!["alpha", "beta"]);
        let one = want(&[("kind", "function"), ("name", "alpha")]);
        let r = report("x", &one, 0, &all).unwrap();
        assert_eq!(r["candidates"], 1);
        assert_eq!(r["matches"].as_array().unwrap().len(), 1);
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
    fn an_oversized_roster_is_refused_never_truncated() {
        let w = want(&[("kind", "function")]);
        let many: Vec<Candidate> = (0..20_000)
            .map(|i| {
                Candidate::new(
                    [
                        ("kind".to_owned(), "function".to_owned()),
                        (
                            "name".to_owned(),
                            format!("f{i}_padded_out_to_take_some_room"),
                        ),
                    ]
                    .into(),
                    json!({ "body": "0".repeat(64) }),
                )
            })
            .collect();
        let e = report("x", &w, 0, &many).unwrap_err();
        assert!(e.contains("坐标太宽"), "{e}");
        assert!(e.contains("不截断"), "{e}");
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
}
