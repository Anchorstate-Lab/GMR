use gmr::Runtime;

use crate::error::CliError;

pub async fn run(rt: &Runtime, json: bool) -> Result<i32, CliError> {
    let p = rt.pass().await?;
    if json {
        println!("{}", serde_json::to_string(&p)?);
    } else {
        println!(
            "observed {} | moved {} | unseen {} | retired {}",
            p.observed, p.moved, p.unseen, p.retired
        );
    }
    Ok(if p.moved > 0 { 1 } else { 0 })
}
