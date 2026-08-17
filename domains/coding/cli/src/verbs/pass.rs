use std::path::Path;

use gmr::Runtime;

use crate::delivery::Subscriptions;
use crate::error::CliError;
use crate::probes::Catalog;

pub async fn run(rt: &Runtime, root: &Path, json: bool) -> Result<i32, CliError> {
    let p = rt.pass().await?;
    let (subs, _) = Subscriptions::load(root, &Catalog::load(root)?)?;
    let source = crate::memories::declaring(root);

    let mut moved = Vec::new();
    let mut unclaimed = Vec::new();
    let mut handed = 0;
    for key in &p.moved {
        let state = rt.read(key).await?.state;
        let memories =
            super::observe::delivered(rt, &subs, key, &state, true, &mut unclaimed).await?;
        handed += memories.len();
        moved.push((key, memories));
    }

    if json {
        let rows: Vec<_> = moved
            .iter()
            .map(|(key, memories)| serde_json::json!({ "anchor": key, "memories": super::observe::shown_all(memories, &source) }))
            .collect();
        println!(
            "{}",
            serde_json::json!({
                "observed": p.observed, "moved": rows, "handed_back": handed,
                "unseen": p.unseen, "retired": p.retired, "skipped": p.skipped,
            })
        );
    } else if p.observed == 0 && p.skipped == 0 {
        println!("nothing was due — a pass only observes anchors whose cadence has come round");
    } else {
        println!(
            "observed {} | moved {} | handed back {handed} | unseen {} | retired {}",
            p.observed,
            p.moved.len(),
            p.unseen,
            p.retired
        );
        if p.skipped > 0 {
            println!(
                "  {} due anchors were never looked at — this pass ran out of its probe budget \
                 before their turn. They keep their place at the front of the next pass. If this \
                 number does not fall, the batch is too small for the queue or the budget is too \
                 tight: raise --probe-budget-ms",
                p.skipped
            );
        }
        for (key, memories) in &moved {
            if memories.is_empty() {
                continue;
            }
            println!("  {key}");
            for m in memories {
                println!("    → {}", crate::memories::shown(m, &source));
            }
        }
        super::observe::report_unclaimed(&unclaimed);
    }
    Ok(if p.moved.is_empty() { 0 } else { 1 })
}
