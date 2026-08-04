use gmr::Runtime;

use crate::error::CliError;

pub async fn run(rt: &Runtime, json: bool) -> Result<i32, CliError> {
    let p = rt.pass().await?;

    let mut moved = Vec::new();
    let mut unclaimed = Vec::new();
    for key in &p.moved {
        let memories = super::memories_on(rt, key).await?;
        if memories.is_empty() {
            unclaimed.push(key.clone());
        }
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
                "observed": p.observed, "moved": rows,
                "unseen": p.unseen, "retired": p.retired,
            })
        );
    } else {
        println!(
            "observed {} | moved {} | unseen {} | retired {}",
            p.observed,
            p.moved.len(),
            p.unseen,
            p.retired
        );
        for (key, memories) in &moved {
            println!("  {key}");
            for m in memories {
                println!("    → {m}");
            }
        }
        super::observe::report_unclaimed(&unclaimed);
    }
    Ok(if p.moved.is_empty() { 0 } else { 1 })
}
