use gmr::{OpenRequest, Retain, RunSettings, Runtime, State, Supersede};

use crate::cli::OpenArgs;
use crate::error::CliError;
use crate::probes::Catalog;
use crate::rules;

pub async fn run(
    rt: &Runtime,
    root: &std::path::Path,
    args: OpenArgs,
    json: bool,
) -> Result<i32, CliError> {
    let catalog = Catalog::load(root)?;
    let key = rules::key(&args.key)?;

    let routed = match &args.probe {
        Some(_) => None,
        None => Some(
            crate::coord::route(&args.key, args.shape.as_deref(), &catalog)
                .map_err(|e| CliError(format!("{key}: {e}")))?,
        ),
    };
    let probe_name = match (&args.probe, &routed) {
        (Some(p), _) => p.clone(),
        (None, Some(r)) => r.probe.clone(),
        (None, None) => unreachable!("a routed coordinate always names a probe"),
    };
    let initial = routed
        .as_ref()
        .map(|r| State::new(serde_json::json!({ "position": r.position })));

    let shape = routed
        .as_ref()
        .map(|r| r.shape.as_str())
        .or(args.shape.as_deref());
    let transitions = match (shape, args.rules.is_empty()) {
        (Some(name), true) => rules::transitions(&crate::shapes::rules_of(
            crate::shapes::get(name).map_err(|e| CliError(format!("{key}: {e}")))?,
        )),
        (None, _) => rules::transitions(&args.rules),
        (Some(_), false) => unreachable!("clap already refuses --shape together with --rule"),
    }?;

    let reads =
        crate::contract::reads_of(&transitions).map_err(|e| CliError(format!("{key}: {e}")))?;
    let missing = crate::contract::unmet(&reads, &catalog.obs_of(&probe_name)?);
    if !missing.is_empty() {
        return Err(CliError(format!(
            "{key}: rules read {}, which probe `{probe_name}` does not emit",
            missing.join(" · ")
        )));
    }

    let supersedes = match args.supersedes.zip(args.why) {
        None => None,
        Some((k, why)) => Some(Supersede {
            key: rules::key(&k)?,
            rationale: why.into_bytes(),
        }),
    };
    let opened = rt
        .open(OpenRequest {
            key: key.clone(),
            probe: rules::probe(catalog.kind_of(&probe_name), &probe_name, &args.params)?,
            transitions,
            terminal: rules::terminal(&args.terminal)?,
            initial,
            settings: RunSettings {
                budget_ms: args.budget_ms,
                retain: if args.retain_full {
                    Retain::Full
                } else {
                    Retain::Tick
                },
                cadence_secs: args.cadence_secs,
            },
            supersedes,
        })
        .await?;

    if json {
        println!(
            "{}",
            serde_json::json!({
                "opened": key,
                "state": opened.state,
                "warnings": opened.warnings,
                "supersedes": opened.supersedes,
            })
        );
    } else {
        println!("{key} opened, initial state {}", opened.state.as_value());
        if let Some(old) = &opened.supersedes {
            println!("  supersedes {old}; that generation is closed and the rationale is sealed");
        }
        for w in &opened.warnings {
            println!("  ! {w}");
        }
    }
    Ok(0)
}
