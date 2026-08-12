use std::collections::BTreeMap;
use std::path::Path;

use gmr::State;
use serde_json::Value;

use crate::error::CliError;
use crate::probes::Catalog;
use crate::verbs::sync::{DEFAULT_FILE, merged, read_declared};

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
    per_note: BTreeMap<String, Vec<String>>,
}

pub struct Unwatchable {
    pub note: String,
    pub reason: String,
}

impl Subscriptions {
    pub fn load(root: &Path, catalog: &Catalog) -> Result<(Self, Vec<Unwatchable>), CliError> {
        let crate::memories::Scanned { notes, .. } = crate::memories::scan(root, catalog)?;
        let declared = read_declared(root, DEFAULT_FILE)?;

        let mut broken = Vec::new();
        let mut axes_by_anchor: BTreeMap<&str, Vec<&'static str>> = BTreeMap::new();
        for decl in merged(&declared, &notes) {
            let Some(name) = &decl.shape else { continue };
            match crate::shapes::get(name) {
                Ok(shape) => {
                    axes_by_anchor.insert(&decl.key, crate::shapes::axes_of(shape));
                }
                Err(e) => broken.push(Unwatchable {
                    note: decl.key.clone(),
                    reason: e.to_string(),
                }),
            }
        }

        let mut per_note = BTreeMap::new();
        'note: for note in &notes {
            let Some(watch) = &note.watch else { continue };
            for want in &note.wants {
                let Some(axes) = axes_by_anchor.get(want.key()) else {
                    continue;
                };
                if let Some(bad) = watch.iter().find(|w| !axes.contains(&w.as_str())) {
                    broken.push(Unwatchable {
                        note: note.path.clone(),
                        reason: format!(
                            "`watch: {bad}` names no axis of `{}`; it has {}",
                            want.key(),
                            axes.join(" · ")
                        ),
                    });
                    continue 'note;
                }
            }
            per_note.insert(note.path.clone(), watch.clone());
        }

        Ok((Self { per_note }, broken))
    }

    pub fn delivers(
        &self,
        shape: Option<&crate::shapes::Shape>,
        note: &str,
        state: &State,
        moved: bool,
    ) -> bool {
        let Some(shape) = shape else {
            return moved;
        };
        let set = axes_set(state).unwrap_or_default();
        if set.is_empty() {
            return false;
        }
        match self.per_note.get(note) {
            Some(watch) => set.iter().any(|a| watch.contains(a)),
            None => {
                let watch = crate::shapes::watch_of(shape);
                set.iter().any(|a| watch.contains(&a.as_str()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(v: serde_json::Value) -> State {
        State::new(serde_json::json!({ "position": {}, "v": v, "status": "x" }))
    }

    fn narrowed(note: &[&str]) -> Subscriptions {
        Subscriptions {
            per_note: BTreeMap::from([(
                "memories/a.md".to_owned(),
                note.iter().map(|s| (*s).to_owned()).collect(),
            )]),
        }
    }

    fn contract() -> Option<&'static crate::shapes::Shape> {
        crate::shapes::get("contract").ok()
    }

    #[test]
    fn an_unwatched_axis_moves_without_handing_back_the_memory() {
        let s = narrowed(&["logic"]);
        let moved_place = state(serde_json::json!({ "logic": false, "place": true }));
        assert!(!s.delivers(contract(), "memories/a.md", &moved_place, true));

        let moved_logic = state(serde_json::json!({ "logic": true, "place": false }));
        assert!(s.delivers(contract(), "memories/a.md", &moved_logic, true));
    }

    #[test]
    fn a_note_that_says_nothing_takes_its_shapes_default() {
        let s = narrowed(&["logic"]);
        let moved_place = state(serde_json::json!({ "logic": false, "place": true }));
        assert!(
            s.delivers(contract(), "memories/b.md", &moved_place, true),
            "contract watches every axis, and this note asked for nothing else"
        );
    }

    #[test]
    fn a_settled_vector_hands_back_nothing() {
        let s = Subscriptions::default();
        assert!(!s.delivers(
            contract(),
            "memories/a.md",
            &state(serde_json::json!({ "sig": false })),
            true
        ));
    }

    #[test]
    fn a_set_bit_keeps_handing_the_memory_back_after_the_observation_that_set_it() {
        let s = narrowed(&["sig"]);
        let carried = state(serde_json::json!({ "sig": true }));
        assert!(s.delivers(contract(), "memories/a.md", &carried, false));
        assert!(s.delivers(contract(), "memories/b.md", &carried, false));
    }

    #[test]
    fn an_anchor_with_no_shape_falls_back_to_the_transition_edge() {
        let s = Subscriptions::default();
        let hand = State::new(serde_json::json!({ "position": {}, "n": 3, "status": "moved" }));
        assert!(s.delivers(None, "memories/a.md", &hand, true));
        assert!(!s.delivers(None, "memories/a.md", &hand, false));
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
        let (subs, broken) = Subscriptions::load(d.path(), &c).unwrap();
        assert_eq!(broken.len(), 1);
        assert_eq!(broken[0].note, "memories/bad.md");
        assert!(
            broken[0].reason.contains("not_a_real_axis"),
            "{}",
            broken[0].reason
        );

        let roster = crate::shapes::get("roster").ok();
        let moved_roll = state(serde_json::json!({ "roll": true }));
        assert!(
            subs.delivers(roster, "memories/good.md", &moved_roll, true),
            "the well-formed note in the same load still narrows correctly"
        );
    }
}
