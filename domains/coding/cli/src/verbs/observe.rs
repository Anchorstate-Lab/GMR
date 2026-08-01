use gmr::{AnchorKey, Observed, Runtime};

use crate::error::CliError;

pub async fn run(rt: &Runtime, key: Option<String>, json: bool) -> Result<i32, CliError> {
    let keys = match key {
        Some(k) => vec![AnchorKey::new(k)],
        None => rt.anchors().await?,
    };

    let mut moved = 0;
    let mut report = Vec::new();
    for key in &keys {
        let observed = rt.observe(key).await?;
        let (word, detail) = match &observed {
            Observed::Transitioned { from, to } if from == to => ("settled", None),
            Observed::Transitioned { to, .. } => {
                moved += 1;
                ("moved", Some(to.as_value().to_string()))
            }
            Observed::Still => ("still", None),
            Observed::Attempt { reason, message } => {
                ("unseen", Some(format!("{reason:?}: {message}")))
            }
            Observed::Closed => ("closed", None),
        };

        if json {
            report.push(serde_json::json!({
                "anchor": key, "observed": word, "detail": detail,
            }));
        } else if word != "still" {
            match &detail {
                Some(d) => println!("{key}  {word}  {d}"),
                None => println!("{key}  {word}"),
            }
        }
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("observed {} anchors, {moved} moved", keys.len());
    }
    Ok(if moved > 0 { 1 } else { 0 })
}
