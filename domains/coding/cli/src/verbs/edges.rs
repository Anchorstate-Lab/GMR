use gmr::{AnchorKey, Edge, Runtime, Standing, StatusId};

use crate::error::CliError;

pub async fn run(
    rt: &Runtime,
    since: u64,
    status: Option<String>,
    json: bool,
) -> Result<i32, CliError> {
    let status = status.map(StatusId::new);
    let out = rt.changed_since(since, status.as_ref()).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(0);
    }

    for e in &out.edges {
        match e {
            Edge::Transitioned {
                anchor, to, status, ..
            } => match status {
                Some(s) => println!("transition  {anchor}  -> {s}"),
                None => println!("transition  {anchor}  -> {}", to.as_value()),
            },
            Edge::Closed {
                anchor,
                self_sealed,
                ..
            } => {
                let by = if *self_sealed {
                    "entered terminal state"
                } else {
                    "closed by author"
                };
                println!("closed      {anchor}  {by}");
            }
            Edge::Stalled {
                anchor,
                count,
                last,
                ..
            } => println!("unseen      {anchor}  {count} consecutive failed attempts ({last:?})"),
        }
    }

    // Standing conditions do not come from the journal, so "after cursor" does
    // not apply to them. Print and label them separately.
    if !out.standing.is_empty() {
        println!("\nCurrent standing conditions (cursor-independent; repeated every time)");
        for s in &out.standing {
            match s {
                Standing::Stale {
                    anchor,
                    last_sighting,
                } => match last_sighting {
                    Some(t) => println!("stale       {anchor}  last sighting {t}"),
                    None => println!("stale       {anchor}  never sighted"),
                },
                Standing::Rewritten {
                    anchor,
                    reference,
                    retrievable,
                    ..
                } => {
                    let tail = match retrievable {
                        Some(false) => "  bound version is no longer retrievable",
                        _ => "",
                    };
                    println!("rewritten   {anchor}  {}{tail}", reference.external_id);
                }
            }
        }
    }

    println!("\ncursor {}", out.cursor);
    Ok(0)
}

pub async fn health(rt: &Runtime, key: Option<String>, json: bool) -> Result<i32, CliError> {
    let keys = match key {
        Some(k) => vec![AnchorKey::new(k)],
        None => rt.anchors().await?,
    };

    let mut per_anchor = Vec::new();
    for k in &keys {
        per_anchor.push(rt.health(k).await?);
    }
    let corpus = rt.corpus_health().await?;

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
