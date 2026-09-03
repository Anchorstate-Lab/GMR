use gmr::{Claim, Depends, Runtime, Shown, Standing};

use crate::error::CliError;

pub async fn run(
    rt: &Runtime,
    id: Option<String>,
    retire: bool,
    fresher_than_secs: Option<u64>,
    json: bool,
) -> Result<i32, CliError> {
    let claims: Vec<Claim> = match &id {
        Some(named) if named.contains(':') => vec![Claim::parse(named).ok_or_else(|| {
            CliError(format!(
                "`{named}` names nothing -- a stored record is `<provider>:<id>`, a \
                 conclusion is `said:<id>`"
            ))
        })?],
        Some(named) => vec![Claim::said(named.as_str())],
        None => rt
            .claims()
            .await?
            .into_iter()
            .filter(|c| c.stored().is_none())
            .collect(),
    };
    if retire {
        let Some(one) = claims.first() else {
            return Err(CliError("name the conclusion to retire".into()));
        };
        let sources = rt.memory().binding_of(one).await?.sources();
        let source =
            match !sources.is_empty() && sources.iter().all(|s| *s == gmr::Source::SelfAttested) {
                true => gmr::Source::SelfAttested,
                false => gmr::Source::Adjudicated,
            };
        let cleared = rt.revoke(one, source).await?;
        println!(
            "{one} retired on {}",
            cleared
                .iter()
                .map(gmr::AnchorKey::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        );
        println!("  what it said stays in the table; nothing asks about it any more");
        return Ok(0);
    }
    let stood: Vec<Standing> = match claims.is_empty() {
        true => Vec::new(),
        false => rt
            .ground(
                &claims
                    .iter()
                    .cloned()
                    .map(gmr::Asked::about)
                    .collect::<Vec<_>>(),
                &match fresher_than_secs {
                    Some(secs) => {
                        gmr::Instructions::fresher_than(std::time::Duration::from_secs(secs))
                    }
                    None => gmr::Instructions::default(),
                },
            )
            .await?
            .into_iter()
            .filter(|s| !s.on.is_empty())
            .collect(),
    };

    if json {
        println!("{}", serde_json::to_string(&stood).map_err(render)?);
        return Ok(exit_of(&stood));
    }
    if stood.is_empty() {
        println!("nothing is being asked about here; `gmr said` records a conclusion");
        return Ok(0);
    }

    for one in &stood {
        let text = said_text(one);
        println!("{}  {text}", one.claim);
        for anchored in &one.on {
            let Some(warrant) = anchored.warrant() else {
                println!("    {}   never opened", anchored.key());
                continue;
            };
            let shown = match anchored.evidence().map(|e| e.shown.clone()) {
                Some(Shown::Seen { at }) => format!("saw its reading at {at}"),
                Some(Shown::Superseded { at }) => format!(
                    "cited a reading (at {at}) the anchor had already replaced when this \
                     conclusion landed"
                ),
                Some(Shown::Unseen) => "cited a reading this anchor never took".to_owned(),
                _ => "cited no reading".to_owned(),
            };
            let moved = crate::render::holding(&warrant.holding)
                .unwrap_or_else(|| "the ground still holds".to_owned());
            println!("    {}   {moved}   {shown}", anchored.key());
        }
        match &one.depends {
            Depends::Holds => println!("    depends: still holds"),
            Depends::Broken => println!("    depends: no longer holds"),
            Depends::Ungrounded { missing } => println!(
                "    depends: cannot be answered — {} named and never opened here",
                missing
                    .iter()
                    .map(|k| k.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Depends::Vacuous { wrote } => {
                println!("    depends: `{wrote}` reads no anchor, so nothing could ever break it")
            }
            Depends::Unevaluable { why } => println!("    depends: cannot be settled — {why}"),
            Depends::Unstated if moved(one) => {
                println!("    depends: nothing was stated, and the ground moved — re-read this one")
            }
            Depends::Unstated => {}
        }
    }

    let (broken, unseen, bare) = counted(&stood);
    println!();
    println!(
        "{} conclusion(s) · {broken} the ground no longer settles · {unseen} built beside \
         an anchor rather than through it · {bare} that cited no reading at all",
        stood.len()
    );
    Ok(exit_of(&stood))
}

fn said_text(one: &Standing) -> String {
    match &one.claim {
        Claim::Said {
            asserts: Some(v), ..
        } => v
            .get("text")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_owned(),
        _ => String::new(),
    }
}

fn dead_ground(one: &Standing) -> bool {
    one.on.iter().any(|a| {
        matches!(a, gmr::Anchored::Unopened { .. })
            || a.warrant()
                .is_some_and(|w| matches!(w.holding, gmr::Holding::Finished))
    })
}

fn moved(one: &Standing) -> bool {
    one.on.iter().any(|a| {
        a.warrant()
            .is_some_and(|w| !matches!(w.holding, gmr::Holding::Holds))
            || a.evidence()
                .is_some_and(|e| matches!(e.shown, Shown::Superseded { .. }))
    })
}

fn unsettled(one: &Standing) -> bool {
    if dead_ground(one) {
        return true;
    }
    match &one.depends {
        Depends::Broken
        | Depends::Unevaluable { .. }
        | Depends::Vacuous { .. }
        | Depends::Ungrounded { .. } => true,
        Depends::Holds => false,
        Depends::Unstated => moved(one),
    }
}

fn counted(stood: &[Standing]) -> (usize, usize, usize) {
    let broken = stood.iter().filter(|s| unsettled(s)).count();
    let unseen = stood
        .iter()
        .filter(|s| {
            s.on.iter()
                .any(|a| a.evidence().is_some_and(|e| e.shown == Shown::Unseen))
        })
        .count();
    let bare = stood
        .iter()
        .filter(|s| {
            !s.on.is_empty()
                && s.on
                    .iter()
                    .all(|a| a.evidence().is_some_and(|e| e.shown == Shown::NotSaid))
        })
        .count();
    (broken, unseen, bare)
}

fn exit_of(stood: &[Standing]) -> i32 {
    let (broken, unseen, _) = counted(stood);
    match broken + unseen {
        0 => 0,
        _ => 1,
    }
}

fn render(e: serde_json::Error) -> CliError {
    CliError(e.to_string())
}
