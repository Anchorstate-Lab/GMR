use gmr::{AnchorKey, OpenRequest, Retain, Runtime, Supersede};

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
            probe: rules::probe(&args.artifact, &args.params)?,
            transitions: rules::transitions(&args.rules)?,
            terminal: rules::terminal(&args.terminal),
            initial: None,
            retain: Retain::Tick,
            cadence_secs: None,
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
        println!("{key} 已开，起始状态 {}", opened.state.as_value());
        if let Some(old) = &opened.supersedes {
            println!("  接替 {old} —— 那一代终结了，理由已密封");
        }
        for w in &opened.warnings {
            println!("  ! {w}");
        }
    }
    Ok(0)
}
