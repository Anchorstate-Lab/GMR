use std::collections::BTreeMap;
use std::path::Path;

use gmr::{Ref, State};
use serde_json::Value;

use crate::error::CliError;
use crate::memories::{Fault, Names, Note, Watch, Weight};
use crate::probes::Catalog;
use crate::verbs::sync::{DEFAULT_FILE, merged, read_declared};

fn declaring(notes: &[Note], names: &Names, key: &str) -> String {
    notes
        .iter()
        .find(|n| n.wants.iter().any(|w| w.key() == key))
        .map_or_else(|| DEFAULT_FILE.to_owned(), |n| names.of(&n.reference))
}

pub fn axes_set(state: &State) -> Option<Vec<String>> {
    let v = state.as_value().get("v").and_then(Value::as_object)?;
    Some(
        v.iter()
            .filter(|(_, on)| on.as_bool().unwrap_or(false))
            .map(|(k, _)| k.clone())
            .collect(),
    )
}

#[derive(Debug, Default)]
pub struct Subscriptions {
    per_note: BTreeMap<Ref, gmr::expr::Node>,
    per_anchor: BTreeMap<String, gmr::expr::Node>,
}

pub fn axes_predicate(axes: &[impl AsRef<str>]) -> String {
    match axes.is_empty() {
        true => "false".to_owned(),
        false => axes
            .iter()
            .map(|a| {
                let axis = a.as_ref();
                format!("(exists(state.v.{axis}) and state.v.{axis})")
            })
            .collect::<Vec<_>>()
            .join(" or "),
    }
}

fn compile(watch: &Watch) -> Result<gmr::expr::Node, String> {
    let source = match watch {
        Watch::Axes(axes) => axes_predicate(axes),
        Watch::When(src) => src.clone(),
    };
    gmr::expr::parse(&source).map_err(|e| format!("`{source}`: {e}"))
}

fn unwritable(
    paths: &std::collections::BTreeSet<String>,
    writes: &crate::contract::Writes,
) -> Vec<String> {
    paths
        .iter()
        .filter(|p| !writes.reaches(p))
        .cloned()
        .collect()
}

impl Subscriptions {
    pub fn load(
        root: &Path,
        catalog: &Catalog,
        names: &Names,
    ) -> Result<(Self, Vec<Fault>), CliError> {
        let crate::memories::Scanned { notes, .. } = crate::memories::scan(root, catalog)?;
        let declared = read_declared(root, DEFAULT_FILE)?;

        let mut faults = Vec::new();
        let mut per_anchor = BTreeMap::new();
        let mut undecidable = std::collections::BTreeSet::new();
        let mut writes_by_anchor: BTreeMap<&str, crate::contract::Writes> = BTreeMap::new();
        for decl in merged(&declared, &notes) {
            if let Some(name) = &decl.shape {
                if let Err(e) = crate::shapes::get(name) {
                    faults.push(Fault {
                        note: declaring(&notes, names, &decl.key),
                        key: Some(decl.key.clone()),
                        code: "unknown-shape",
                        detail: format!("`{}`: {e}", decl.key),
                        weight: Weight::Breaks,
                    });
                    continue;
                }
            }
            if let Ok(transitions) = decl.to_transitions() {
                if let Ok(writes) = crate::contract::writes_of(&transitions) {
                    writes_by_anchor.insert(&decl.key, writes);
                }
            }
            if let Some(watch) = &decl.watch {
                match compile(watch) {
                    Ok(node) => {
                        per_anchor.insert(decl.key.clone(), node);
                    }
                    Err(detail) => faults.push(Fault {
                        note: declaring(&notes, names, &decl.key),
                        key: Some(decl.key.clone()),
                        code: "watch-invalid",
                        detail: format!("`{}`'s own `watch:` does not parse: {detail}", decl.key),
                        weight: Weight::Breaks,
                    }),
                }
            }
            if decl.shape.is_none() && decl.watch.is_none() {
                undecidable.insert(decl.key.clone());
            }
        }

        let mut per_note = BTreeMap::new();
        'note: for note in &notes {
            let Some(watch) = &note.watch else { continue };
            let node = match compile(watch) {
                Ok(node) => node,
                Err(detail) => {
                    faults.push(Fault {
                        note: names.of(&note.reference),
                        key: note.wants.first().map(|w| w.key().to_owned()),
                        code: "watch-invalid",
                        detail: format!("`watch:` does not parse: {detail}"),
                        weight: Weight::Breaks,
                    });
                    continue;
                }
            };
            let named = crate::contract::state_paths(&node);
            for want in &note.wants {
                let Some(writes) = writes_by_anchor.get(want.key()) else {
                    continue;
                };
                let bad = unwritable(&named, writes);
                if !bad.is_empty() {
                    faults.push(Fault {
                        note: names.of(&note.reference),
                        key: Some(want.key().to_owned()),
                        code: "watch-invalid",
                        detail: format!(
                            "`watch:` names {}, which no rule of `{}` ever writes; it writes {}",
                            bad.join(" · "),
                            want.key(),
                            writes.render()
                        ),
                        weight: Weight::Breaks,
                    });
                    continue 'note;
                }
            }
            per_note.insert(note.reference.clone(), node);
        }

        for note in &notes {
            if note.watch.is_some() {
                continue;
            }
            for want in &note.wants {
                if !undecidable.contains(want.key()) {
                    continue;
                }
                faults.push(Fault {
                    note: names.of(&note.reference),
                    key: Some(want.key().to_owned()),
                    code: "watch-missing",
                    detail: format!(
                        "`{}` writes its own rules, so nothing says when this memory should \
                         come back. Give the note a `watch:`, or the anchor a default one",
                        want.key()
                    ),
                    weight: Weight::Breaks,
                });
            }
        }

        Ok((
            Self {
                per_note,
                per_anchor,
            },
            faults,
        ))
    }

    pub fn delivers(
        &self,
        key: &str,
        shape: Option<&crate::shapes::Shape>,
        note: &Ref,
        state: &State,
    ) -> Result<bool, String> {
        let owned;
        let node = match self.per_note.get(note).or_else(|| self.per_anchor.get(key)) {
            Some(node) => node,
            None => {
                let Some(shape) = shape else {
                    return Err(
                        "nothing says when this memory should come back: the anchor writes its \
                         own rules and neither it nor the note carries a `watch:`"
                            .to_owned(),
                    );
                };
                let source = axes_predicate(crate::shapes::watch_of(shape));
                owned = gmr::expr::parse(&source).map_err(|e| format!("`{source}`: {e}"))?;
                &owned
            }
        };
        let nothing = Value::Null;
        let ctx = gmr::expr::Ctx::new(&nothing, state.as_value());
        match gmr::expr::eval(node, ctx) {
            gmr::expr::Evaluated::Value(Value::Bool(on)) => Ok(on),
            gmr::expr::Evaluated::Value(other) => Err(format!(
                "`watch:` answered with {other}, which is not a yes or a no"
            )),
            gmr::expr::Evaluated::Absent => Ok(false),
            gmr::expr::Evaluated::Fault(f) => Err(format!("`watch:` could not be settled: {f:?}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(v: serde_json::Value) -> State {
        State::new(serde_json::json!({ "position": {}, "v": v, "status": "x" }))
    }

    fn at(provider: &str, id: &str) -> Ref {
        Ref::new(provider, id)
    }

    fn narrowed(note: &[&str]) -> Subscriptions {
        Subscriptions {
            per_note: BTreeMap::from([(
                at("git", "memories/a.md"),
                gmr::expr::parse(&axes_predicate(note)).unwrap(),
            )]),
            per_anchor: BTreeMap::new(),
        }
    }

    fn hands(s: &Subscriptions, note: &Ref, st: &State) -> bool {
        s.delivers("k", contract(), note, st).unwrap()
    }

    fn contract() -> Option<&'static crate::shapes::Shape> {
        crate::shapes::get("contract").ok()
    }

    #[test]
    fn an_unwatched_axis_moves_without_handing_back_the_memory() {
        let s = narrowed(&["logic"]);
        let moved_place = state(serde_json::json!({ "logic": false, "place": true }));
        assert!(!hands(&s, &at("git", "memories/a.md"), &moved_place));

        let moved_logic = state(serde_json::json!({ "logic": true, "place": false }));
        assert!(hands(&s, &at("git", "memories/a.md"), &moved_logic));
    }

    #[test]
    fn the_same_id_in_two_stores_is_two_notes_not_one() {
        let s = narrowed(&["logic"]);
        let moved_place = state(serde_json::json!({ "logic": false, "place": true }));

        assert!(!hands(&s, &at("git", "memories/a.md"), &moved_place));
        assert!(
            hands(&s, &at("mem0", "memories/a.md"), &moved_place),
            "a subscription belongs to one record in one store. Keyed by the bare id, a note \
             in a second store would silently inherit the narrowing of a note it merely shares \
             a name with — and the symptom is a memory that stops being handed back, which \
             looks exactly like the axis simply not having moved"
        );
    }

    #[test]
    fn a_note_that_says_nothing_takes_its_shapes_default() {
        let s = narrowed(&["logic"]);
        let moved_place = state(serde_json::json!({ "logic": false, "place": true }));
        assert!(
            hands(&s, &at("git", "memories/b.md"), &moved_place),
            "contract watches every axis, and this note asked for nothing else"
        );
    }

    #[test]
    fn a_settled_vector_hands_back_nothing() {
        let s = Subscriptions::default();
        assert!(!hands(
            &s,
            &at("git", "memories/a.md"),
            &state(serde_json::json!({ "sig": false }))
        ));
    }

    #[test]
    fn a_set_bit_keeps_handing_the_memory_back_after_the_observation_that_set_it() {
        let s = narrowed(&["sig"]);
        let carried = state(serde_json::json!({ "sig": true }));
        assert!(
            s.delivers("k", contract(), &at("git", "memories/a.md"), &carried)
                .unwrap()
        );
        assert!(
            s.delivers("k", contract(), &at("git", "memories/b.md"), &carried)
                .unwrap()
        );
    }

    #[test]
    fn an_anchor_with_no_shape_and_no_watch_refuses_to_guess() {
        let s = Subscriptions::default();
        let hand = State::new(serde_json::json!({ "position": {}, "n": 3, "status": "moved" }));
        assert!(
            s.delivers("k", None, &at("git", "memories/a.md"), &hand)
                .is_err(),
            "delivering on the transition edge announced the obligation once and lost it; \
             staying quiet would lose the memory. Neither is an answer this layer may invent"
        );
    }

    fn book(root: &Path) -> Names {
        Names::over(vec![std::sync::Arc::new(crate::memories::declaring(root))])
    }

    fn world(files: &[(&str, &str)]) -> (tempfile::TempDir, crate::probes::Catalog) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".anchor")).unwrap();
        for (path, body) in files {
            let p = dir.path().join(path);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, body).unwrap();
        }
        let catalog = crate::probes::Catalog::load(dir.path()).unwrap();
        (dir, catalog)
    }

    #[test]
    fn a_watch_axis_a_shape_does_not_have_is_isolated_to_its_own_note() {
        let (d, c) = world(&[
            ("src/a.rs", "fn a() {}"),
            (
                "memories/bad.md",
                "---\nabout: src/a.rs#a\nwatch: [not_a_real_axis]\n---\n",
            ),
            (
                "memories/good.md",
                "---\nabout: src/a.rs\nwatch: [roll]\n---\n",
            ),
        ]);
        let (subs, broken) = Subscriptions::load(d.path(), &c, &book(d.path())).unwrap();
        assert_eq!(broken.len(), 1);
        assert_eq!(
            broken[0].note, "bad",
            "doctor prints this column next to the note lint's, and that one has always \
             spelled a note by its name. Two spellings in one column is how a reader learns \
             that `bad` and `memories/bad.md` might be two different files"
        );
        assert!(
            broken[0].detail.contains("not_a_real_axis"),
            "{}",
            broken[0].detail
        );

        let roster = crate::shapes::get("roster").ok();
        let moved_roll = state(serde_json::json!({ "roll": true }));
        assert!(
            subs.delivers("k", roster, &at("git", "memories/good.md"), &moved_roll)
                .unwrap(),
            "the well-formed note in the same load still narrows correctly"
        );
    }

    #[test]
    fn a_shape_this_build_does_not_ship_names_the_file_to_edit_not_the_anchor() {
        let (d, c) = world(&[
            ("src/a.rs", "fn a() {}"),
            (
                "memories/custom.md",
                "---\nanchors:\n  - key: custom::thing\n    probe: ast-map\n    \
                 position: { file: src/a.rs }\n    shape: no-such-shape\n---\n",
            ),
        ]);
        let (_, faults) = Subscriptions::load(d.path(), &c, &book(d.path())).unwrap();
        assert_eq!(faults.len(), 1);
        assert_eq!(
            faults[0].note, "custom",
            "the column a person reads to know what to open. It used to hold the anchor key, \
             because this failure had a record type of its own whose `note` field nobody had \
             to fill with a note"
        );
        assert_eq!(faults[0].key.as_deref(), Some("custom::thing"));
        assert_eq!(
            faults[0].code, "unknown-shape",
            "an unknown shape has nothing to do with `watch:`, and was reported under \
             `watch-invalid` for as long as the two shared one record"
        );
    }
}
