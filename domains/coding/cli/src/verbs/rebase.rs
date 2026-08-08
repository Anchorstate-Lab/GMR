use gmr::Runtime;

use crate::error::CliError;
use crate::verbs::sealed;

pub async fn run(
    rt: &Runtime,
    keys: Vec<String>,
    all: bool,
    why: String,
    json: bool,
) -> Result<i32, CliError> {
    let keys = match all {
        true => crate::verbs::swapped(rt, &rt.anchors().await?)
            .await?
            .into_iter()
            .map(|(key, _)| key)
            .collect(),
        false => {
            let mut out = Vec::new();
            for arg in &keys {
                out.push(crate::verbs::resolve_one(rt, arg).await?);
            }
            out
        }
    };
    if keys.is_empty() {
        println!("no anchor is standing on a reading a different instrument took");
        return Ok(0);
    }

    let mut done = Vec::new();
    for key in &keys {
        done.push((
            key.clone(),
            crate::verbs::recapture(rt, key, why.as_bytes()).await?,
        ));
    }

    if json {
        println!(
            "{}",
            serde_json::json!({
                "rebased": done.iter().map(|(k, _)| k).collect::<Vec<_>>(),
                "context": done.first().map(|(_, r)| &r.context),
                "rationale": done.first().map(|(_, r)| &r.rationale),
            })
        );
        return Ok(0);
    }

    for (key, _) in &done {
        println!("{key} recaptured");
    }
    if let Some((_, revised)) = done.first() {
        sealed(&revised.context, &revised.rationale);
    }
    Ok(0)
}
