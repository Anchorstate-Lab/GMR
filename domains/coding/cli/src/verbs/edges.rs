use gmr::{AnchorKey, Edge, Runtime, Stall, StatusId};

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
                Some(s) => println!("转换      {anchor}  → {s}"),
                None => println!("转换      {anchor}  → {}", to.as_value()),
            },
            Edge::Closed {
                anchor,
                self_sealed,
                ..
            } => {
                let by = if *self_sealed {
                    "走进终结态"
                } else {
                    "作者关的"
                };
                println!("终结      {anchor}  {by}");
            }
            Edge::Stalled { anchor, reason, .. } => match reason {
                Stall::Attempts { count, last } => {
                    println!("看不见    {anchor}  连续 {count} 次没看成（{last:?}）")
                }
                Stall::Stale { last_sighting } => match last_sighting {
                    Some(t) => println!("陈旧      {anchor}  上次看到是 {t}"),
                    None => println!("陈旧      {anchor}  从没看到过"),
                },
            },
            Edge::Rewritten {
                anchor,
                reference,
                retrievable,
                ..
            } => {
                let tail = match retrievable {
                    Some(false) => "  当初那一版已取不回",
                    _ => "",
                };
                println!("记录改写  {anchor}  {}{tail}", reference.external_id);
            }
        }
    }

    println!("\n游标 {}", out.cursor);
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
            println!("  手动设过状态  {} 次", h.restate_count);
            let sizes: Vec<String> = h.rationale_sizes.iter().map(|n| n.to_string()).collect();
            println!("  理由的字数    {}", sizes.join(" · "));
        }
        if h.state_drifted {
            println!("  状态跟开锚那一刻已经不同");
        }
        if h.stall_ratio > 0.0 {
            println!("  没看成的比例  {:.0}%", h.stall_ratio * 100.0);
        }
        if let Some(f) = &h.last_failure {
            println!("  最近一次失败  {f}");
        }
    }

    println!("\n被锚的记录  {}", corpus.bound_refs);
    println!("活着的锚    {}", corpus.active_anchors);
    if !corpus.barren_anchors.is_empty() {
        let names: Vec<&str> = corpus.barren_anchors.iter().map(|k| k.as_str()).collect();
        println!(
            "没有记忆    {}\n            ← 在守一个没人写过东西的位置，纯观测开销",
            names.join(" · ")
        );
    }
    Ok(0)
}
