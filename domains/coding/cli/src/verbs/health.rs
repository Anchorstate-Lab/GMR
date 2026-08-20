use gmr::Runtime;

use crate::error::CliError;

pub async fn run(rt: &Runtime, key: Option<String>, json: bool) -> Result<i32, CliError> {
    let keys = match key {
        Some(k) => super::resolve(rt, &k).await?,
        None => rt.anchors().await?,
    };

    let mut per_anchor = Vec::new();
    for k in &keys {
        per_anchor.push(rt.health(k).await?);
    }
    let corpus = rt.corpus().await?;
    let corpus = corpus.health();

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "anchors": per_anchor, "corpus": corpus,
            }))?
        );
        return Ok(0);
    }

    for h in &per_anchor {
        if h.restate_count == 0 && h.stall_ratio == 0.0 && !h.state_drifted {
            continue;
        }
        println!("{}", h.anchor);
        if h.restate_count > 0 {
            println!("  manual restates  {}", h.restate_count);
            let sizes: Vec<String> = h.rationale_sizes.iter().map(|n| n.to_string()).collect();
            println!("  rationale bytes  {}", sizes.join(", "));
        }
        if h.state_drifted {
            println!("  state differs from the opening state");
        }
        if h.stall_ratio > 0.0 {
            println!("  failed attempt ratio  {:.0}%", h.stall_ratio * 100.0);
        }
        if let Some(f) = &h.last_failure {
            println!("  last failure  {f}");
        }
    }

    println!("\nbound memories  {}", corpus.bound_refs);
    println!("live anchors    {}", corpus.active_anchors);
    if !corpus.barren_anchors.is_empty() {
        let names: Vec<&str> = corpus.barren_anchors.iter().map(|k| k.as_str()).collect();
        println!(
            "barren anchors  {}\n                <- observing a position where nobody has written a memory",
            names.join(", ")
        );
    }
    Ok(0)
}
