use gmr::{Change, Runtime};

use crate::error::CliError;
use crate::rules;
use crate::verbs::sealed;

pub async fn run(
    rt: &Runtime,
    key: String,
    terminal: Vec<String>,
    why: String,
    json: bool,
) -> Result<i32, CliError> {
    let key = super::resolve_one(rt, &key).await?;
    let want = rules::terminal(&terminal);
    let revised = rt
        .revise(
            &key,
            Change::Reterminal {
                terminal: want.clone(),
            },
            why.as_bytes(),
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
    Ok(0)
}
