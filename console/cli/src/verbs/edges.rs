use gmr::{Before, Edge, Raised, Runtime, StatusId};

use crate::error::CliError;
use crate::memories::Names;

pub async fn run(
    rt: &Runtime,
    names: &Names,
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

    match &out.raised {
        None => println!("\n(standing conditions are not computed when --status filters edges)"),
        Some(standing) if standing.is_empty() => {}
        Some(standing) => {
            println!("\nCurrent standing conditions (cursor-independent; repeated every time)");
            for s in standing {
                match s {
                    Raised::Stale {
                        anchor,
                        last_sighting,
                    } => match last_sighting {
                        Some(t) => println!("stale       {anchor}  last sighting {t}"),
                        None => println!("stale       {anchor}  never sighted"),
                    },
                    Raised::Rewritten {
                        anchor,
                        reference,
                        before,
                        ..
                    } => {
                        let tail = match before {
                            Before::Retrieved { .. } => "",
                            Before::NotRetained => "  the bound version was not kept",
                            Before::NotAsked => "  history was not asked for",
                            Before::NoHistory => "  this provider keeps no history",
                            Before::Unreachable { .. } => {
                                "  the bound version could not be reached"
                            }
                        };
                        println!("rewritten   {anchor}  {}{tail}", names.of(reference));
                    }
                    Raised::Gone {
                        anchor, reference, ..
                    } => println!(
                        "gone        {anchor}  {}  the provider says this record is gone",
                        names.of(reference)
                    ),
                    Raised::NoProvider {
                        anchor,
                        reference,
                        provider,
                    } => println!(
                        "no provider {anchor}  {}  `{provider}` is not registered in this binary",
                        names.of(reference)
                    ),
                    Raised::Unreachable {
                        anchor,
                        reference,
                        why,
                        ..
                    } => println!("unreachable {anchor}  {}  {why}", names.of(reference)),
                }
            }
        }
    }

    println!("\ncursor {}", out.cursor);
    Ok(0)
}
