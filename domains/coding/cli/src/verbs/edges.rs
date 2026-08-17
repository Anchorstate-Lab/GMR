use gmr::{Before, Edge, Runtime, Standing, StatusId};

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
                        before,
                        ..
                    } => {
                        let tail = match before {
                            Before::Retrieved { .. } => "",
                            Before::NotRetained => "  the bound version was not kept",
                            Before::NoHistory => "  this provider keeps no history",
                            Before::Unreachable { .. } => {
                                "  the bound version could not be reached"
                            }
                        };
                        println!("rewritten   {anchor}  {}{tail}", reference.external_id);
                    }
                    Standing::Gone {
                        anchor, reference, ..
                    } => println!(
                        "gone        {anchor}  {}  the provider says this record is gone",
                        reference.external_id
                    ),
                    Standing::NoProvider {
                        anchor,
                        reference,
                        provider,
                    } => println!(
                        "no provider {anchor}  {}  `{provider}` is not registered in this binary",
                        reference.external_id
                    ),
                    Standing::Unreachable {
                        anchor,
                        reference,
                        why,
                        ..
                    } => println!("unreachable {anchor}  {}  {why}", reference.external_id),
                }
            }
        }
    }

    println!("\ncursor {}", out.cursor);
    Ok(0)
}
