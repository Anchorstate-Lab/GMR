use std::path::Path;

use gmr::{AnchorKey, Change, Runtime, State};
use serde_json::Value;

use crate::error::CliError;
use crate::probes::Catalog;
use crate::rules;
use crate::verbs::sealed;
use crate::verbs::sync::{AnchorDecl, Context, DEFAULT_FILE, differs, merged, read_declared};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum What {
    Baseline,
    Criteria,
}

pub struct Pending {
    pub axes: Vec<String>,
    pub missing: bool,
    pub unsettled: Option<String>,
    pub facets: Vec<&'static str>,
}

impl Pending {
    fn baseline(&self) -> bool {
        !self.axes.is_empty() || self.unsettled.is_some()
    }

    fn criteria(&self) -> bool {
        !self.facets.is_empty()
    }

    fn standing(&self) -> String {
        match (&self.unsettled, self.axes.is_empty()) {
            (Some(status), _) => status.clone(),
            (None, _) => self.axes.join(" · "),
        }
    }
}

fn recaptured(state: &State) -> State {
    State::new(serde_json::json!({ "position": state.position() }))
}

fn repinned(state: &State) -> Result<State, CliError> {
    let obj = state
        .as_value()
        .as_object()
        .ok_or_else(|| CliError("this anchor's state is not an object".into()))?;
    let Some(v) = obj.get("v").and_then(Value::as_object) else {
        return Ok(recaptured(state));
    };
    let now = obj
        .get("now")
        .cloned()
        .ok_or_else(|| CliError("this anchor's state carries no reading to pin".into()))?;

    let mut out = obj.clone();
    out.insert("baseline".to_owned(), now);
    out.insert(
        "v".to_owned(),
        Value::Object(v.keys().map(|k| (k.clone(), Value::Bool(false))).collect()),
    );
    out.insert(
        "status".to_owned(),
        Value::String(crate::shapes::SETTLED.to_owned()),
    );
    Ok(State::new(Value::Object(out)))
}

fn unsettled_status(view: &gmr::AnchorView) -> Option<String> {
    let name = crate::shapes::name_of(&view.anchor.transitions)?;
    let ok = crate::shapes::settled_of(crate::shapes::get(name).ok()?);
    let status = view.state.status()?.to_string();
    match ok.contains(&status.as_str()) {
        true => None,
        false => Some(status),
    }
}

async fn pending(
    rt: &Runtime,
    root: &Path,
    key: &AnchorKey,
) -> Result<(Pending, State, Option<AnchorDecl>), CliError> {
    let view = rt.read(key).await?;
    if view.closed {
        return Err(CliError(format!(
            "{key} is closed; closure is irreversible"
        )));
    }
    let vector = crate::delivery::axes_set(&view.state);
    let axes = vector.clone().unwrap_or_default();
    let missing = axes.iter().any(|k| k == crate::shapes::MISSING);
    let unsettled = match vector {
        Some(_) => None,
        None => unsettled_status(&view),
    };

    let ctx = Context {
        catalog: Catalog::load(root)?,
    };
    let declared = read_declared(root, DEFAULT_FILE)?;
    let notes = crate::memories::scan(root, &ctx.catalog)?;
    let decl = merged(&declared, &notes)
        .into_iter()
        .find(|d| d.key == key.as_str())
        .cloned();

    let facets = match &decl {
        Some(d) => differs(&view.anchor, d, &ctx)?,
        None => Vec::new(),
    };

    Ok((
        Pending {
            axes,
            missing,
            unsettled,
            facets,
        },
        view.state,
        decl,
    ))
}

fn choose(p: &Pending, asked: Option<What>) -> Result<What, CliError> {
    match asked {
        Some(What::Baseline) if !p.baseline() => Err(CliError(
            "no axis is set; there is no drift to accept".into(),
        )),
        Some(What::Criteria) if !p.criteria() => Err(CliError(
            "the declaration matches the anchor's criteria; there is nothing to accept".into(),
        )),
        Some(w) => Ok(w),
        None => match (p.baseline(), p.criteria()) {
            (false, false) => Err(CliError("nothing is pending on this anchor".into())),
            (true, false) => Ok(What::Baseline),
            (false, true) => Ok(What::Criteria),
            (true, true) => Err(CliError(format!(
                "two different judgments are pending, and one reason cannot cover both:\n\
                 \n    baseline  {} set\n    criteria  the declaration changed its {}\n\
                 \nAccept them one at a time, each with its own reason:\n\
                 \n    gmr accept <key> --baseline --why '...'\n    gmr accept <key> --criteria --why '...'",
                p.standing(),
                p.facets.join(" · ")
            ))),
        },
    }
}

pub async fn run(
    rt: &Runtime,
    root: &Path,
    key: String,
    why: String,
    asked: Option<What>,
    json: bool,
) -> Result<i32, CliError> {
    let key = AnchorKey::new(key);
    let (p, state, decl) = pending(rt, root, &key).await?;
    let what = choose(&p, asked)?;

    let changes: Vec<Change> = match what {
        What::Baseline => {
            if p.missing {
                return Err(CliError(format!(
                    "{key} is missing, so its last reading is stale and there is nothing \
                     current to pin a baseline to.\n\
                     Point the anchor at where the target went, or close it with a reason."
                )));
            }
            vec![Change::Restate {
                state: repinned(&state)?,
            }]
        }
        What::Criteria => {
            let ctx = Context {
                catalog: Catalog::load(root)?,
            };
            let decl = decl.expect("a criteria facet can only differ against a declaration");
            p.facets
                .iter()
                .map(|facet| match *facet {
                    "probe" => Ok(Change::Reprobe {
                        probe: decl.to_probe(&ctx)?,
                    }),
                    "rules" => Ok(Change::Retransition {
                        transitions: decl.to_transitions()?,
                    }),
                    _ => Ok(Change::Reterminal {
                        terminal: rules::terminal(&decl.terminal),
                    }),
                })
                .collect::<Result<_, CliError>>()?
        }
    };

    let mut revised = Vec::new();
    for change in changes {
        revised.push(rt.revise(&key, change, why.as_bytes()).await?);
    }
    let last = revised
        .last()
        .expect("a chosen judgment always has at least one change");

    if json {
        println!(
            "{}",
            serde_json::json!({
                "anchor": key,
                "accepted": match what { What::Baseline => "baseline", What::Criteria => "criteria" },
                "axes": p.axes, "unsettled": p.unsettled, "facets": p.facets,
                "context": last.context, "rationale": last.rationale,
            })
        );
        return Ok(0);
    }

    match what {
        What::Baseline => match &p.unsettled {
            Some(status) => println!(
                "{key} re-captured from `{status}`; if the world still reads that way it says so again"
            ),
            None => println!("{key} re-pinned; {} cleared", p.axes.join(" · ")),
        },
        What::Criteria => println!("{key} took the declaration's {}", p.facets.join(" · ")),
    }
    sealed(&last.context, &last.rationale);
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(v: serde_json::Value) -> State {
        State::new(serde_json::json!({
            "position": { "file": "a.rs", "name": "f" },
            "baseline": { "sig": "(a)", "body": "b1" },
            "now": { "sig": "(a, b)", "body": "b2" },
            "v": v,
            "status": "signature-changed",
        }))
    }

    fn pending(axes: &[&str], facets: &[&'static str]) -> Pending {
        Pending {
            axes: axes.iter().map(|s| (*s).to_owned()).collect(),
            missing: axes.contains(&"missing"),
            unsettled: None,
            facets: facets.to_vec(),
        }
    }

    fn unsettled(status: &str) -> Pending {
        Pending {
            axes: Vec::new(),
            missing: false,
            unsettled: Some(status.to_owned()),
            facets: Vec::new(),
        }
    }

    #[test]
    fn re_pinning_takes_the_reading_as_the_new_baseline() {
        let s = repinned(&state(
            serde_json::json!({ "missing": false, "sig": true, "logic": true }),
        ))
        .unwrap();
        let v = s.as_value();
        assert_eq!(v["baseline"], v["now"]);
        assert_eq!(
            v["v"],
            serde_json::json!({ "missing": false, "sig": false, "logic": false })
        );
        assert_eq!(v["status"], "settled");
        assert_eq!(v["position"]["name"], "f");
    }

    #[test]
    fn a_shape_without_a_vector_re_captures_instead_of_pinning_a_baseline_it_has_not_got() {
        let table = State::new(
            serde_json::json!({ "position": { "file": "a.md" }, "n": 3, "status": "moved" }),
        );
        let s = repinned(&table).unwrap();
        assert_eq!(
            s.as_value(),
            &serde_json::json!({ "position": { "file": "a.md" } })
        );
    }

    #[test]
    fn a_table_shape_sitting_on_an_unsettled_status_has_something_to_accept() {
        assert_eq!(
            choose(&unsettled("section-gone"), None).unwrap(),
            What::Baseline
        );
        assert!(choose(&unsettled("section-gone"), Some(What::Baseline)).is_ok());
    }

    #[test]
    fn two_pending_judgments_refuse_to_share_one_reason() {
        let e = choose(&pending(&["sig"], &["rules"]), None).unwrap_err();
        assert!(e.to_string().contains("one at a time"), "{e}");
        assert!(e.to_string().contains("--baseline"), "{e}");
    }

    #[test]
    fn one_pending_judgment_needs_no_flag() {
        assert_eq!(
            choose(&pending(&["sig"], &[]), None).unwrap(),
            What::Baseline
        );
        assert_eq!(
            choose(&pending(&[], &["probe"]), None).unwrap(),
            What::Criteria
        );
    }

    #[test]
    fn accepting_what_is_not_pending_is_refused() {
        let e = choose(&pending(&[], &[]), None).unwrap_err();
        assert!(e.to_string().contains("nothing is pending"), "{e}");

        let e = choose(&pending(&["sig"], &[]), Some(What::Criteria)).unwrap_err();
        assert!(e.to_string().contains("nothing to accept"), "{e}");
    }
}
