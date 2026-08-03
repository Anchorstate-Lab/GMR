use gmr::{Edge, Runtime, Standing, StatusId};

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
    // not apply to them. Print and label them separately. `None` means a
    // `--status` filter was given, so standing was not computed at all —
    // distinct from `Some(vec![])`, which means it was computed and nothing
    // is currently stale or rewritten.
    match &out.standing {
        None => println!("\n(standing conditions are not computed when --status filters edges)"),
        Some(standing) if standing.is_empty() => {}
        Some(standing) => {
            println!("\nCurrent standing conditions (cursor-independent; repeated every time)");
            for s in standing {
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
    }

    println!("\ncursor {}", out.cursor);
    Ok(0)
}
