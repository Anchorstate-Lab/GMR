use std::path::Path;

use gmr::{AnchorKey, Observed, Runtime, State};

use crate::delivery::Subscriptions;
use crate::error::CliError;
use crate::memories::Names;
use crate::probes::Catalog;

pub async fn run(
    rt: &Runtime,
    root: &Path,
    names: &Names,
    key: Option<String>,
    json: bool,
) -> Result<i32, CliError> {
    let keys = match key {
        Some(k) => super::resolve(rt, &k).await?,
        None => rt.anchors().await?,
    };
    let (subs, _) = Subscriptions::load(root, &Catalog::load(root)?, names)?;

    let mut moved = 0;
    let mut handed = 0;
    let mut unclaimed = Vec::new();
    let mut report = Vec::new();
    for key in &keys {
        let observed = rt.observe(key).await?;
        let (word, detail) = match &observed {
            Observed::Unchanged { .. } => ("settled", None),
            Observed::Transitioned { to, .. } => {
                moved += 1;
                ("moved", to.status().map(|s| s.to_string()))
            }
            Observed::Still => ("still", None),
            Observed::Attempt { code, message, .. } => {
                ("unseen", Some(format!("{code:?}: {message}")))
            }
            Observed::Closed => ("closed", None),
        };

        let memories = match &observed {
            Observed::Transitioned { to, .. } => {
                delivered(rt, &subs, key, to, true, &mut unclaimed).await?
            }
            _ => Vec::new(),
        };
        handed += memories.len();

        if json {
            let state = match &observed {
                Observed::Transitioned { to, .. } => Some(to.as_value()),
                _ => None,
            };
            report.push(serde_json::json!({
                "anchor": key, "observed": word, "detail": detail,
                "state": state, "memories": addressed_all(&memories),
            }));
        } else if word != "still" {
            match &detail {
                Some(d) => println!("{key}  {word}  {d}"),
                None => println!("{key}  {word}"),
            }
            for m in &memories {
                println!("    → {}", names.of(m));
            }
        }
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "observed {} anchors, {moved} moved, {handed} handed back",
            keys.len()
        );
        report_unclaimed(&unclaimed);
    }
    Ok(if moved > 0 { 1 } else { 0 })
}

pub(crate) async fn delivered(
    rt: &Runtime,
    subs: &Subscriptions,
    key: &AnchorKey,
    to: &State,
    moved: bool,
    unclaimed: &mut Vec<AnchorKey>,
) -> Result<Vec<gmr::Ref>, CliError> {
    let bound = super::memories_on(rt, key).await?;
    if bound.is_empty() {
        if moved {
            unclaimed.push(key.clone());
        }
        return Ok(Vec::new());
    }
    let shape = crate::shapes::of(&rt.read(key).await?.anchor.transitions);
    Ok(bound
        .into_iter()
        .filter(|m| subs.delivers(shape, m, to, moved))
        .collect())
}

pub(crate) fn report_unclaimed(unclaimed: &[AnchorKey]) {
    if unclaimed.is_empty() {
        return;
    }
    println!("\n{} moved with no note bound to them:", unclaimed.len());
    for k in unclaimed {
        println!("  ? {k}");
    }
}

pub(crate) fn addressed_all(refs: &[gmr::Ref]) -> Vec<String> {
    refs.iter().map(crate::memories::addressed).collect()
}
