use gmr::{AnchorKey, Kind, OpenRequest, Probe, Retain, Runtime};

use crate::error::CliError;
use crate::rules;

pub async fn run(
    rt: &Runtime,
    key: String,
    probe: String,
    rules_text: Vec<String>,
    terminal: Vec<String>,
    json: bool,
) -> Result<i32, CliError> {
    let key = AnchorKey::new(key);
    let opened = rt
        .open(OpenRequest {
            key: key.clone(),
            probe: Probe::new(Kind::new("shell"), serde_json::json!({ "run": probe })),
            transitions: rules::transitions(&rules_text)?,
            terminal: rules::terminal(&terminal),
            initial: None,
            retain: Retain::Tick,
            cadence_secs: None,
        })
        .await?;

    if json {
        println!(
            "{}",
            serde_json::json!({
                "opened": key,
                "state": opened.state,
                "warnings": opened.warnings,
            })
        );
    } else {
        println!("{key} 已开，起始状态 {}", opened.state.as_value());
        for w in &opened.warnings {
            println!("  ! {w}");
        }
    }
    Ok(0)
}
