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
    per_anchor: BTreeMap<String, Vec<String>>,
}

impl Subscriptions {
    pub fn load(root: &Path, catalog: &Catalog) -> Result<Self, CliError> {
        let notes = crate::memories::scan(root, catalog)?;
        let declared = read_declared(root, DEFAULT_FILE)?;

        let mut per_anchor = BTreeMap::new();
        let mut axes_by_anchor: BTreeMap<&str, Vec<&'static str>> = BTreeMap::new();
        for decl in merged(&declared, &notes) {
            let Some(name) = &decl.shape else { continue };
            let shape = crate::shapes::get(name)?;
            if let Some(watch) = crate::shapes::watch_of(shape) {
                per_anchor.insert(
                    decl.key.clone(),
                    watch.iter().map(|s| (*s).to_owned()).collect(),
                );
                axes_by_anchor.insert(&decl.key, crate::shapes::axes_of(shape));
            }
        }

        let mut per_note = BTreeMap::new();
        for note in &notes {
            let Some(watch) = &note.watch else { continue };
            for want in &note.wants {
                let Some(axes) = axes_by_anchor.get(want.key()) else {
                    continue;
                };
                if let Some(bad) = watch.iter().find(|w| !axes.contains(&w.as_str())) {
                    return Err(CliError(format!(
                        "{}: `watch: {bad}` names no axis of `{}`; it has {}",
                        note.path,
                        want.key(),
                        axes.join(" · ")
                    )));
                }
            }
            per_note.insert(note.path.clone(), watch.clone());
        }

        Ok(Self {
            per_note,
            per_anchor,
        })
    }

    pub fn delivers(&self, anchor: &str, note: &str, state: &State) -> bool {
        let Some(set) = axes_set(state) else {
            return true;
        };
        if set.is_empty() {
            return false;
        }
        match self
            .per_note
            .get(note)
            .or_else(|| self.per_anchor.get(anchor))
        {
            Some(watch) => set.iter().any(|a| watch.contains(a)),
            None => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(v: serde_json::Value) -> State {
        State::new(serde_json::json!({ "position": {}, "v": v, "status": "x" }))
    }

    fn subs(note: &[&str], anchor: &[&str]) -> Subscriptions {
        Subscriptions {
            per_note: BTreeMap::from([(
                "memories/a.md".to_owned(),
                note.iter().map(|s| (*s).to_owned()).collect(),
            )]),
            per_anchor: BTreeMap::from([(
                "k".to_owned(),
                anchor.iter().map(|s| (*s).to_owned()).collect(),
            )]),
        }
    }

    #[test]
    fn an_unwatched_axis_moves_without_handing_back_the_memory() {
        let s = subs(&["logic"], &["missing", "sig", "logic"]);
        let moved_line = state(serde_json::json!({ "logic": false, "line": true }));
        assert!(!s.delivers("k", "memories/a.md", &moved_line));

        let moved_logic = state(serde_json::json!({ "logic": true, "line": false }));
        assert!(s.delivers("k", "memories/a.md", &moved_logic));
    }

    #[test]
    fn a_note_that_says_nothing_takes_its_shapes_default() {
        let s = subs(&[], &["missing", "sig"]);
        let moved_sig = state(serde_json::json!({ "sig": true, "line": false }));
        assert!(s.delivers("k", "memories/b.md", &moved_sig));

        let moved_line = state(serde_json::json!({ "sig": false, "line": true }));
        assert!(!s.delivers("k", "memories/b.md", &moved_line));
    }

    #[test]
    fn a_shape_with_no_vector_delivers_on_any_transition() {
        let s = Subscriptions::default();
        let table = State::new(serde_json::json!({ "position": {}, "n": 3, "status": "moved" }));
        assert!(s.delivers("k", "memories/a.md", &table));
    }

    #[test]
    fn a_settled_vector_hands_back_nothing() {
        let s = Subscriptions::default();
        assert!(!s.delivers(
            "k",
            "memories/a.md",
            &state(serde_json::json!({ "sig": false }))
        ));
    }
}
