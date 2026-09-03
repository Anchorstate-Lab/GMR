use std::collections::BTreeSet;

use gmr::{AnchorKey, Asked, Binding, Claim, FactAddress, Runtime, SaidId, Source};

use crate::error::CliError;

pub struct Said {
    pub id: Option<String>,
    pub text: String,
    pub on: Vec<String>,
    pub saw: Vec<String>,
    pub depends: Option<String>,
}

pub async fn run(rt: &Runtime, asked: Said, json: bool) -> Result<i32, CliError> {
    if asked.on.is_empty() {
        return Err(CliError(
            "name at least one anchor with --on: a conclusion resting on nothing is not \
             something this can hold you to"
                .into(),
        ));
    }
    let id = asked.id.unwrap_or_else(minted);
    let mut anchors: Vec<AnchorKey> = Vec::new();
    for named in &asked.on {
        anchors.push(super::resolve_one(rt, named).await?);
    }
    let saw: BTreeSet<FactAddress> = asked
        .saw
        .iter()
        .map(|a| {
            FactAddress::try_new(a).map_err(|e| {
                CliError(format!(
                    "`--saw {a}` is not the address of a reading: {e}. `gmr read <key> --json` \
                     prints one per anchor as `fact_address`"
                ))
            })
        })
        .collect::<Result<_, _>>()?;

    let claim = Claim::Said {
        id: SaidId::new(&id),
        asserts: Some(serde_json::json!({
            "text": asked.text,
            "at": chrono::Utc::now().to_rfc3339(),
        })),
    };
    let mut binding = Binding::on(claim.clone(), anchors.clone());
    if let Some(source) = &asked.depends {
        gmr::expr::parse(source).map_err(|e| CliError(format!("`--depends {source}`: {e}")))?;
        binding = binding.depending(source);
    }

    let landed = rt
        .bind(binding, None, saw.clone(), Source::SelfAttested)
        .await?;

    let stood = rt
        .ground(
            &[Asked::about(claim.clone())],
            &gmr::Instructions::default(),
        )
        .await?;
    let unseen: Vec<String> = stood
        .first()
        .into_iter()
        .flat_map(|s| &s.on)
        .filter(|a| a.evidence().is_some_and(|e| e.shown == gmr::Shown::Unseen))
        .map(|a| a.key().to_string())
        .collect();
    let superseded: Vec<String> = stood
        .first()
        .into_iter()
        .flat_map(|s| &s.on)
        .filter(|a| {
            a.evidence()
                .is_some_and(|e| matches!(e.shown, gmr::Shown::Superseded { .. }))
        })
        .map(|a| a.key().to_string())
        .collect();
    let finished: Vec<String> = stood
        .first()
        .into_iter()
        .flat_map(|s| &s.on)
        .filter(|a| {
            a.warrant()
                .is_some_and(|w| matches!(w.holding, gmr::Holding::Finished))
        })
        .map(|a| a.key().to_string())
        .collect();

    if json {
        println!(
            "{}",
            serde_json::json!({
                "said": format!("said:{id}"),
                "on": landed.anchors.iter().map(AnchorKey::to_string).collect::<Vec<_>>(),
                "saw": saw.iter().map(FactAddress::as_str).collect::<Vec<_>>(),
                "unseen": unseen,
                "superseded": superseded,
                "finished": finished,
                "recorded": landed.recorded,
            })
        );
        return Ok(0);
    }

    println!(
        "said:{id} → {}",
        landed
            .anchors
            .iter()
            .map(AnchorKey::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    );
    match saw.is_empty() {
        true => println!(
            "  no reading cited: this records what you concluded and nothing about what \
             you were looking at when you concluded it"
        ),
        false => println!("  citing {} reading(s)", saw.len()),
    }
    for key in &unseen {
        println!(
            "  {key} has no entry at any address you cited — it will read `unseen`, which \
             is what a conclusion built beside an anchor rather than through it looks like"
        );
    }
    for key in &superseded {
        println!(
            "  {key} had already replaced the reading you cited before this conclusion \
             landed — it will read `superseded`. Re-read with `gmr read {key}` and \
             conclude from what the anchor is showing now"
        );
    }
    for key in &finished {
        println!(
            "  {key} has finished — its journal is frozen and nothing will ever observe \
             this conclusion. `gmr ground` will report it as no longer settled; conclude \
             on the anchor that succeeded it, or retire this once it has served"
        );
    }
    if !landed.recorded {
        println!("  nothing written: this claim already stands, on these anchors, saying this");
    }
    Ok(0)
}

fn minted() -> String {
    static NTH: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let nth = NTH.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let now = chrono::Utc::now();
    format!(
        "{}-{:x}-{:x}",
        now.format("%Y%m%dT%H%M%S"),
        now.timestamp_subsec_nanos().wrapping_add(nth),
        std::process::id()
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn two_conclusions_minted_in_the_same_instant_get_different_ids() {
        assert_ne!(
            super::minted(),
            super::minted(),
            "a second-resolution timestamp is one shared name per second. Two agents \
             concluding in the same instant would fold into one claim identity: the \
             anchors union, and the later saw and depends silently shadow the earlier"
        );
    }
}
