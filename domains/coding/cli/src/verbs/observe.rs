use gmr::{AnchorKey, Observed, Runtime};

use crate::error::CliError;

pub async fn run(rt: &Runtime, key: Option<String>, json: bool) -> Result<i32, CliError> {
    let keys = match key {
        Some(k) => vec![AnchorKey::new(k)],
        None => rt.anchors().await?,
    };

    let mut moved = 0;
    let mut unclaimed = Vec::new();
    let mut report = Vec::new();
    for key in &keys {
        let observed = rt.observe(key).await?;
        let (word, detail) = match &observed {
            Observed::Unchanged { .. } => ("settled", None),
            Observed::Transitioned { to, .. } => {
                moved += 1;
                ("moved", to.status().map(|s| s.to_string()))
            }
            Observed::Still => ("still", None),
            Observed::Attempt { code, message, .. } => {
                ("unseen", Some(format!("{code:?}: {message}")))
            }
            Observed::Closed => ("closed", None),
        };

        let memories = match &observed {
            Observed::Transitioned { .. } => super::memories_on(rt, key).await?,
            _ => Vec::new(),
        };
        if word == "moved" && memories.is_empty() {
            unclaimed.push(key.clone());
        }

        if json {
            let state = match &observed {
                Observed::Transitioned { to, .. } => Some(to.as_value()),
                _ => None,
            };
            report.push(serde_json::json!({
                "anchor": key, "observed": word, "detail": detail,
                "state": state, "memories": memories,
            }));
        } else if word != "still" {
            match &detail {
                Some(d) => println!("{key}  {word}  {d}"),
                None => println!("{key}  {word}"),
            }
            for m in &memories {
                println!("    → {m}");
            }
        }
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("observed {} anchors, {moved} moved", keys.len());
        report_unclaimed(&unclaimed);
    }
    Ok(if moved > 0 { 1 } else { 0 })
}

/// 动了却没有绑定的锚：要么该绑记忆，要么该关掉。
pub(crate) fn report_unclaimed(unclaimed: &[AnchorKey]) {
    if unclaimed.is_empty() {
        return;
    }
    println!("\n{} moved with no note bound to them:", unclaimed.len());
    for k in unclaimed {
        println!("  ? {k}");
    }
}
