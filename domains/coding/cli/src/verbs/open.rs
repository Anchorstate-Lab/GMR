use gmr::{AnchorKey, OpenRequest, Retain, RunSettings, Runtime, Supersede};

use crate::cli::OpenArgs;
use crate::error::CliError;
use crate::rules;

pub async fn run(rt: &Runtime, args: OpenArgs, json: bool) -> Result<i32, CliError> {
    let key = AnchorKey::new(args.key);
    let supersedes = args.supersedes.zip(args.why).map(|(k, why)| Supersede {
        key: AnchorKey::new(k),
        rationale: why.into_bytes(),
    });
    let opened = rt
        .open(OpenRequest {
            key: key.clone(),
            probe: rules::probe(&args.probe, &args.params)?,
            transitions: rules::transitions(&args.rules)?,
            terminal: rules::terminal(&args.terminal),
            initial: None,
            settings: RunSettings {
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
