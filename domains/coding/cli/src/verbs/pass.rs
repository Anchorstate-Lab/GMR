use std::path::Path;

use gmr::Runtime;

use crate::delivery::Subscriptions;
use crate::error::CliError;
use crate::probes::Catalog;

pub async fn run(rt: &Runtime, root: &Path, json: bool) -> Result<i32, CliError> {
    let p = rt.pass().await?;
    let subs = Subscriptions::load(root, &Catalog::load(root)?)?;

    let mut moved = Vec::new();
    let mut unclaimed = Vec::new();
    let mut handed = 0;
    for key in &p.moved {
        let state = rt.read(key).await?.state;
        let memories = super::observe::delivered(rt, &subs, key, &state, &mut unclaimed).await?;
        handed += memories.len();
        moved.push((key, memories));
    }

    if json {
        let rows: Vec<_> = moved
            .iter()
            .map(|(key, memories)| serde_json::json!({ "anchor": key, "memories": memories }))
            .collect();
        println!(
            "{}",
            serde_json::json!({
                "observed": p.observed, "moved": rows, "handed_back": handed,
                "unseen": p.unseen, "retired": p.retired,
            })
        );
    } else if p.observed == 0 {
        println!("nothing was due — a pass only observes anchors whose cadence has come round");
    } else {
        println!(
            "observed {} | moved {} | handed back {handed} | unseen {} | retired {}",
            p.observed,
            p.moved.len(),
            p.unseen,
            p.retired
        );
        for (key, memories) in &moved {
            if memories.is_empty() {
                continue;
            }
            println!("  {key}");
            for m in memories {
                println!("    → {m}");
            }
        }
        super::observe::report_unclaimed(&unclaimed);
    }
    Ok(if p.moved.is_empty() { 0 } else { 1 })
}
