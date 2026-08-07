use std::path::Path;

use gmr::{AnchorKey, Observed, Runtime};

use crate::delivery::Subscriptions;
use crate::error::CliError;
use crate::probes::Catalog;

pub async fn run(
    rt: &Runtime,
    root: &Path,
    key: Option<String>,
    json: bool,
) -> Result<i32, CliError> {
    let keys = match key {
        Some(k) => vec![AnchorKey::new(k)],
        None => rt.anchors().await?,
    };
    let subs = Subscriptions::load(root, &Catalog::load(root)?)?;

    let mut handed: Vec<(AnchorKey, String, Vec<String>)> = Vec::new();
    let mut unclaimed = Vec::new();
    let mut unseen = Vec::new();
    let mut quiet = 0;

    for key in &keys {
        match rt.observe(key).await? {
            Observed::Transitioned { to, .. } => {
                let memories =
                    super::observe::delivered(rt, &subs, key, &to, &mut unclaimed).await?;
                let status = to.status().map(|s| s.to_string()).unwrap_or_default();
                match memories.is_empty() {
                    true => quiet += 1,
                    false => handed.push((key.clone(), status, memories)),
                }
            }
            Observed::Attempt { code, message, .. } => {
                unseen.push((key.clone(), format!("{code:?}: {message}")))
            }
            _ => {}
        }
    }

    let wrong = !handed.is_empty() || !unclaimed.is_empty() || !unseen.is_empty();

    if json {
        println!(
            "{}",
            serde_json::json!({
                "observed": keys.len(),
                "handed_back": handed.iter().map(|(k, s, m)| serde_json::json!({
                    "anchor": k, "status": s, "memories": m
                })).collect::<Vec<_>>(),
                "moved_unwatched": quiet,
                "unclaimed": unclaimed,
                "unseen": unseen.iter().map(|(k, m)| serde_json::json!({
                    "anchor": k, "detail": m
                })).collect::<Vec<_>>(),
            })
        );
        return Ok(i32::from(wrong));
    }

    for (key, status, memories) in &handed {
        println!("{key}   {status}");
        for m in memories {
            println!("  → {m}");
        }
    }
    if !unseen.is_empty() {
        println!("\n{} could not be looked at:", unseen.len());
        for (key, detail) in &unseen {
            println!("  ! {key}  {detail}");
        }
    }
    super::observe::report_unclaimed(&unclaimed);

    match (handed.len(), quiet) {
        (0, 0) if unseen.is_empty() && unclaimed.is_empty() => {
            println!("{} anchors, nothing moved.", keys.len())
        }
        (0, n) if n > 0 => println!(
            "\n{} anchors, {n} moved on axes nobody asked about. `gmr status` shows them.",
            keys.len()
        ),
        (n, _) => println!(
            "\n{n} of {} handed a memory back. Re-read it: does what you wrote still hold?",
            keys.len()
        ),
    }
    Ok(i32::from(wrong))
}
