use std::path::Path;

use gmr::{Change, Runtime, State};

use crate::cli::ReviseArgs;
use crate::error::CliError;
use crate::probes::Catalog;
use crate::rules;
use crate::verbs::sealed;

#[derive(Debug)]
enum What {
    Probe,
    Rules,
    Terminal,
    State,
}

fn choose(args: &ReviseArgs) -> Result<What, CliError> {
    let picked: Vec<What> = [
        (What::Probe, args.probe.is_some()),
        (What::Rules, !args.rules.is_empty()),
        (What::Terminal, !args.terminal.is_empty()),
        (What::State, args.state.is_some()),
    ]
    .into_iter()
    .filter_map(|(w, present)| present.then_some(w))
    .collect();

    match picked.len() {
        0 => Err(CliError(
            "name what to revise: --probe, --rule, --terminal, or --state".into(),
        )),
        1 => Ok(picked.into_iter().next().expect("len checked above")),
        _ => Err(CliError(
            "revise one criterion at a time: --probe, --rule, --terminal and --state \
             are separate judgments, each wants its own --why"
                .into(),
        )),
    }
}

pub async fn run(rt: &Runtime, root: &Path, args: ReviseArgs, json: bool) -> Result<i32, CliError> {
    let what = choose(&args)?;
    let key = super::resolve_one(rt, &args.key).await?;

    match what {
        What::Probe => {
            let probe_name = args.probe.expect("What::Probe implies args.probe");
            let kind = Catalog::load(root)?.kind_of(&probe_name);
            let probe = rules::probe(kind, &probe_name, rules::params(&args.params)?)?;
            let revised = rt
                .revise(&key, Change::Reprobe { probe }, args.why.as_bytes())
                .await?;

            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "reprobed": key,
                        "context": revised.context,
                        "rationale": revised.rationale,
                        "incomparable_state": revised.incomparable_state,
                    })
                );
            } else {
                println!("{key} changed probe");
                if revised.incomparable_state {
                    println!(
                        "  ! The state was derived by another rule and is not comparable to the new observation.\n    \
                         Either restate to recapture it, or explicitly accept cross-rule comparability; that is your assertion, the substrate only records it."
                    );
                }
                sealed(&revised.context, &revised.rationale);
            }
        }
        What::Rules => {
            let revised = rt
                .revise(
                    &key,
                    Change::Retransition {
                        transitions: rules::transitions(&args.rules)?,
                    },
                    args.why.as_bytes(),
                )
                .await?;

            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "retransitioned": key,
                        "context": revised.context,
                        "rationale": revised.rationale,
                        "warnings": revised.warnings,
                    })
                );
            } else {
                println!("{key} changed transition table");
                for w in &revised.warnings {
                    println!("  ! {w}");
                }
                sealed(&revised.context, &revised.rationale);
            }
        }
        What::Terminal => {
            let want = rules::terminal(&args.terminal)?;
            let revised = rt
                .revise(
                    &key,
                    Change::Reterminal {
                        terminal: want.clone(),
                    },
                    args.why.as_bytes(),
                )
                .await?;

            let names: Vec<&str> = want.iter().map(|s| s.as_str()).collect();
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "reterminal": key, "terminal": names,
                        "context": revised.context, "rationale": revised.rationale,
                    })
                );
            } else {
                let view = rt.read(&key).await?;
                println!("{key} terminal set is now: {}", names.join(", "));
                if view.closed {
                    println!("  its current state is in that set, so the anchor is now closed");
                }
                sealed(&revised.context, &revised.rationale);
            }
        }
        What::State => {
            let raw = args.state.expect("What::State implies args.state");
            let value: serde_json::Value = serde_json::from_str(&raw)
                .map_err(|e| CliError(format!("new state is not valid JSON: {e}")))?;
            if !value.is_object() {
                return Err(CliError("new state must be an object".into()));
            }

            let before = rt.read(&key).await?;
            let revised = rt
                .revise(
                    &key,
                    Change::Restate {
                        state: State::new(value.clone()),
                    },
                    args.why.as_bytes(),
                )
                .await?;

            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "restated": key, "from": before.state, "to": value,
                        "context": revised.context, "rationale": revised.rationale,
                    })
                );
            } else {
                println!("{key} state changed");
                println!("  from  {}", before.state.as_value());
                println!("  to    {value}");
                sealed(&revised.context, &revised.rationale);
            }
        }
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(
        probe: Option<&str>,
        rules: &[&str],
        terminal: &[&str],
        state: Option<&str>,
    ) -> ReviseArgs {
        ReviseArgs {
            key: "k".to_owned(),
            probe: probe.map(str::to_owned),
            params: "{}".to_owned(),
            rules: rules.iter().map(|s| (*s).to_owned()).collect(),
            terminal: terminal.iter().map(|s| (*s).to_owned()).collect(),
            state: state.map(str::to_owned),
            why: String::new(),
        }
    }

    #[test]
    fn naming_nothing_to_revise_is_refused() {
        let e = choose(&args(None, &[], &[], None)).unwrap_err();
        assert!(e.to_string().contains("name what to revise"), "{e}");
    }

    #[test]
    fn naming_two_facets_at_once_is_refused() {
        let e = choose(&args(Some("p"), &["true => {}"], &[], None)).unwrap_err();
        assert!(e.to_string().contains("one criterion at a time"), "{e}");
    }

    #[test]
    fn each_lone_facet_is_accepted() {
        assert!(matches!(
            choose(&args(Some("p"), &[], &[], None)).unwrap(),
            What::Probe
        ));
        assert!(matches!(
            choose(&args(None, &["true => {}"], &[], None)).unwrap(),
            What::Rules
        ));
        assert!(matches!(
            choose(&args(None, &[], &["done"], None)).unwrap(),
            What::Terminal
        ));
        assert!(matches!(
            choose(&args(None, &[], &[], Some("{}"))).unwrap(),
            What::State
        ));
    }
}
