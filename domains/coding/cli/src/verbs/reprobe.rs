use gmr::{AnchorKey, Change, Runtime};

use crate::error::CliError;
use crate::verbs::sealed;

pub async fn run(
    rt: &Runtime,
    key: String,
    probe: String,
    params: String,
    why: String,
    json: bool,
) -> Result<i32, CliError> {
    let key = AnchorKey::new(key);
    let probe = crate::rules::probe(&probe, &params)?;
    let revised = rt
        .revise(&key, Change::Reprobe { probe }, why.as_bytes())
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
    Ok(0)
}
