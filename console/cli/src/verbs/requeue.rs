use gmr::Runtime;

use crate::error::CliError;

pub async fn run(rt: &Runtime, key: String, json: bool) -> Result<i32, CliError> {
    let key = super::resolve_one(rt, &key).await?;
    let requeued = rt.requeue(&key).await?;

    if json {
        println!(
            "{}",
            serde_json::json!({ "requeued": requeued, "anchor": key })
        );
        return Ok(0);
    }

    if requeued {
        println!("{key} is due now; any backoff or parked state was cleared");
    } else {
        println!("this deployment has no queue — nothing to requeue");
    }
    Ok(0)
}
