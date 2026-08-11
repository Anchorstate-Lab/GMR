//! The postings union: the only candidates a coordinate can possibly select.
//!
//! Nothing calls this yet. It lands ahead of the query that will use it so the
//! equivalence property below exists before anything depends on it.

use crate::matching::{Candidate, Want};

pub fn touches(candidate: &Candidate, want: &Want) -> bool {
    want.iter().any(|(k, v)| candidate.coord.get(k) == Some(v))
}

pub fn narrow(candidates: &[Candidate], want: &Want) -> Vec<Candidate> {
    candidates
        .iter()
        .filter(|c| touches(c, want))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matching::{MAX_BYTES, report};
    use std::collections::BTreeMap;

    struct Rng(u64);

    impl Rng {
        fn bits(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            self.0 >> 33
        }

        fn below(&mut self, n: usize) -> usize {
            (self.bits() % n as u64) as usize
        }
    }

    const KEYS: [&str; 3] = ["file", "kind", "name"];
    const VALUES: [&str; 4] = ["a", "b", "c", ""];

    fn candidate(rng: &mut Rng) -> Candidate {
        let mut coord: BTreeMap<String, String> = BTreeMap::new();
        for k in KEYS {
            if rng.below(4) > 0 {
                coord.insert(k.to_owned(), VALUES[rng.below(VALUES.len())].to_owned());
            }
        }
        let id = coord
            .values()
            .cloned()
            .collect::<Vec<_>>()
            .join("/")
            .to_string();
        Candidate::new(id, coord, serde_json::json!({}))
    }

    fn corpus(rng: &mut Rng) -> Vec<Candidate> {
        (0..rng.below(9)).map(|_| candidate(rng)).collect()
    }

    fn want(rng: &mut Rng) -> Want {
        loop {
            let mut picked = Want::new();
            for k in KEYS {
                if rng.below(2) == 1 {
                    picked.push((k.to_owned(), VALUES[rng.below(VALUES.len())].to_owned()));
                }
            }
            if !picked.is_empty() {
                return picked;
            }
        }
    }

    fn skips_a_key(want: &Want) -> bool {
        want.iter()
            .map(|(k, _)| k.as_str())
            .ne(KEYS.iter().copied().take(want.len()))
    }

    #[test]
    fn narrowing_never_changes_what_report_says() {
        let mut rng = Rng(0x5EED);
        for round in 0..5000 {
            let all = corpus(&mut rng);
            let w = want(&mut rng);
            let nth = rng.below(3);
            assert_eq!(
                report("x", &w, nth, &all),
                report("x", &w, nth, &narrow(&all, &w)),
                "round {round}: want {w:?}, {} candidates",
                all.len()
            );
        }
    }

    #[test]
    fn the_rounds_actually_reach_every_branch_they_claim_to() {
        let mut rng = Rng(0x5EED);
        let (mut found, mut absent, mut refused, mut narrowed) = (0, 0, 0, 0);
        let mut gapped = 0;
        for _ in 0..5000 {
            let all = corpus(&mut rng);
            let w = want(&mut rng);
            let kept = narrow(&all, &w);
            if kept.len() < all.len() {
                narrowed += 1;
            }
            if skips_a_key(&w) {
                gapped += 1;
            }
            match report("x", &w, rng.below(3), &all) {
                Err(_) => refused += 1,
                Ok(v) if v["found"] == true => found += 1,
                Ok(_) => absent += 1,
            }
        }
        assert!(found > 100, "found:true was reached {found} times");
        assert!(absent > 100, "found:false was reached {absent} times");
        assert!(refused > 10, "the nth refusal was reached {refused} times");
        assert!(
            narrowed > 100,
            "narrowing actually dropped something {narrowed} times"
        );
        assert!(
            gapped > 100,
            "a want that skips an item and keeps a later one was reached {gapped} times. \
             `wanted` builds the want by filtering the probe's items against the position, so \
             any subsequence is reachable — and priority is what `best`'s lexicographic max \
             turns on, so a generator that only ever emits prefixes leaves that untested"
        );
    }

    fn cand(pairs: &[(&str, &str)]) -> Candidate {
        Candidate::new(
            pairs.iter().map(|(_, v)| *v).collect::<Vec<_>>().join("/"),
            pairs
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
            serde_json::json!({}),
        )
    }

    fn w(pairs: &[(&str, &str)]) -> Want {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn an_empty_corpus_agrees() {
        let want = w(&[("a", "1")]);
        assert_eq!(
            report("x", &want, 0, &[]),
            report("x", &want, 0, &narrow(&[], &want))
        );
    }

    #[test]
    fn a_want_nothing_matches_is_found_false_on_both_sides() {
        let want = w(&[("file", "gone.rs")]);
        let all = [cand(&[("file", "a.rs")]), cand(&[("file", "b.rs")])];
        let kept = narrow(&all, &want);
        assert!(
            kept.is_empty(),
            "nothing can win, so nothing is materialised"
        );
        let full = report("x", &want, 0, &all).unwrap();
        let thin = report("x", &want, 0, &kept).unwrap();
        assert_eq!(full["found"], false);
        assert_eq!(full, thin);
    }

    #[test]
    fn a_want_everything_matches_narrows_to_nothing_and_still_agrees() {
        let want = w(&[("kind", "function")]);
        let all = [
            cand(&[("kind", "function"), ("name", "a")]),
            cand(&[("kind", "function"), ("name", "b")]),
        ];
        let kept = narrow(&all, &want);
        assert_eq!(kept.len(), all.len());
        assert_eq!(report("x", &want, 0, &all), report("x", &want, 0, &kept));
    }

    #[test]
    fn an_out_of_range_nth_refuses_with_the_same_words_on_both_sides() {
        let want = w(&[("kind", "function")]);
        let all = [
            cand(&[("kind", "function"), ("name", "a")]),
            cand(&[("kind", "type"), ("name", "b")]),
        ];
        let full = report("x", &want, 9, &all);
        let thin = report("x", &want, 9, &narrow(&all, &want));
        assert!(full.is_err());
        assert_eq!(full, thin);
    }

    #[test]
    fn an_oversized_roll_refuses_with_the_same_words_on_both_sides() {
        let want = w(&[("kind", "function")]);
        let all: Vec<Candidate> = (0..20_000)
            .map(|i| {
                let name = format!("f{i}_padded_out_to_take_up_some_room_in_the_roll");
                Candidate::new(
                    name.clone(),
                    [
                        ("kind".to_owned(), "function".to_owned()),
                        ("name".to_owned(), name),
                    ]
                    .into(),
                    serde_json::json!({ "body": "0".repeat(64) }),
                )
            })
            .collect();
        let full = report("x", &want, 0, &all);
        let thin = report("x", &want, 0, &narrow(&all, &want));
        assert!(full.is_err());
        assert_eq!(full, thin);
        assert!(full.unwrap_err().contains(&MAX_BYTES.to_string()));
    }

    #[test]
    fn narrowing_keeps_the_order_nth_counts_in() {
        let want = w(&[("kind", "function")]);
        let all = [
            cand(&[("kind", "function"), ("name", "first")]),
            cand(&[("kind", "type"), ("name", "middle")]),
            cand(&[("kind", "function"), ("name", "last")]),
        ];
        let kept = narrow(&all, &want);
        assert_eq!(kept.len(), 2);
        for nth in 0..2 {
            assert_eq!(
                report("x", &want, nth, &all).unwrap()["at"]["name"],
                report("x", &want, nth, &kept).unwrap()["at"]["name"],
                "nth={nth} has to name the same object on both sides"
            );
        }
    }

    #[test]
    fn a_candidate_that_hits_nothing_can_never_be_the_one_reported() {
        let want = w(&[("file", "a.rs"), ("name", "wanted")]);
        let all = [
            cand(&[("file", "a.rs"), ("name", "other")]),
            cand(&[("file", "z.rs"), ("name", "unrelated")]),
        ];
        assert!(!touches(&all[1], &want));
        let reported = report("x", &want, 0, &all).unwrap();
        assert_eq!(reported["at"]["file"], "a.rs");
        assert_eq!(
            reported,
            report("x", &want, 0, &narrow(&all, &want)).unwrap()
        );
    }
}
